use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

pub struct ResponseIngestionAgent {
    state: Arc<AppState>,
}

impl ResponseIngestionAgent {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Full pipeline when an expert submits a response:
    /// 1. Save response to PG
    /// 2. Embed the response
    /// 3. Store in Pinecone attributed to expert + paper
    /// 4. Write ExpertResponse node to Neo4j linked to Paper
    /// 5. Mark as embedded
    pub async fn ingest(
        &self,
        paper_id: Uuid,
        expert_email: &str,
        response_text: &str,
    ) -> Result<()> {
        tracing::info!(
            "[ResponseIngestionAgent] Ingesting expert response from {} for paper {}",
            expert_email,
            paper_id
        );

        // 1. Save to PG
        let expert_response = self.state.postgres
            .create_expert_response(paper_id, expert_email, response_text)
            .await?;

        // 2. Get paper for title (for metadata)
        let paper = self.state.postgres.get_paper(paper_id).await?
            .ok_or_else(|| anyhow::anyhow!("Paper {} not found", paper_id))?;

        // 3. Embed response
        let embedding = self.state.embedding.embed(response_text).await?;

        // 4. Store in Pinecone
        let vector_id = format!("expert_resp_{}", expert_response.id);
        let metadata = json!({
            "paper_id": paper_id.to_string(),
            "paper_title": paper.title,
            "chunk_type": "expert_response",
            "text": response_text,
            "expert_email": expert_email,
        });

        self.state.pinecone.upsert(&vector_id, embedding, metadata).await
            .map_err(|e| anyhow::anyhow!("Pinecone upsert failed: {}", e))?;

        // 5. Write to Neo4j
        self.state.neo4j.ingest_expert_response(
            &expert_response.id.to_string(),
            &paper_id.to_string(),
            expert_email,
            response_text,
        ).await?;

        // 6. Mark as embedded
        self.state.postgres.mark_response_embedded(expert_response.id).await?;

        tracing::info!(
            "[ResponseIngestionAgent] Expert response {} ingested successfully",
            expert_response.id
        );

        Ok(())
    }
}
