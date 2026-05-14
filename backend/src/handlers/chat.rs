use axum::{Extension, extract::State, Json};
use serde::{Deserialize, Serialize};

use std::sync::Arc;
use uuid::Uuid;

use crate::{
    AppState,
    agents::chat::ChatAgent,
    middleware::auth::AuthUser,
    services::llm::ChatMessage,
    utils::errors::{AppError, AppResult},
};

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub session_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub session_id: Uuid,
    pub message: String,
    pub sources: serde_json::Value,
    pub escalated: bool,
    pub escalation_note: Option<String>,
    pub trace_id: String,
}

pub async fn chat_handler(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Json(req): Json<ChatRequest>,
) -> AppResult<Json<ChatResponse>> {
    if req.message.trim().is_empty() {
        return Err(AppError::Validation("Message cannot be empty".into()));
    }

    let session_id = req.session_id.unwrap_or_else(Uuid::new_v4);

    state.postgres.get_or_create_session(session_id, auth_user.user_id)
        .await.map_err(|e| AppError::Database(e.to_string()))?;

    // Load the last 10 messages (5 user + 5 assistant turns) in chronological order.
    // Fetched before saving the current user message so it reflects prior context only.
    let history_rows = state.postgres
        .get_recent_session_messages(session_id, 10)
        .await.map_err(|e| AppError::Database(e.to_string()))?;

    let history: Vec<ChatMessage> = history_rows.iter()
        .filter(|m| m.role != "system")
        .map(|m| ChatMessage { role: m.role.clone(), content: m.content.clone() })
        .collect();

    // Save user turn
    state.postgres.add_chat_message(session_id, "user", &req.message, None, false)
        .await.map_err(|e| AppError::Database(e.to_string()))?;

    // Run agent
    let agent = ChatAgent::new(Arc::new(state.clone()));
    let result = agent.process(&req.message, session_id, auth_user.user_id, &history)
        .await.map_err(|e| AppError::Llm(e.to_string()))?;

    // Save assistant turn
    let assistant_msg = state.postgres
        .add_chat_message(session_id, "assistant", &result.content, Some(result.sources.clone()), result.escalated)
        .await.map_err(|e| AppError::Database(e.to_string()))?;

    // Log escalation if occurred
    if result.escalated && !result.notified_experts.is_empty() {
        state.postgres.create_escalation(
            session_id,
            assistant_msg.id,
            &req.message,
            result.sources["confidence"].as_f64().unwrap_or(0.0),
            &result.notified_experts,
            Some(&result.trace_id),
        ).await.ok();
    }

    Ok(Json(ChatResponse {
        session_id,
        message: result.content,
        sources: result.sources,
        escalated: result.escalated,
        escalation_note: result.escalation_message,
        trace_id: result.trace_id,
    }))
}