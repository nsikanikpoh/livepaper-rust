use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    AppState,
    models::{PaperExtraction, Concept, ConceptCategory, CitationRef},
    services::pdf::PdfService,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionAuthorInput {
    pub name: String,
    pub email: String,
}

pub struct IngestionAgent {
    state: Arc<AppState>,
    pdf_service: PdfService,
}

impl IngestionAgent {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state, pdf_service: PdfService::new() }
    }

    pub async fn ingest(
        &self,
        paper_id: Uuid,
        title: &str,
        abstract_text: &str,
        paper_url: &str,
        pdf_url: Option<&str>,
        authors: &[IngestionAuthorInput],
        trace_id: &str,
    ) -> Result<()> {
        let pid = paper_id.to_string();
        tracing::info!("[IngestionAgent] Starting for paper {pid}");

        self.state.postgres.update_paper_status(paper_id, "processing").await?;

        // Neo4j paper node
        self.state.neo4j.upsert_paper(&pid, title, paper_url, abstract_text).await?;

        // Authors — postgres + neo4j
        for a in authors {
            if a.email.is_empty() { continue; }
            let pg_author = self.state.postgres.upsert_author(&a.name, &a.email).await?;
            self.state.postgres.link_paper_author(paper_id, pg_author.id).await?;
            self.state.neo4j.upsert_author_and_link(&pid, &a.name, &a.email).await?;
        }

        // Download + extract PDF text
        let full_text = if let Some(url) = pdf_url {
            match self.pdf_service.download_and_extract(url).await {
                Ok(t) => { tracing::info!("[IngestionAgent] PDF: {} chars", t.len()); t }
                Err(e) => { tracing::warn!("[IngestionAgent] PDF failed: {e}, using abstract"); abstract_text.to_string() }
            }
        } else {
            abstract_text.to_string()
        };

        // LLM knowledge extraction
        let extraction = self.extract_knowledge(title, abstract_text, &full_text, trace_id).await
            .unwrap_or_else(|e| {
                tracing::warn!("[IngestionAgent] LLM extraction failed: {e}");
                PaperExtraction { concepts: vec![], citations: vec![], key_findings: vec![], methods: vec![], topics: vec![] }
            });

        // Write to Neo4j
        self.state.neo4j.ingest_paper_extraction(&pid, &extraction).await?;

        self.state.langfuse.log_span(
            trace_id, "ingestion_extraction",
            json!({"paper_id": pid}),
            json!({"concepts": extraction.concepts.len(), "topics": extraction.topics.len()}),
            None,
        ).await.ok();

        // Embed + store in Pinecone
        self.embed_and_store(paper_id, title, abstract_text, &full_text).await?;

        self.state.postgres.update_paper_status(paper_id, "completed").await?;
        tracing::info!("[IngestionAgent] Done for paper {pid}");
        Ok(())
    }

    async fn extract_knowledge(&self, title: &str, abstract_text: &str, full_text: &str, trace_id: &str) -> Result<PaperExtraction> {
        let truncated = if full_text.len() > 8000 { &full_text[..8000] } else { full_text };
        let system = r#"You are a research knowledge extractor. Extract structured knowledge.
Return ONLY valid JSON:
{"concepts":[{"name":"string","category":"Method|Finding|Topic|Citation|TechnicalConcept","description":"string"}],
 "citations":[{"title":"string","authors":["string"],"year":0}],
 "key_findings":["string"],"methods":["string"],"topics":["string"]}"#;
        let user = format!("Title: {title}\n\nAbstract: {abstract_text}\n\nText excerpt:\n{truncated}");
        let raw = self.state.llm.extract_json(system, &user).await?;
        self.state.langfuse.log_generation(trace_id, "paper_extraction", &self.state.config.llm_model, &user, &raw).await.ok();
        self.parse_extraction(&raw)
    }

    fn parse_extraction(&self, raw: &str) -> Result<PaperExtraction> {
        let v: serde_json::Value = serde_json::from_str(raw)
            .map_err(|e| anyhow::anyhow!("Parse extraction JSON: {e}"))?;

        let concepts = v["concepts"].as_array().unwrap_or(&vec![]).iter().filter_map(|c| {
            let name = c["name"].as_str()?.to_string();
            let cat = match c["category"].as_str().unwrap_or("TechnicalConcept") {
                "Method"   => ConceptCategory::Method,
                "Finding"  => ConceptCategory::Finding,
                "Topic"    => ConceptCategory::Topic,
                "Citation" => ConceptCategory::Citation,
                _          => ConceptCategory::TechnicalConcept,
            };
            Some(Concept { name, category: cat, description: c["description"].as_str().map(String::from) })
        }).collect();

        let citations = v["citations"].as_array().unwrap_or(&vec![]).iter().filter_map(|c| {
            Some(CitationRef {
                title: c["title"].as_str()?.to_string(),
                authors: c["authors"].as_array().unwrap_or(&vec![])
                    .iter().filter_map(|a| a.as_str().map(String::from)).collect(),
                year: c["year"].as_i64().map(|y| y as i32),
            })
        }).collect();

        let str_arr = |key: &str| -> Vec<String> {
            v[key].as_array().unwrap_or(&vec![])
                .iter().filter_map(|x| x.as_str().map(String::from)).collect()
        };

        Ok(PaperExtraction {
            concepts,
            citations,
            key_findings: str_arr("key_findings"),
            methods: str_arr("methods"),
            topics: str_arr("topics"),
        })
    }

    async fn embed_and_store(&self, paper_id: Uuid, title: &str, abstract_text: &str, full_text: &str) -> Result<()> {
        let pid = paper_id.to_string();

        // Pinecone rejects null metadata values — omit expert_email entirely
        // for paper vectors; it is only present on expert-response vectors.
        let paper_meta = |chunk_type: &str, text: &str| json!({
            "paper_id":    pid,
            "paper_title": title,
            "chunk_type":  chunk_type,
            "text":        text,
        });

        // Abstract embedding
        let emb = self.state.embedding.embed(abstract_text).await?;
        self.state.pinecone.upsert(
            &format!("{pid}_abstract"),
            emb,
            paper_meta("abstract", abstract_text),
        ).await?;

        // Body chunks (cap at 20)
        let chunks = PdfService::chunk_text(full_text, 300, 50);
        for (i, chunk) in chunks.iter().take(20).enumerate() {
            match self.state.embedding.embed(chunk).await {
                Ok(emb) => {
                    self.state.pinecone.upsert(
                        &format!("{pid}_chunk_{i}"),
                        emb,
                        paper_meta("body", chunk),
                    ).await.ok();
                }
                Err(e) => tracing::warn!("chunk {i} embed failed: {e}"),
            }
        }
        tracing::info!("[IngestionAgent] Vectors stored for {pid}");
        Ok(())
    }
}