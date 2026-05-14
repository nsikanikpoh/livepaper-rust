use anyhow::Result;
use neo4rs::{Graph, ConfigBuilder};

use crate::models::PaperExtraction;

pub struct Neo4jDb {
    pub graph: Graph,
}

impl Neo4jDb {
    pub async fn new(uri: &str, user: &str, password: &str, db: &str) -> Result<Self> {
        let mut builder = ConfigBuilder::new()
            .uri(uri)
            .user(user)
            .password(password);

        // Only set the db name when explicitly provided.
        // On Neo4j Community Edition the implicit default database is used
        // when no name is given. Passing an explicit name that doesn't exist
        // raises Neo.ClientError.Database.DatabaseNotFound.
        if !db.is_empty() {
            builder = builder.db(db);
        }

        let graph = Graph::connect(builder.build()?).await?;
        Ok(Self { graph })
    }

    pub async fn ensure_constraints(&self) -> Result<()> {
        let stmts = vec![
            "CREATE CONSTRAINT paper_id IF NOT EXISTS ON (p:Paper) ASSERT p.id IS UNIQUE",
            "CREATE CONSTRAINT author_email IF NOT EXISTS ON (a:Author) ASSERT a.email IS UNIQUE",
            "CREATE CONSTRAINT expert_email IF NOT EXISTS ON (e:Expert) ASSERT e.email IS UNIQUE",
            "CREATE CONSTRAINT concept_name IF NOT EXISTS ON (c:Concept) ASSERT c.name IS UNIQUE",
        ];
        for s in stmts {
            self.graph.run(neo4rs::query(s)).await.ok();
        }
        Ok(())
    }

    // ── Paper ─────────────────────────────────────────────────────────────

    pub async fn upsert_paper(&self, paper_id: &str, title: &str, paper_url: &str, abstract_text: &str) -> Result<()> {
        self.graph.run(
            neo4rs::query("MERGE (p:Paper {id: $id}) SET p.title=$title, p.paper_url=$url, p.abstract=$abs")
                .param("id", paper_id)
                .param("title", title)
                .param("url", paper_url)
                .param("abs", abstract_text),
        ).await.map_err(|e| anyhow::anyhow!("Neo4j upsert_paper: {e}"))?;
        Ok(())
    }

    // ── Author ────────────────────────────────────────────────────────────

    pub async fn upsert_author_and_link(&self, paper_id: &str, name: &str, email: &str) -> Result<()> {
        self.graph.run(
            neo4rs::query(
                "MATCH (p:Paper {id:$pid}) MERGE (a:Author {email:$email}) SET a.name=$name MERGE (a)-[:AUTHORED]->(p)"
            )
            .param("pid", paper_id)
            .param("email", email)
            .param("name", name),
        ).await.map_err(|e| anyhow::anyhow!("Neo4j author_link: {e}"))?;
        Ok(())
    }

    // ── Expert ────────────────────────────────────────────────────────────

    pub async fn upsert_expert_and_link_paper(&self, paper_id: &str, name: &str, email: &str, bio: &str) -> Result<()> {
        self.graph.run(
            neo4rs::query(
                "MATCH (p:Paper {id:$pid}) MERGE (e:Expert {email:$email}) SET e.name=$name, e.bio=$bio MERGE (e)-[:EXPERT_FOR]->(p)"
            )
            .param("pid", paper_id)
            .param("email", email)
            .param("name", name)
            .param("bio", bio),
        ).await.map_err(|e| anyhow::anyhow!("Neo4j expert_link: {e}"))?;
        Ok(())
    }

    pub async fn link_expert_to_paper_concepts(&self, paper_id: &str, expert_email: &str) -> Result<()> {
        self.graph.run(
            neo4rs::query(
                "MATCH (p:Paper {id:$pid})-[:HAS_CONCEPT]->(c:Concept) \
                 MATCH (e:Expert {email:$email}) MERGE (e)-[:KNOWS]->(c)"
            )
            .param("pid", paper_id)
            .param("email", expert_email),
        ).await.map_err(|e| anyhow::anyhow!("Neo4j concept_sync: {e}"))?;
        Ok(())
    }

    // ── Ingestion ─────────────────────────────────────────────────────────

    pub async fn ingest_paper_extraction(&self, paper_id: &str, extraction: &PaperExtraction) -> Result<()> {
        for concept in &extraction.concepts {
            let category = format!("{:?}", concept.category);
            self.graph.run(
                neo4rs::query(
                    "MATCH (p:Paper {id:$pid}) MERGE (c:Concept {name:$name}) \
                     SET c.category=$cat, c.description=$desc MERGE (p)-[:HAS_CONCEPT]->(c)"
                )
                .param("pid", paper_id)
                .param("name", concept.name.as_str())
                .param("cat", category.as_str())
                .param("desc", concept.description.as_deref().unwrap_or("")),
            ).await.map_err(|e| anyhow::anyhow!("concept ingest: {e}"))?;
        }

        for topic in &extraction.topics {
            self.graph.run(
                neo4rs::query(
                    "MATCH (p:Paper {id:$pid}) MERGE (t:Topic {name:$name}) MERGE (p)-[:COVERS_TOPIC]->(t)"
                )
                .param("pid", paper_id)
                .param("name", topic.as_str()),
            ).await.map_err(|e| anyhow::anyhow!("topic ingest: {e}"))?;
        }

        for citation in &extraction.citations {
            self.graph.run(
                neo4rs::query(
                    "MATCH (p:Paper {id:$pid}) MERGE (c:Citation {title:$title}) \
                     SET c.year=$year MERGE (p)-[:CITES]->(c)"
                )
                .param("pid", paper_id)
                .param("title", citation.title.as_str())
                .param("year", citation.year.unwrap_or(0) as i64),
            ).await.map_err(|e| anyhow::anyhow!("citation ingest: {e}"))?;
        }
        Ok(())
    }

