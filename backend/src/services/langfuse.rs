use anyhow::Result;
use reqwest::Client;
use serde_json::{json, Value};
use uuid::Uuid;
use chrono::Utc;
use base64::{engine::general_purpose::STANDARD, Engine};

pub struct LangfuseService {
    client: Client,
    secret_key: String,
    public_key: String,
    host: String,
}

impl LangfuseService {
    pub fn new(secret_key: String, public_key: String, host: String) -> Self {
        Self {
            client: Client::new(),
            secret_key,
            public_key,
            host,
        }
    }

    fn auth_header(&self) -> String {
        let creds = format!("{}:{}", self.public_key, self.secret_key);
        format!("Basic {}", STANDARD.encode(creds))
    }

    fn is_configured(&self) -> bool {
        !self.secret_key.is_empty() && !self.public_key.is_empty()
    }

    /// Create a new trace for a user session
    pub async fn create_trace(
        &self,
        name: &str,
        session_id: &str,
        user_id: &str,
        input: Value,
    ) -> Result<String> {
        if !self.is_configured() {
            return Ok(Uuid::new_v4().to_string());
        }

        let trace_id = Uuid::new_v4().to_string();
        let url = format!("{}/api/public/traces", self.host.trim_end_matches('/'));

        let body = json!({
            "id": trace_id,
            "name": name,
            "sessionId": session_id,
            "userId": user_id,
            "input": input,
            "timestamp": Utc::now().to_rfc3339(),
        });

        let resp = self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        if let Err(e) = resp {
            tracing::warn!("LangFuse trace creation failed: {}", e);
        }

        Ok(trace_id)
    }

    /// Log a span (agent step) within a trace
    pub async fn log_span(
        &self,
        trace_id: &str,
        name: &str,
        input: Value,
        output: Value,
        metadata: Option<Value>,
    ) -> Result<()> {
        if !self.is_configured() {
            return Ok(());
        }

        let url = format!("{}/api/public/spans", self.host.trim_end_matches('/'));

        let body = json!({
            "traceId": trace_id,
            "name": name,
            "input": input,
            "output": output,
            "metadata": metadata,
            "startTime": Utc::now().to_rfc3339(),
            "endTime": Utc::now().to_rfc3339(),
        });

        let resp = self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        if let Err(e) = resp {
            tracing::warn!("LangFuse span log failed: {}", e);
        }

        Ok(())
    }

    /// Log a generation (LLM call) within a trace
    pub async fn log_generation(
        &self,
        trace_id: &str,
        name: &str,
        model: &str,
        prompt: &str,
        completion: &str,
    ) -> Result<()> {
        if !self.is_configured() {
            return Ok(());
        }

        let url = format!("{}/api/public/generations", self.host.trim_end_matches('/'));

        let body = json!({
            "traceId": trace_id,
            "name": name,
            "model": model,
            "prompt": prompt,
            "completion": completion,
            "startTime": Utc::now().to_rfc3339(),
            "endTime": Utc::now().to_rfc3339(),
        });

        let resp = self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        if let Err(e) = resp {
            tracing::warn!("LangFuse generation log failed: {}", e);
        }

        Ok(())
    }

    /// Log escalation event with full trace
    pub async fn log_escalation(
        &self,
        trace_id: &str,
        question: &str,
        confidence: f64,
        experts_contacted: &[String],
    ) -> Result<()> {
        self.log_span(
            trace_id,
            "gap_detector_escalation",
            json!({ "question": question, "confidence": confidence }),
            json!({ "escalated": true, "experts_contacted": experts_contacted }),
            Some(json!({ "event_type": "escalation", "confidence_below_threshold": true })),
        ).await
    }
}
