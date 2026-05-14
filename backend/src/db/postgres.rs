use anyhow::Result;
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

use crate::models::*;

pub struct PostgresDb {
    pub pool: PgPool,
}

impl PostgresDb {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        // Each sqlx::query() call must contain exactly ONE statement —
        // prepared statements do not allow semicolon-separated multi-commands.
        sqlx::query(CREATE_USERS).execute(&self.pool).await?;
        sqlx::query(CREATE_AUTHORS).execute(&self.pool).await?;
        sqlx::query(CREATE_PAPERS).execute(&self.pool).await?;
        sqlx::query(CREATE_PAPERS_IDX).execute(&self.pool).await?;
        sqlx::query(CREATE_PAPER_AUTHORS).execute(&self.pool).await?;
        sqlx::query(CREATE_EXPERTS).execute(&self.pool).await?;
        sqlx::query(CREATE_EXPERT_PAPERS).execute(&self.pool).await?;
        sqlx::query(CREATE_EXPERT_RESPONSES).execute(&self.pool).await?;
        sqlx::query(CREATE_EXPERT_RESPONSES_IDX).execute(&self.pool).await?;
        sqlx::query(CREATE_CHAT_SESSIONS).execute(&self.pool).await?;
        sqlx::query(CREATE_CHAT_MESSAGES).execute(&self.pool).await?;
        sqlx::query(CREATE_CHAT_MESSAGES_IDX).execute(&self.pool).await?;
        sqlx::query(CREATE_ESCALATIONS).execute(&self.pool).await?;
        tracing::info!("Database migrations completed");
        Ok(())
    }

    // ── Users ──────────────────────────────────────────────────────────────

    pub async fn upsert_user(&self, clerk_id: &str, email: &str, name: &str, role: &str) -> Result<User> {
        let user = sqlx::query_as::<_, User>(
            r#"INSERT INTO users (id, clerk_id, email, name, role, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
               ON CONFLICT (clerk_id) DO UPDATE
                   SET email = EXCLUDED.email, name = EXCLUDED.name, updated_at = NOW()
               RETURNING *"#,
        )
        .bind(Uuid::new_v4()).bind(clerk_id).bind(email).bind(name).bind(role)
        .fetch_one(&self.pool).await?;
        Ok(user)
    }

    pub async fn get_user_by_clerk_id(&self, clerk_id: &str) -> Result<Option<User>> {
        Ok(sqlx::query_as::<_, User>("SELECT * FROM users WHERE clerk_id = $1")
            .bind(clerk_id).fetch_optional(&self.pool).await?)
    }

    // ── Papers ─────────────────────────────────────────────────────────────

    pub async fn create_paper(&self, title: &str, abstract_text: &str, paper_url: &str,
                               pdf_url: Option<&str>, user_id: Uuid) -> Result<Paper> {
        Ok(sqlx::query_as::<_, Paper>(
            r#"INSERT INTO papers (id, title, abstract_text, paper_url, pdf_url, user_id,
                                   ingestion_status, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,'pending',NOW(),NOW()) RETURNING *"#,
        )
        .bind(Uuid::new_v4()).bind(title).bind(abstract_text)
        .bind(paper_url).bind(pdf_url).bind(user_id)
        .fetch_one(&self.pool).await?)
    }

    pub async fn get_paper(&self, id: Uuid) -> Result<Option<Paper>> {
        Ok(sqlx::query_as::<_, Paper>("SELECT * FROM papers WHERE id = $1")
            .bind(id).fetch_optional(&self.pool).await?)
    }

    pub async fn list_papers(&self, user_id: Uuid) -> Result<Vec<Paper>> {
        Ok(sqlx::query_as::<_, Paper>(
            "SELECT * FROM papers WHERE user_id = $1 ORDER BY created_at DESC",
        ).bind(user_id).fetch_all(&self.pool).await?)
    }

    pub async fn update_paper_status(&self, id: Uuid, status: &str) -> Result<()> {
        sqlx::query("UPDATE papers SET ingestion_status = $1, updated_at = NOW() WHERE id = $2")
            .bind(status).bind(id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn update_paper(&self, id: Uuid, title: Option<&str>, abstract_text: Option<&str>,
                               paper_url: Option<&str>, pdf_url: Option<&str>) -> Result<Paper> {
        Ok(sqlx::query_as::<_, Paper>(
            r#"UPDATE papers SET
               title = COALESCE($2, title),
               abstract_text = COALESCE($3, abstract_text),
               paper_url = COALESCE($4, paper_url),
               pdf_url = COALESCE($5, pdf_url),
               updated_at = NOW()
               WHERE id = $1 RETURNING *"#,
        )
        .bind(id).bind(title).bind(abstract_text).bind(paper_url).bind(pdf_url)
        .fetch_one(&self.pool).await?)
    }

    pub async fn delete_paper(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM papers WHERE id = $1").bind(id).execute(&self.pool).await?;
        Ok(())
    }

    // ── Authors ────────────────────────────────────────────────────────────

    pub async fn upsert_author(&self, name: &str, email: &str) -> Result<Author> {
        Ok(sqlx::query_as::<_, Author>(
            r#"INSERT INTO authors (id, name, email, created_at) VALUES ($1,$2,$3,NOW())
               ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name RETURNING *"#,
        ).bind(Uuid::new_v4()).bind(name).bind(email).fetch_one(&self.pool).await?)
    }

    pub async fn link_paper_author(&self, paper_id: Uuid, author_id: Uuid) -> Result<()> {
        sqlx::query(
            "INSERT INTO paper_authors (paper_id, author_id) VALUES ($1,$2) ON CONFLICT DO NOTHING",
        ).bind(paper_id).bind(author_id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_paper_authors(&self, paper_id: Uuid) -> Result<Vec<Author>> {
        Ok(sqlx::query_as::<_, Author>(
            r#"SELECT a.* FROM authors a
               JOIN paper_authors pa ON pa.author_id = a.id
               WHERE pa.paper_id = $1"#,
        ).bind(paper_id).fetch_all(&self.pool).await?)
    }

    pub async fn get_papers_with_authors(&self, user_id: Uuid) -> Result<Vec<PaperWithAuthors>> {
        let papers = self.list_papers(user_id).await?;
        let mut result = Vec::new();
        for paper in papers {
            let authors = self.get_paper_authors(paper.id).await?;
            result.push(PaperWithAuthors { paper, authors });
        }
        Ok(result)
    }

    // ── Experts ────────────────────────────────────────────────────────────

    pub async fn upsert_expert(&self, name: &str, email: &str, bio: &str, user_id: Uuid) -> Result<Expert> {
        Ok(sqlx::query_as::<_, Expert>(
            r#"INSERT INTO experts (id, name, email, bio, user_id, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,NOW(),NOW())
               ON CONFLICT (email) DO UPDATE SET name=EXCLUDED.name, bio=EXCLUDED.bio, updated_at=NOW()
               RETURNING *"#,
        ).bind(Uuid::new_v4()).bind(name).bind(email).bind(bio).bind(user_id)
        .fetch_one(&self.pool).await?)
    }

    pub async fn get_expert_by_email(&self, email: &str) -> Result<Option<Expert>> {
        Ok(sqlx::query_as::<_, Expert>("SELECT * FROM experts WHERE email = $1")
            .bind(email).fetch_optional(&self.pool).await?)
    }

    pub async fn list_experts(&self) -> Result<Vec<Expert>> {
        Ok(sqlx::query_as::<_, Expert>("SELECT * FROM experts ORDER BY created_at DESC")
            .fetch_all(&self.pool).await?)
    }

    pub async fn link_expert_paper(&self, expert_id: Uuid, paper_id: Uuid) -> Result<()> {
        sqlx::query(
            "INSERT INTO expert_papers (expert_id, paper_id) VALUES ($1,$2) ON CONFLICT DO NOTHING",
        ).bind(expert_id).bind(paper_id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_expert_papers(&self, expert_id: Uuid) -> Result<Vec<Paper>> {
        Ok(sqlx::query_as::<_, Paper>(
            r#"SELECT p.* FROM papers p
               JOIN expert_papers ep ON ep.paper_id = p.id
               WHERE ep.expert_id = $1"#,
        ).bind(expert_id).fetch_all(&self.pool).await?)
    }

    pub async fn get_experts_with_papers(&self) -> Result<Vec<ExpertWithPapers>> {
        let experts = self.list_experts().await?;
        let mut result = Vec::new();
        for expert in experts {
            let papers = self.get_expert_papers(expert.id).await?;
            result.push(ExpertWithPapers { expert, papers });
        }
        Ok(result)
    }

    // ── Expert Responses ───────────────────────────────────────────────────

    pub async fn create_expert_response(&self, paper_id: Uuid, expert_email: &str,
                                         response: &str) -> Result<ExpertResponse> {
        Ok(sqlx::query_as::<_, ExpertResponse>(
            r#"INSERT INTO expert_responses (id, paper_id, expert_email, response, embedded, created_at)
               VALUES ($1,$2,$3,$4,false,NOW()) RETURNING *"#,
        ).bind(Uuid::new_v4()).bind(paper_id).bind(expert_email).bind(response)
        .fetch_one(&self.pool).await?)
    }

    pub async fn mark_response_embedded(&self, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE expert_responses SET embedded = true WHERE id = $1")
            .bind(id).execute(&self.pool).await?;
        Ok(())
    }

    // ── Chat ───────────────────────────────────────────────────────────────

    pub async fn get_or_create_session(&self, session_id: Uuid, user_id: Uuid) -> Result<ChatSession> {
        Ok(sqlx::query_as::<_, ChatSession>(
            r#"INSERT INTO chat_sessions (id, user_id, created_at, updated_at)
               VALUES ($1,$2,NOW(),NOW())
               ON CONFLICT (id) DO UPDATE SET updated_at = NOW()
               RETURNING *"#,
        ).bind(session_id).bind(user_id).fetch_one(&self.pool).await?)
    }

    pub async fn add_chat_message(&self, session_id: Uuid, role: &str, content: &str,
                                   sources: Option<serde_json::Value>, escalated: bool) -> Result<ChatMessage> {
        Ok(sqlx::query_as::<_, ChatMessage>(
            r#"INSERT INTO chat_messages (id, session_id, role, content, sources, escalated, created_at)
               VALUES ($1,$2,$3,$4,$5,$6,NOW()) RETURNING *"#,
        ).bind(Uuid::new_v4()).bind(session_id).bind(role).bind(content)
        .bind(sources).bind(escalated).fetch_one(&self.pool).await?)
    }

    pub async fn get_session_messages(&self, session_id: Uuid) -> Result<Vec<ChatMessage>> {
        Ok(sqlx::query_as::<_, ChatMessage>(
            "SELECT * FROM chat_messages WHERE session_id = $1 ORDER BY created_at ASC",
        ).bind(session_id).fetch_all(&self.pool).await?)
    }

    /// Fetch the last `limit` messages for a session in chronological order.
    /// Uses a subquery so the DB does the windowing — no full table scan in memory.
    pub async fn get_recent_session_messages(&self, session_id: Uuid, limit: i64) -> Result<Vec<ChatMessage>> {
        Ok(sqlx::query_as::<_, ChatMessage>(
            r#"SELECT * FROM (
                   SELECT * FROM chat_messages
                   WHERE session_id = $1
                   ORDER BY created_at DESC
                   LIMIT $2
               ) sub
               ORDER BY sub.created_at ASC"#,
        ).bind(session_id).bind(limit).fetch_all(&self.pool).await?)
    }

    // ── Escalations ────────────────────────────────────────────────────────

    pub async fn create_escalation(&self, session_id: Uuid, message_id: Uuid, question: &str,
                                    confidence: f64, experts_notified: &[String],
                                    langfuse_trace_id: Option<&str>) -> Result<EscalationEvent> {
        Ok(sqlx::query_as::<_, EscalationEvent>(
            r#"INSERT INTO escalation_events
               (id, session_id, message_id, question, confidence, experts_notified, status, langfuse_trace_id, created_at)
               VALUES ($1,$2,$3,$4,$5,$6,'pending',$7,NOW()) RETURNING *"#,
        )
        .bind(Uuid::new_v4()).bind(session_id).bind(message_id).bind(question)
        .bind(confidence).bind(experts_notified).bind(langfuse_trace_id)
        .fetch_one(&self.pool).await?)
    }
}

// ─── DDL ─────────────────────────────────────────────────────────────────────

// ─── DDL — one statement per constant, one execute() per call ────────────────

const CREATE_USERS: &str = "CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    clerk_id TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'researcher',
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
)";

const CREATE_AUTHORS: &str = "CREATE TABLE IF NOT EXISTS authors (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL
)";

const CREATE_PAPERS: &str = "CREATE TABLE IF NOT EXISTS papers (
    id UUID PRIMARY KEY,
    title TEXT NOT NULL,
    abstract_text TEXT NOT NULL DEFAULT '',
    paper_url TEXT NOT NULL,
    pdf_url TEXT,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ingestion_status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
)";

const CREATE_PAPERS_IDX: &str =
    "CREATE INDEX IF NOT EXISTS papers_user_id_idx ON papers(user_id)";

const CREATE_PAPER_AUTHORS: &str = "CREATE TABLE IF NOT EXISTS paper_authors (
    paper_id UUID NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    author_id UUID NOT NULL REFERENCES authors(id) ON DELETE CASCADE,
    PRIMARY KEY (paper_id, author_id)
)";

