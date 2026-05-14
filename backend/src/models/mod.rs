use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── User ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub clerk_id: String,
    pub email: String,
    pub name: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ─── Author ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Author {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
}

// ─── Paper ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Paper {
    pub id: Uuid,
    pub title: String,
    pub abstract_text: String,
    pub paper_url: String,
    pub pdf_url: Option<String>,
    pub user_id: Uuid,
    pub ingestion_status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperWithAuthors {
    #[serde(flatten)]
    pub paper: Paper,
    pub authors: Vec<Author>,
}

// ─── Expert ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Expert {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub bio: String,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpertWithPapers {
    #[serde(flatten)]
    pub expert: Expert,
    pub papers: Vec<Paper>,
}

// ─── Expert Response ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExpertResponse {
    pub id: Uuid,
    pub paper_id: Uuid,
    pub expert_email: String,
    pub response: String,
    pub embedded: bool,
    pub created_at: DateTime<Utc>,
}

// ─── Chat ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChatSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChatMessage {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: String,
    pub content: String,
    pub sources: Option<serde_json::Value>,
    pub escalated: bool,
    pub created_at: DateTime<Utc>,
}

// ─── Escalation ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EscalationEvent {
    pub id: Uuid,
    pub session_id: Uuid,
    pub message_id: Uuid,
    pub question: String,
    pub confidence: f64,
    pub experts_notified: Vec<String>,
    pub status: String,
    pub langfuse_trace_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ─── Knowledge Graph ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    pub name: String,
    pub category: ConceptCategory,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConceptCategory {
    Method,
    Finding,
    Topic,
    Citation,
    TechnicalConcept,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperExtraction {
    pub concepts: Vec<Concept>,
    pub citations: Vec<CitationRef>,
    pub key_findings: Vec<String>,
    pub methods: Vec<String>,
    pub topics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationRef {
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
}

// ─── Vector types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorMatch {
    pub id: String,
    pub score: f64,
    pub metadata: VectorMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorMetadata {
    pub paper_id: String,
    pub paper_title: String,
    pub chunk_type: String,
    pub text: String,
    pub expert_email: Option<String>,
}

// ─── Agent types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RetrievalResult {
    pub matches: Vec<VectorMatch>,
    pub top_confidence: f64,
    pub expanded_query: String,
}

#[derive(Debug, Clone)]
pub struct AgentContext {
    pub trace_id: String,
    pub session_id: Uuid,
    pub user_id: Uuid,
}
