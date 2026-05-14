use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

pub struct LlmService {
    client: Client,
    api_key: String,
    base_url: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct CompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChatMessageRaw,
}

#[derive(Debug, Deserialize)]
struct ChatMessageRaw {
    content: String,
}

impl LlmService {
    pub fn new(api_key: String, base_url: String, model: String) -> Self {
        Self { client: Client::new(), api_key, base_url, model }
    }

    pub async fn complete(&self, system: &str, user: &str) -> Result<String> {
        self.chat(system, vec![ChatMessage { role: "user".into(), content: user.into() }]).await
    }

    pub async fn chat(&self, system: &str, messages: Vec<ChatMessage>) -> Result<String> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let mut msgs = vec![json!({"role":"system","content":system})];
        msgs.extend(messages.iter().map(|m| json!({"role":m.role,"content":m.content})));

        let resp = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("HTTP-Referer", "https://livepaper.ai")
            .json(&json!({"model":self.model,"messages":msgs,"temperature":0.2,"max_tokens":2048}))
            .send().await?;

        if !resp.status().is_success() {
            let t = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("LLM error: {t}"));
        }
        let cr: CompletionResponse = resp.json().await?;
        Ok(cr.choices.first().map(|c| c.message.content.clone()).unwrap_or_default())
    }

    pub async fn extract_json(&self, system: &str, user: &str) -> Result<String> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let resp = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("HTTP-Referer", "https://livepaper.ai")
            .json(&json!({
                "model": self.model,
                "messages": [{"role":"system","content":system},{"role":"user","content":user}],
                "temperature": 0.0,
                "max_tokens": 3000,
                "response_format": {"type":"json_object"}
            }))
            .send().await?;

        if !resp.status().is_success() {
            let t = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("LLM JSON error: {t}"));
        }
        let cr: CompletionResponse = resp.json().await?;
        Ok(cr.choices.first().map(|c| c.message.content.clone()).unwrap_or_default())
    }
}
