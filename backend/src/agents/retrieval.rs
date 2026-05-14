use anyhow::Result;
use serde_json::json;
use std::sync::Arc;

use crate::{AppState, models::{RetrievalResult, VectorMatch}};

pub struct RetrievalAgent {
    state: Arc<AppState>,
}

impl RetrievalAgent {
    pub fn new(state: Arc<AppState>) -> Self { Self { state } }

    pub async fn retrieve(&self, question: &str, top_k: usize, trace_id: &str) -> Result<RetrievalResult> {
        // 1. Expand query
        let expanded = self.expand_query(question, trace_id).await
            .unwrap_or_else(|_| question.to_string());

        self.state.langfuse.log_span(trace_id, "query_expansion",
            json!({"original": question}), json!({"expanded": expanded}), None).await.ok();

        // 2. Embed
        let emb = self.state.embedding.embed(&expanded).await?;

        // 3. Vector search
        let matches = self.state.pinecone.query(emb, top_k as u32).await.unwrap_or_default();
        let top_confidence = matches.first().map(|m| m.score).unwrap_or(0.0);

        self.state.langfuse.log_span(trace_id, "vector_search",
            json!({"query": expanded}),
            json!({"matches": matches.len(), "top_confidence": top_confidence}), None).await.ok();

        Ok(RetrievalResult { matches, top_confidence, expanded_query: expanded })
    }

    pub async fn get_graph_context(&self, matches: &[VectorMatch]) -> Result<serde_json::Value> {
        let paper_ids: Vec<String> = matches.iter()
            .map(|m| m.metadata.paper_id.clone())
            .filter(|id| !id.is_empty())
            .collect::<std::collections::HashSet<_>>()
            .into_iter().collect();
        self.state.neo4j.get_graph_context(&paper_ids).await
    }

    async fn expand_query(&self, question: &str, trace_id: &str) -> Result<String> {
        let system = "You are a research query expansion assistant. \
            Expand the user's question with related concepts, synonyms, and technical terms. \
            Return ONLY the expanded query text, nothing else.";
        let expanded = self.state.llm.complete(system, question).await?;
        self.state.langfuse.log_generation(
            trace_id, "query_expansion_llm",
            &self.state.config.llm_model, question, &expanded).await.ok();
        Ok(expanded.trim().to_string())
    }

    pub fn build_context_string(matches: &[VectorMatch], graph_ctx: &serde_json::Value) -> String {
        let mut parts: Vec<String> = matches.iter().take(5).enumerate().map(|(i, m)| {
            let src = if m.metadata.chunk_type == "expert_response" {
                format!("Expert Response ({})", m.metadata.expert_email.as_deref().unwrap_or("unknown"))
            } else {
                format!("Paper: {}", m.metadata.paper_title)
            };
            format!("[Source {}] {}\n{}", i + 1, src, m.metadata.text)
        }).collect();

        if let Some(papers) = graph_ctx.get("papers").and_then(|p| p.as_array()) {
            for p in papers.iter().take(3) {
                let title    = p["title"].as_str().unwrap_or("");
                let concepts = p["concepts"].as_array().map(|c|
                    c.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", ")
                ).unwrap_or_default();
                let authors  = p["authors"].as_array().map(|a|
                    a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", ")
                ).unwrap_or_default();
                if !concepts.is_empty() {
                    parts.push(format!("[Graph] '{title}' by {authors} — concepts: {concepts}"));
                }
            }
        }
        parts.join("\n\n")
    }
}
