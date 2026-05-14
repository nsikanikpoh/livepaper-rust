use anyhow::{Context, Result};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub neo4j_uri: String,
    pub neo4j_user: String,
    pub neo4j_password: String,
    pub neo4j_db: String,        // leave empty to use the server default
    pub pinecone_api_key: String,
    pub pinecone_host: String,
    pub pinecone_index: String,
    pub pinecone_namespace: String,
    /// Optional: truncate embeddings to this many dimensions.
    /// Must match the dimension your Pinecone index was created with.
    /// Leave unset (or 0) to use the model's native output size.
    /// Example: set to 1024 if your index was created at 1024 dims.
    pub pinecone_dimensions: Option<u32>,
    pub openrouter_api_key: String,
    pub openrouter_base_url: String,
    pub llm_model: String,
    pub embedding_model: String,
    pub clerk_jwks_url: String,
    pub clerk_secret_key: String,
    pub sendgrid_api_key: String,
    pub email_from: String,
    pub email_from_name: String,
    /// Base URL of the frontend app — used in email portal links.
    pub app_base_url: String,
    pub langfuse_secret_key: String,
    pub langfuse_public_key: String,
    pub langfuse_host: String,
    pub confidence_threshold: f64,
    pub top_k_results: usize,
    pub max_expert_emails_per_query: usize,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        // Try candidate .env locations in priority order:
        //   1. Same dir as Cargo.toml  (backend/.env)
        //   2. One level up            (project_root/.env)  ← common monorepo layout
        //   3. Two levels up           (workspace_root/.env)
        //   4. cwd walk-up             (standard dotenvy fallback)
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let candidates = [
            manifest.join(".env"),
            manifest.parent().map(|p| p.join(".env")).unwrap_or_default(),
            manifest.parent()
                .and_then(|p| p.parent())
                .map(|p| p.join(".env"))
                .unwrap_or_default(),
        ];

        let mut loaded_from: Option<PathBuf> = None;
        for path in &candidates {
            if path.exists() {
                if dotenvy::from_path(path).is_ok() {
                    loaded_from = Some(path.clone());
                    break;
                }
            }
        }

        if loaded_from.is_none() {
            // Last resort: walk up from cwd
            if let Ok(path) = dotenvy::dotenv() {
                loaded_from = Some(path);
            }
        }

        match &loaded_from {
            Some(p) => eprintln!("[config] Loaded .env from: {}", p.display()),
            None    => eprintln!("[config] No .env found — using shell environment variables only"),
        }

        Ok(Self {
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "8080".into())
                .parse()
                .context("PORT must be a number")?,

            database_url: std::env::var("DATABASE_URL").context("DATABASE_URL required")?,

            neo4j_uri:      std::env::var("NEO4J_URI").context("NEO4J_URI required")?,
            neo4j_user:     std::env::var("NEO4J_USER").context("NEO4J_USER required")?,
            neo4j_password: std::env::var("NEO4J_PASSWORD").context("NEO4J_PASSWORD required")?,

            neo4j_db: std::env::var("NEO4J_DB").unwrap_or_default(), // default = server default,

            pinecone_api_key:  std::env::var("PINECONE_API_KEY").context("PINECONE_API_KEY required")?,
            pinecone_host:     std::env::var("PINECONE_HOST").context("PINECONE_HOST required")?,
            pinecone_index:    std::env::var("PINECONE_INDEX").unwrap_or_else(|_| "livepaper".into()),
            pinecone_namespace: std::env::var("PINECONE_NAMESPACE").unwrap_or_else(|_| "papers".into()),
            pinecone_dimensions: std::env::var("PINECONE_DIMENSIONS")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .filter(|&d| d > 0),

            openrouter_api_key:  std::env::var("OPENROUTER_API_KEY").context("OPENROUTER_API_KEY required")?,
            openrouter_base_url: std::env::var("OPENROUTER_BASE_URL")
                .unwrap_or_else(|_| "https://openrouter.ai/api/v1".into()),
            llm_model:       std::env::var("LLM_MODEL").unwrap_or_else(|_| "openai/gpt-4o-mini".into()),
            embedding_model: std::env::var("EMBEDDING_MODEL")
                .unwrap_or_else(|_| "openai/text-embedding-3-small".into()),

            clerk_jwks_url:  std::env::var("CLERK_JWKS_URL").context("CLERK_JWKS_URL required")?,
            clerk_secret_key: std::env::var("CLERK_SECRET_KEY").context("CLERK_SECRET_KEY required")?,

            email_from:       std::env::var("EMAIL_FROM").context("EMAIL_FROM required")?,
            sendgrid_api_key: std::env::var("SENDGRID_API_KEY").context("SENDGRID_API_KEY required")?,
            email_from_name: std::env::var("EMAIL_FROM_NAME").unwrap_or_else(|_| "LivePaper".into()),
            app_base_url: std::env::var("APP_BASE_URL").unwrap_or_else(|_| "https://livepaper.ai".into()),

            langfuse_secret_key: std::env::var("LANGFUSE_SECRET_KEY").unwrap_or_default(),
            langfuse_public_key: std::env::var("LANGFUSE_PUBLIC_KEY").unwrap_or_default(),
            langfuse_host: std::env::var("LANGFUSE_HOST")
                .unwrap_or_else(|_| "https://cloud.langfuse.com".into()),

            confidence_threshold: std::env::var("CONFIDENCE_THRESHOLD")
                .unwrap_or_else(|_| "0.72".into()).parse().unwrap_or(0.55), // cosine similarity; 0.55 catches gaps without over-escalating
            top_k_results: std::env::var("TOP_K_RESULTS")
                .unwrap_or_else(|_| "8".into()).parse().unwrap_or(8),
            max_expert_emails_per_query: std::env::var("MAX_EXPERT_EMAILS_PER_QUERY")
                .unwrap_or_else(|_| "3".into()).parse().unwrap_or(3),
        })
    }
}