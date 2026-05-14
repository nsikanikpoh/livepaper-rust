use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

pub struct EmbeddingService {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    /// If Some(n), the returned vector is truncated to n dimensions on our
    /// side after the API responds. This is necessary because OpenRouter does
    /// not forward the `dimensions` parameter to the upstream OpenAI API, so
    /// we always receive the model's full native output (1536 for
    /// text-embedding-3-small) and truncate it ourselves to match the
    /// Pinecone index size.
    ///
    /// Truncation preserves semantic quality for text-embedding-3-* models
    /// because they are trained with Matryoshka Representation Learning —
    /// the first N dimensions of any truncation still form a meaningful
    /// embedding. Set via `PINECONE_DIMENSIONS` in the environment.
    dimensions: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

impl EmbeddingService {
    pub fn new(api_key: String, base_url: String, model: String, dimensions: Option<u32>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url,
            model,
            dimensions: dimensions.map(|d| d as usize),
        }
    }

    /// Truncate a vector to `self.dimensions` if configured.
    fn maybe_truncate(&self, mut v: Vec<f32>) -> Vec<f32> {
        if let Some(n) = self.dimensions {
            v.truncate(n);
        }
        v
    }

    /// Embed a single text and return the (possibly truncated) vector.
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/embeddings", self.base_url.trim_end_matches('/'));

        let body = json!({
            "model": self.model,
            "input": text,
        });

        let resp = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Embedding API error: {}", text));
        }

        let emb_resp: EmbeddingResponse = resp.json().await?;
        let vec = emb_resp.data.into_iter().next()
            .map(|d| d.embedding)
            .unwrap_or_default();

        Ok(self.maybe_truncate(vec))
    }

    /// Embed multiple texts in a single API call, truncating each vector.
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let url = format!("{}/embeddings", self.base_url.trim_end_matches('/'));

        let body = json!({
            "model": self.model,
            "input": texts,
        });

        let resp = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Embedding batch API error: {}", text));
        }

        let emb_resp: EmbeddingResponse = resp.json().await?;
        Ok(emb_resp.data
            .into_iter()
            .map(|d| self.maybe_truncate(d.embedding))
            .collect())
    }
}