const CREATE_EXPERTS: &str = "CREATE TABLE IF NOT EXISTS experts (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE,
    bio TEXT NOT NULL DEFAULT '',
    user_id UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
)";

const CREATE_EXPERT_PAPERS: &str = "CREATE TABLE IF NOT EXISTS expert_papers (
    expert_id UUID NOT NULL REFERENCES experts(id) ON DELETE CASCADE,
    paper_id UUID NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    PRIMARY KEY (expert_id, paper_id)
)";

const CREATE_EXPERT_RESPONSES: &str = "CREATE TABLE IF NOT EXISTS expert_responses (
    id UUID PRIMARY KEY,
    paper_id UUID NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    expert_email TEXT NOT NULL,
    response TEXT NOT NULL,
    embedded BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL
)";

const CREATE_EXPERT_RESPONSES_IDX: &str =
    "CREATE INDEX IF NOT EXISTS er_paper_id_idx ON expert_responses(paper_id)";

const CREATE_CHAT_SESSIONS: &str = "CREATE TABLE IF NOT EXISTS chat_sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
)";

const CREATE_CHAT_MESSAGES: &str = "CREATE TABLE IF NOT EXISTS chat_messages (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    sources JSONB,
    escalated BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL
)";

const CREATE_CHAT_MESSAGES_IDX: &str =
    "CREATE INDEX IF NOT EXISTS cm_session_id_idx ON chat_messages(session_id)";

const CREATE_ESCALATIONS: &str = "CREATE TABLE IF NOT EXISTS escalation_events (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL,
    message_id UUID NOT NULL,
    question TEXT NOT NULL,
    confidence DOUBLE PRECISION NOT NULL,
    experts_notified TEXT[] NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'pending',
    langfuse_trace_id TEXT,
    created_at TIMESTAMPTZ NOT NULL
)";