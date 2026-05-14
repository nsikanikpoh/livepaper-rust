use axum::{Extension, extract::{Path, State}, Json};
use serde::{Deserialize};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;
use tokio::task;

use crate::{
    AppState,
    agents::ingestion::{IngestionAgent, IngestionAuthorInput},
    middleware::auth::AuthUser,
    models::PaperWithAuthors,
    utils::errors::{AppError, AppResult},
};

#[derive(Debug, Deserialize)]
pub struct CreatePaperRequest {
    pub title: String,
    pub abstract_text: String,
    pub paper_url: String,
    pub pdf_url: Option<String>,
    pub authors: Vec<AuthorInput>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthorInput {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePaperRequest {
    pub title: Option<String>,
    pub abstract_text: Option<String>,
    pub paper_url: Option<String>,
    pub pdf_url: Option<String>,
}

pub async fn list_papers(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
) -> AppResult<Json<Vec<PaperWithAuthors>>> {
    let papers = state.postgres.get_papers_with_authors(auth_user.user_id)
        .await.map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(papers))
}

pub async fn get_paper(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PaperWithAuthors>> {
    println!("get_paper: fetching paper {}", id);
    let paper = state.postgres.get_paper(id).await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Paper {id} not found")))?;
    println!("get_paper: found paper with title {:?}", paper);

    let authors = state.postgres.get_paper_authors(id)
        .await.map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(PaperWithAuthors { paper, authors }))
}

pub async fn create_paper(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Json(req): Json<CreatePaperRequest>,
) -> AppResult<Json<PaperWithAuthors>> {
    if req.title.trim().is_empty() {
        return Err(AppError::Validation("title is required".into()));
    }

    let paper = state.postgres.create_paper(
        &req.title, &req.abstract_text, &req.paper_url,
        req.pdf_url.as_deref(), auth_user.user_id,
    ).await.map_err(|e| AppError::Database(e.to_string()))?;

    // Optimistic author linking (also done inside ingestion task)
    let mut authors = Vec::new();
    for a in &req.authors {
        if let Ok(author) = state.postgres.upsert_author(&a.name, &a.email).await {
            state.postgres.link_paper_author(paper.id, author.id).await.ok();
            authors.push(author);
        }
    }

    // Spawn background ingestion
    let state2 = Arc::new(state.clone());
    let pid = paper.id;
    let title = req.title.clone();
    let abs   = req.abstract_text.clone();
    let purl  = req.paper_url.clone();
    let pdf   = req.pdf_url.clone();
    let ing_authors: Vec<IngestionAuthorInput> = req.authors.iter()
        .map(|a| IngestionAuthorInput { name: a.name.clone(), email: a.email.clone() })
        .collect();

    task::spawn(async move {
        let trace_id = state2.langfuse.create_trace("ingestion", &pid.to_string(), "", json!({}))
            .await.unwrap_or_else(|_| Uuid::new_v4().to_string());
        let agent = IngestionAgent::new(state2.clone());
        if let Err(e) = agent.ingest(pid, &title, &abs, &purl, pdf.as_deref(), &ing_authors, &trace_id).await {
            tracing::error!("[create_paper] ingestion failed for {pid}: {e}");
            state2.postgres.update_paper_status(pid, "failed").await.ok();
        }
    });

    Ok(Json(PaperWithAuthors { paper, authors }))
}

pub async fn update_paper(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePaperRequest>,
) -> AppResult<Json<PaperWithAuthors>> {
    let existing = state.postgres.get_paper(id).await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Paper {id} not found")))?;
    if existing.user_id != auth_user.user_id {
        return Err(AppError::Forbidden("Not your paper".into()));
    }
    let paper = state.postgres.update_paper(id, req.title.as_deref(), req.abstract_text.as_deref(),
        req.paper_url.as_deref(), req.pdf_url.as_deref())
        .await.map_err(|e| AppError::Database(e.to_string()))?;
    let authors = state.postgres.get_paper_authors(id)
        .await.map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(PaperWithAuthors { paper, authors }))
}

pub async fn delete_paper(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let existing = state.postgres.get_paper(id).await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Paper {id} not found")))?;
    if existing.user_id != auth_user.user_id {
        return Err(AppError::Forbidden("Not your paper".into()));
    }
    state.pinecone.delete_by_paper(&id.to_string()).await.ok();
    state.postgres.delete_paper(id).await.map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(json!({"deleted": true, "id": id})))
}
