use axum::{Extension, extract::State, Json};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    AppState,
    agents::response_ingestion::ResponseIngestionAgent,
    middleware::auth::AuthUser,
    models::ExpertWithPapers,
    utils::errors::{AppError, AppResult},
};

#[derive(Debug, Deserialize)]
pub struct InviteExpertRequest {
    pub email: String,
    pub name: String,
    pub bio: String,
    pub paper_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct ExpertResponseRequest {
    pub paper_id: Uuid,
    pub expert_email: String,
    pub response: String,
}

pub async fn list_experts(
    State(state): State<AppState>,
    Extension(_auth): Extension<AuthUser>,
) -> AppResult<Json<Vec<ExpertWithPapers>>> {
    let experts = state.postgres.get_experts_with_papers()
        .await.map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(experts))
}

pub async fn invite_expert(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Json(req): Json<InviteExpertRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if req.email.trim().is_empty() || req.name.trim().is_empty() {
        return Err(AppError::Validation("email and name are required".into()));
    }

    let paper = state.postgres.get_paper(req.paper_id).await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Paper {} not found", req.paper_id)))?;
    if paper.user_id != auth_user.user_id {
        return Err(AppError::Forbidden("Not your paper".into()));
    }

    let expert = state.postgres.upsert_expert(&req.name, &req.email, &req.bio, auth_user.user_id)
        .await.map_err(|e| AppError::Database(e.to_string()))?;

    state.postgres.link_expert_paper(expert.id, req.paper_id)
        .await.map_err(|e| AppError::Database(e.to_string()))?;

    let pid_str = req.paper_id.to_string();
    state.neo4j.upsert_expert_and_link_paper(&pid_str, &req.name, &req.email, &req.bio)
        .await.map_err(|e| AppError::Neo4j(e.to_string()))?;
    state.neo4j.link_expert_to_paper_concepts(&pid_str, &req.email)
        .await.map_err(|e| AppError::Neo4j(e.to_string()))?;

    state.email.send_expert_invitation_email(&req.name, &req.email, &paper.title, &pid_str)
        .await.map_err(|e| AppError::Email(e.to_string()))?;

    Ok(Json(json!({
        "expert": {"id": expert.id, "name": expert.name, "email": expert.email},
        "paper_id": req.paper_id,
        "invited": true,
    })))
}

pub async fn submit_expert_response(
    State(state): State<AppState>,
    Json(req): Json<ExpertResponseRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if req.response.trim().is_empty() {
        return Err(AppError::Validation("response cannot be empty".into()));
    }

    state.postgres.get_paper(req.paper_id).await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Paper {} not found", req.paper_id)))?;

    state.postgres.get_expert_by_email(&req.expert_email).await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Expert {} not found", req.expert_email)))?;

    let agent = ResponseIngestionAgent::new(Arc::new(state));
    agent.ingest(req.paper_id, &req.expert_email, &req.response)
        .await.map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "message": "Expert response ingested into knowledge base."
    })))
}
