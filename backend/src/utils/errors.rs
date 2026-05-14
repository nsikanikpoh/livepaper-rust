use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Neo4j error: {0}")]
    Neo4j(String),
    #[error("Pinecone error: {0}")]
    Pinecone(String),
    #[error("LLM error: {0}")]
    Llm(String),
    #[error("Email error: {0}")]
    Email(String),
    #[error("PDF processing error: {0}")]
    PdfProcessing(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(m)       => (StatusCode::NOT_FOUND, m.clone()),
            AppError::Unauthorized(m)   => (StatusCode::UNAUTHORIZED, m.clone()),
            AppError::Forbidden(m)      => (StatusCode::FORBIDDEN, m.clone()),
            AppError::Validation(m)     => (StatusCode::UNPROCESSABLE_ENTITY, m.clone()),
            AppError::Database(m)       => { tracing::error!("DB: {m}"); (StatusCode::INTERNAL_SERVER_ERROR, "Database error".into()) }
            AppError::Neo4j(m)          => { tracing::error!("Neo4j: {m}"); (StatusCode::INTERNAL_SERVER_ERROR, "Graph database error".into()) }
            AppError::Pinecone(m)       => { tracing::error!("Pinecone: {m}"); (StatusCode::INTERNAL_SERVER_ERROR, "Vector database error".into()) }
            AppError::Llm(m)            => { tracing::error!("LLM: {m}"); (StatusCode::INTERNAL_SERVER_ERROR, "AI service error".into()) }
            AppError::Email(m)          => { tracing::error!("Email: {m}"); (StatusCode::INTERNAL_SERVER_ERROR, "Email service error".into()) }
            AppError::PdfProcessing(m)  => (StatusCode::UNPROCESSABLE_ENTITY, format!("PDF error: {m}")),
            AppError::Internal(m)       => { tracing::error!("Internal: {m}"); (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".into()) }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

// Convenience conversions
impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self { AppError::Database(e.to_string()) }
}
impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self { AppError::Internal(e.to_string()) }
}

pub type AppResult<T> = Result<T, AppError>;
