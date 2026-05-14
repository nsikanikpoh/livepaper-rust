use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::models::{VectorMatch, VectorMetadata};

pub struct PineconeClient {
    client: Client,
    api_key: String,
    host: String,
    index: String,
    namespace: String,
}

#[derive(Debug, Serialize)]
struct UpsertRequest {
    vectors: Vec<PineconeVector>,
    namespace: String,
}

#[derive(Debug, Serialize)]
struct PineconeVector {
    id: String,
    values: Vec<f32>,
    metadata: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct QueryRequest {
    vector: Vec<f32>,
    top_k: u32,
    namespace: String,
    include_metadata: bool,
}

#[derive(Debug, Deserialize)]
struct QueryResponse {
    matches: Vec<PineconeMatch>,
}

#[derive(Debug, Deserialize)]
struct PineconeMatch {
    id: String,
    score: f64,
    metadata: Option<serde_json::Value>,
}

impl PineconeClient {
    pub fn new(api_key: String, host: String, index: String, namespace: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            host,
            index,
            namespace,
        }
    }

    fn base_url(&self) -> String {
        self.host.trim_end_matches('/').to_string()
    }

    /// Upsert a vector with metadata into Pinecone
    pub async fn upsert(
        &self,
        id: &str,
        embedding: Vec<f32>,
        metadata: serde_json::Value,
    ) -> Result<()> {
        let url = format!("{}/vectors/upsert", self.base_url());
        let body = UpsertRequest {
            vectors: vec![PineconeVector {
                id: id.to_string(),
                values: embedding,
                metadata,
            }],
            namespace: self.namespace.clone(),
        };

        let resp = self.client
            .post(&url)
            .header("Api-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Pinecone upsert failed: {}", text));
        }
        Ok(())
    }

    /// Query top-k similar vectors
    pub async fn query(&self, embedding: Vec<f32>, top_k: u32) -> Result<Vec<VectorMatch>> {
        let url = format!("{}/query", self.base_url());
        let body = QueryRequest {
            vector: embedding,
            top_k,
            namespace: self.namespace.clone(),
            include_metadata: true,
        };

        let resp = self.client
            .post(&url)
            .header("Api-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Pinecone query failed: {}", text));
        }

        let query_resp: QueryResponse = resp.json().await?;
        let matches = query_resp.matches
            .into_iter()
            .map(|m| {
                let meta = m.metadata.unwrap_or_default();
                VectorMatch {
                    id: m.id,
                    score: m.score,
                    metadata: VectorMetadata {
                        paper_id: meta.get("paper_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        paper_title: meta.get("paper_title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        chunk_type: meta.get("chunk_type").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        text: meta.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        expert_email: meta.get("expert_email").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    },
                }
            })
            .collect();

        Ok(matches)
    }

    /// Delete vectors by paper id prefix (for cleanup)
    pub async fn delete_by_paper(&self, paper_id: &str) -> Result<()> {
        let url = format!("{}/vectors/delete", self.base_url());
        let body = json!({
            "filter": { "paper_id": { "$eq": paper_id } },
            "namespace": self.namespace
        });

        let resp = self.client
            .post(&url)
            .header("Api-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            tracing::warn!("Pinecone delete failed: {}", text);
        }
        Ok(())
    }
}