    // ── Expert Response ───────────────────────────────────────────────────

    pub async fn ingest_expert_response(&self, resp_id: &str, paper_id: &str,
                                         expert_email: &str, text: &str) -> Result<()> {
        self.graph.run(
            neo4rs::query(
                "MATCH (p:Paper {id:$pid}) MATCH (e:Expert {email:$email}) \
                 MERGE (r:ExpertResponse {id:$rid}) SET r.text=$text, r.created_at=datetime() \
                 MERGE (e)-[:PROVIDED]->(r) MERGE (r)-[:ABOUT]->(p)"
            )
            .param("pid", paper_id)
            .param("email", expert_email)
            .param("rid", resp_id)
            .param("text", text),
        ).await.map_err(|e| anyhow::anyhow!("expert_response ingest: {e}"))?;
        Ok(())
    }

    // ── Retrieval ─────────────────────────────────────────────────────────

    pub async fn find_authors_by_concept(&self, concepts: &[String]) -> Result<Vec<(String, String)>> {
        if concepts.is_empty() { return Ok(vec![]); }
        let mut results = Vec::new();
        for concept in concepts.iter().take(5) {
            let mut stream = self.graph.execute(
                neo4rs::query(
                    "MATCH (c:Concept {name:$name})<-[:HAS_CONCEPT]-(p:Paper)<-[:AUTHORED]-(a:Author) \
                     WHERE a.email IS NOT NULL AND a.email <> '' \
                     RETURN DISTINCT a.name AS name, a.email AS email LIMIT 5"
                ).param("name", concept.as_str()),
            ).await.map_err(|e| anyhow::anyhow!("find_authors: {e}"))?;

            while let Ok(Some(row)) = stream.next().await {
                let name: String = row.get("name").unwrap_or_default();
                let email: String = row.get("email").unwrap_or_default();
                if !email.is_empty() && !results.iter().any(|(_, e)| e == &email) {
                    results.push((name, email));
                }
            }
        }
        Ok(results)
    }

    pub async fn find_experts_by_concept(&self, concepts: &[String]) -> Result<Vec<(String, String)>> {
        if concepts.is_empty() { return Ok(vec![]); }
        let mut results = Vec::new();
        for concept in concepts.iter().take(5) {
            let mut stream = self.graph.execute(
                neo4rs::query(
                    "MATCH (c:Concept {name:$name})<-[:KNOWS]-(e:Expert) \
                     RETURN DISTINCT e.name AS name, e.email AS email LIMIT 5"
                ).param("name", concept.as_str()),
            ).await.map_err(|e| anyhow::anyhow!("find_experts: {e}"))?;

            while let Ok(Some(row)) = stream.next().await {
                let name: String = row.get("name").unwrap_or_default();
                let email: String = row.get("email").unwrap_or_default();
                if !email.is_empty() && !results.iter().any(|(_, e)| e == &email) {
                    results.push((name, email));
                }
            }
        }
        Ok(results)
    }

    pub async fn get_paper_concepts(&self, paper_id: &str) -> Result<Vec<String>> {
        let mut stream = self.graph.execute(
            neo4rs::query("MATCH (p:Paper {id:$pid})-[:HAS_CONCEPT]->(c:Concept) RETURN c.name AS name")
                .param("pid", paper_id),
        ).await.map_err(|e| anyhow::anyhow!("get_paper_concepts: {e}"))?;

        let mut concepts = Vec::new();
        while let Ok(Some(row)) = stream.next().await {
            if let Some(name) = row.get::<String>("name") {
                concepts.push(name);
            }
        }
        Ok(concepts)
    }

    pub async fn get_graph_context(&self, paper_ids: &[String]) -> Result<serde_json::Value> {
        if paper_ids.is_empty() { return Ok(serde_json::json!({"papers":[]})); }
        let mut papers = Vec::new();
        for pid in paper_ids {
            let mut stream = self.graph.execute(
                neo4rs::query(
                    "MATCH (p:Paper {id:$pid}) \
                     OPTIONAL MATCH (p)-[:HAS_CONCEPT]->(c:Concept) \
                     OPTIONAL MATCH (p)-[:COVERS_TOPIC]->(t:Topic) \
                     OPTIONAL MATCH (p)<-[:AUTHORED]-(a:Author) \
                     OPTIONAL MATCH (p)-[:CITES]->(cite:Citation) \
                     RETURN p.title AS title, \
                            collect(DISTINCT c.name) AS concepts, \
                            collect(DISTINCT t.name) AS topics, \
                            collect(DISTINCT a.name) AS authors, \
                            collect(DISTINCT cite.title) AS citations"
                ).param("pid", pid.as_str()),
            ).await.map_err(|e| anyhow::anyhow!("graph_context: {e}"))?;

            if let Ok(Some(row)) = stream.next().await {
                let title: String = row.get("title").unwrap_or_default();
                let concepts: Vec<String> = row.get("concepts").unwrap_or_default();
                let topics: Vec<String>   = row.get("topics").unwrap_or_default();
                let authors: Vec<String>  = row.get("authors").unwrap_or_default();
                let citations: Vec<String> = row.get("citations").unwrap_or_default();
                papers.push(serde_json::json!({
                    "paper_id": pid,
                    "title": title,
                    "concepts": concepts,
                    "topics": topics,
                    "authors": authors,
                    "citations": citations,
                }));
            }
        }
        Ok(serde_json::json!({ "papers": papers }))
    }
}