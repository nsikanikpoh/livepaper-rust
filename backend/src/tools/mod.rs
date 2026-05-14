use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    AppState,
    models::{VectorMatch},
};

// ─── Tool Results ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailToolResult {
    pub sent_to: Vec<String>,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult {
    pub matches: Vec<VectorMatch>,
    pub top_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQueryResult {
    pub experts: Vec<(String, String)>,  // (name, email)
    pub authors: Vec<(String, String)>,
    pub concepts: Vec<String>,
}

// ─── Send Email Tool ──────────────────────────────────────────────────────────

pub struct SendEmailTool {
    state: Arc<AppState>,
}

impl SendEmailTool {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub async fn execute(
        &self,
        to_name: &str,
        to_email: &str,
        paper_title: &str,
        question: &str,
        paper_id: &str,
    ) -> Result<EmailToolResult> {
        tracing::info!("SendEmailTool: sending to {} <{}>", to_name, to_email);

        match self.state.email.send_expert_invitation(
            to_name,
            to_email,
            paper_title,
            question,
            paper_id,
        ).await {
            Ok(_) => Ok(EmailToolResult {
                sent_to: vec![to_email.to_string()],
                success: true,
            }),
            Err(e) => {
                tracing::error!("Email send failed to {}: {}", to_email, e);
                Ok(EmailToolResult {
                    sent_to: vec![],
                    success: false,
                })
            }
        }
    }
}

// ─── Vector Search Tool ───────────────────────────────────────────────────────

pub struct VectorSearchTool {
    state: Arc<AppState>,
}

impl VectorSearchTool {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub async fn execute(&self, query_embedding: Vec<f32>, top_k: u32) -> Result<VectorSearchResult> {
        let matches = self.state.pinecone.query(query_embedding, top_k).await?;
        let top_score = matches.first().map(|m| m.score).unwrap_or(0.0);
        Ok(VectorSearchResult { matches, top_score })
    }
}

// ─── Graph Query Tool ─────────────────────────────────────────────────────────

pub struct GraphQueryTool {
    state: Arc<AppState>,
}

impl GraphQueryTool {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub async fn find_experts_and_authors(&self, concepts: &[String]) -> Result<GraphQueryResult> {
        let experts = self.state.neo4j.find_experts_by_concept(concepts).await
            .unwrap_or_default();
        let authors = self.state.neo4j.find_authors_by_concept(concepts).await
            .unwrap_or_default();

        Ok(GraphQueryResult {
            experts,
            authors,
            concepts: concepts.to_vec(),
        })
    }
}
