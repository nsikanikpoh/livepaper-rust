//! # LivePaper — Axum Backend Entry Point
//!
//! Bootstraps the full application in this order:
//!   1. Load environment variables from `.env`
//!   2. Initialise structured JSON tracing
//!   3. Parse and validate all config from env
//!   4. Connect to PostgreSQL, run DDL migrations
//!   5. Connect to Neo4j, create uniqueness constraints
//!   6. Initialise the Pinecone HTTP client
//!   7. Initialise LLM, embedding, email, and LangFuse services
//!   8. Assemble shared `AppState` (all deps wrapped in `Arc`)
//!   9. Build the Axum router with middleware and start serving
//!
//! ## Module layout
//!
//! ```text
//! src/
//! ├── main.rs          ← you are here
//! ├── models/          ← domain types (Paper, Expert, ChatMessage, …)
//! ├── db/
//! │   ├── postgres.rs  ← PostgreSQL CRUD + DDL migrations (sqlx)
//! │   ├── neo4j.rs     ← GraphRAG node/relationship operations (neo4rs)
//! │   └── pinecone.rs  ← Vector upsert / semantic query (REST)
//! ├── services/
//! │   ├── llm.rs       ← OpenRouter chat + JSON-mode extraction
//! │   ├── embedding.rs ← Text embeddings via OpenRouter
//! │   ├── email.rs     ← SMTP via lettre (rustls, no native-tls)
//! │   ├── langfuse.rs  ← Observability: traces, spans, generations
//! │   └── pdf.rs       ← PDF download, text extraction, chunking
//! ├── agents/
//! │   ├── ingestion.rs         ← PDF → LLM extraction → Neo4j + Pinecone
//! │   ├── retrieval.rs         ← Query expand → embed → vector search → graph enrich
//! │   ├── gap_detector.rs      ← Confidence threshold + LLM gap verification
//! │   ├── expert_router.rs     ← Neo4j multi-hop expert lookup → email
//! │   ├── chat.rs              ← Orchestrates the full chat pipeline
//! │   └── response_ingestion.rs← Expert reply → embed → Pinecone + Neo4j
//! ├── handlers/
//! │   ├── chat.rs    ← POST /chat
//! │   ├── papers.rs  ← GET|POST|PUT|DELETE /papers[/:id]
//! │   └── experts.rs ← GET|POST /experts, POST /experts/response
//! ├── middleware/
//! │   └── auth.rs    ← Clerk JWT verification (JWKS), user upsert
//! ├── tools/         ← Agent tools: SendEmail, VectorSearch, GraphQuery
//! └── utils/
//!     ├── config.rs  ← Typed config loaded from environment variables
//!     └── errors.rs  ← AppError → HTTP response mapping
//! ```

mod agents;
mod db;
mod handlers;
mod middleware;
mod models;
mod services;
mod tools;
mod utils;

use axum::{
    Router,
    http::{Method, header},
    middleware as axum_middleware,
};
use std::sync::Arc;
use std::time::Duration;
use tower_http::{
    cors::{CorsLayer, Any},
    trace::TraceLayer,
    timeout::TimeoutLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    db::{postgres::PostgresDb, neo4j::Neo4jDb, pinecone::PineconeClient},
    services::{
        embedding::EmbeddingService,
        email::EmailService,
        langfuse::LangfuseService,
        llm::LlmService,
    },
};

// ── Shared application state ──────────────────────────────────────────────────
//
// `AppState` is cloned cheaply into every request handler via Axum's `State`
// extractor. All fields are `Arc<T>` so cloning is just an atomic ref-count
// increment — the underlying resources are shared across the thread pool.

/// All long-lived dependencies injected into every handler and agent.
#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL connection pool — users, papers, experts, chat history.
    pub postgres: Arc<PostgresDb>,
    /// Neo4j graph client — knowledge graph nodes and relationships.
    pub neo4j: Arc<Neo4jDb>,
    /// Pinecone HTTP client — vector upsert and semantic search.
    pub pinecone: Arc<PineconeClient>,
    /// OpenRouter LLM service — chat completions and JSON extraction.
    pub llm: Arc<LlmService>,
    /// SMTP email service — expert invitation and escalation emails.
    pub email: Arc<EmailService>,
    /// LangFuse observability — traces, spans, and generation logs.
    pub langfuse: Arc<LangfuseService>,
    /// Embedding service — converts text to vectors for Pinecone.
    pub embedding: Arc<EmbeddingService>,
    /// Parsed, validated configuration from environment variables.
    pub config: Arc<utils::config::Config>,
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── 1. Load .env ──────────────────────────────────────────────────────────
    //
    // Three strategies are tried in order so the binary works correctly
    // regardless of the working directory it is launched from:
    //
    //   Strategy 1 — `CARGO_MANIFEST_DIR` (compile-time absolute path).
    //     Resolves to the directory that contains `Cargo.toml`. This always
    //     works with `cargo run` even when invoked from a parent directory.
    //
    //   Strategy 2 — cwd walk-up (standard dotenvy behaviour).
    //     Searches the current directory and each parent in turn. Covers the
    //     case where the compiled binary is placed alongside the `.env` file.
    //
    //   Strategy 3 — next to the compiled binary itself.
    //     Fallback for deployments where the binary and `.env` live together
    //     but the process is started from an unrelated working directory.
    //
    // Variables already present in the shell environment are NOT overwritten
    // by any of these strategies (dotenvy's default behaviour), so container
    // / CI environments that inject secrets via real env vars take precedence.

    let manifest_env = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    let s1 = dotenvy::from_path(&manifest_env).is_ok();

    let s2 = !s1 && dotenvy::dotenv().is_ok();

    if !s1 && !s2 {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                dotenvy::from_path(dir.join(".env")).ok();
            }
        }
    }

    eprintln!("[livepaper] .env loaded: manifest_dir={s1} cwd_walk={s2}");

    // ── 2. Structured JSON tracing ────────────────────────────────────────────
    //
    // Log level is controlled by the `RUST_LOG` environment variable.
    // Default: `livepaper=debug,tower_http=debug` (verbose in development).
    // Production recommendation: `livepaper=info,tower_http=warn`.
    //
    // Output is newline-delimited JSON — ready for log aggregators such as
    // Datadog, CloudWatch, or the ELK stack without any additional parsing.

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "livepaper=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    // ── 3. Configuration ──────────────────────────────────────────────────────
    //
    // `Config::from_env()` also runs a second dotenvy pass internally, which
    // guarantees that vars are present even if `from_env` is ever called from
    // a test or helper that bypasses the loading above.
    // Fails fast with a descriptive error if any required variable is missing.

    let config = Arc::new(utils::config::Config::from_env()?);
    tracing::info!("Configuration loaded");

    // ── 4. PostgreSQL ─────────────────────────────────────────────────────────
    //
    // `run_migrations` executes a series of idempotent `CREATE TABLE IF NOT
    // EXISTS` and `CREATE INDEX IF NOT EXISTS` statements. Each is a separate
    // prepared-statement call (PostgreSQL rejects multi-command strings over
    // the prepared-statement wire protocol). Safe to run on every startup.

    let postgres = Arc::new(PostgresDb::new(&config.database_url).await?);
    postgres.run_migrations().await?;
    tracing::info!("PostgreSQL connected and migrations applied");

    // ── 5. Neo4j ──────────────────────────────────────────────────────────────
    //
    // `ensure_constraints` creates uniqueness constraints on Paper, Author,
    // Expert, and Concept nodes. These are essential for the `MERGE` pattern
    // used throughout the ingestion pipeline — without them, concurrent writes
    // could produce duplicate nodes.
    //
    // `NEO4J_DB` is optional. When blank (the default), neo4rs connects to the
    // server's default database. Specifying it explicitly is only needed on
    // Neo4j Enterprise multi-database setups; on Community Edition leaving it
    // blank avoids `Neo.ClientError.Database.DatabaseNotFound`.

    let neo4j = Arc::new(
        Neo4jDb::new(
            &config.neo4j_uri,
            &config.neo4j_user,
            &config.neo4j_password,
            &config.neo4j_db,
        )
        .await?,
    );
    neo4j.ensure_constraints().await?;
    tracing::info!("Neo4j connected and constraints ensured");

    // ── 6. Pinecone ───────────────────────────────────────────────────────────
    //
    // Thin HTTP client; no connection to establish at startup. The index and
    // namespace are created in the Pinecone console before running the server.
    // Embedding dimension must match the model used by `EmbeddingService`
    // (default: `text-embedding-3-small` → 1536 dims).

    let pinecone = Arc::new(PineconeClient::new(
        config.pinecone_api_key.clone(),
        config.pinecone_host.clone(),
        config.pinecone_index.clone(),
        config.pinecone_namespace.clone(),
    ));
    tracing::info!("Pinecone client initialised");

    // ── 7. Services ───────────────────────────────────────────────────────────
    //
    // All services are stateless HTTP clients. They share the same OpenRouter
    // API key; the embedding and LLM models can be configured separately via
    // `EMBEDDING_MODEL` and `LLM_MODEL` in the environment.

    // Text embedding — converts queries and paper chunks to vectors.
    let embedding = Arc::new(EmbeddingService::new(
        config.openrouter_api_key.clone(),
        config.openrouter_base_url.clone(),
        config.embedding_model.clone(),
        config.pinecone_dimensions,  // None = use model native dims; Some(n) = truncate to n
    ));

    // Chat completions and JSON-mode extraction (gpt-4o-mini via OpenRouter).
    let llm = Arc::new(LlmService::new(
        config.openrouter_api_key.clone(),
        config.openrouter_base_url.clone(),
        config.llm_model.clone(),
    ));

    // SMTP email — uses rustls exclusively; native-tls is excluded to avoid a
    // transitive dependency chain that requires Rust edition 2024.
    let email = Arc::new(EmailService::new(&config)?);

    // LangFuse — if secret/public keys are blank, all calls are no-ops so the
    // server runs without observability rather than failing to start.
    let langfuse = Arc::new(LangfuseService::new(
        config.langfuse_secret_key.clone(),
        config.langfuse_public_key.clone(),
        config.langfuse_host.clone(),
    ));

    // ── 8. Assemble shared state ──────────────────────────────────────────────

    let state = AppState {
        postgres,
        neo4j,
        pinecone,
        llm,
        email,
        langfuse,
        embedding,
        config,
    };

    // ── 9. Build router and serve ─────────────────────────────────────────────

    let app = build_router(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("LivePaper listening on http://{addr}");

    axum::serve(listener, app).await?;
    Ok(())
}

// ── Router construction ───────────────────────────────────────────────────────

/// Assembles the full Axum router with middleware layers applied.
///
/// Layer order matters in Axum — layers are applied bottom-up (innermost
/// first). The effective order at request time is:
///
///   CorsLayer → TimeoutLayer → TraceLayer → auth_middleware → handler
///
/// CORS and tracing run before auth so that pre-flight OPTIONS requests and
/// request logs work even for unauthenticated calls.
fn build_router(state: AppState) -> Router {
    // CORS — allow all origins in development. Restrict `allow_origin` in
    // production to your actual frontend domain.
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        .allow_origin(Any);

    // Protected routes — require a valid Clerk JWT.
    let protected = Router::new()
        .nest("/chat",    chat_routes())
        .nest("/papers",  paper_mutation_routes())
        .nest("/experts", expert_routes())
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::auth::auth_middleware,
        ));

    // Public routes — no auth required.
    // GET /papers and GET /papers/:id are intentionally open so the
    // expert-response page (and any future public embed) can fetch paper
    // metadata without a Clerk session.
    let public = Router::new()
        .nest("/papers", paper_read_routes())
        .nest("/experts", expert_public_routes());

    Router::new()
        .merge(protected)
        .merge(public)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        // Hard timeout — prevents runaway LLM or PDF-download calls from
        // holding connections indefinitely. The ingestion agent runs in a
        // spawned task so it is not subject to this limit.
        .layer(TimeoutLayer::new(Duration::from_secs(120)))
        .with_state(state)
}

// ── Route definitions ─────────────────────────────────────────────────────────
//
// Each function declares its routes independently so `build_router` stays
// readable. Axum's `Router<S>` generic keeps the state type threaded through
// without needing to pass it explicitly to every sub-router.

/// `POST /chat`
///
/// Accepts `{ message, session_id? }`. Runs the full agent pipeline:
/// retrieval → gap detection → answer generation → optional escalation.
/// Returns the assistant reply plus source citations and a LangFuse trace id.
fn chat_routes() -> Router<AppState> {
    use axum::routing::post;
    Router::new().route("/", post(handlers::chat::chat_handler))
}

/// Public read-only paper routes — no authentication required.
/// `GET /papers`     — list all papers (with authors)
/// `GET /papers/:id` — get a single paper with authors
fn paper_read_routes() -> Router<AppState> {
    use axum::routing::get;
    Router::new()
        .route("/:id", get(handlers::papers::get_paper))
}

/// Protected paper mutation routes — require a valid Clerk JWT.
/// `POST   /papers`     — create a paper and trigger background ingestion
/// `PUT    /papers/:id` — update paper metadata
/// `DELETE /papers/:id` — delete paper and remove its Pinecone vectors
fn paper_mutation_routes() -> Router<AppState> {
    use axum::routing::{put};
    Router::new()
     .route("/", axum::routing::post(handlers::papers::create_paper)
                                .get(handlers::papers::list_papers))
        .route(
            "/:id",
            put(handlers::papers::update_paper)
                .delete(handlers::papers::delete_paper),
        )
}

/// `GET  /experts`          — list all experts with their associated papers
/// `POST /experts`          — invite an expert: creates DB record, links to paper
///                            concepts in Neo4j, sends invitation email

fn expert_routes() -> Router<AppState> {
    use axum::routing::get;
    Router::new()
        .route("/", get(handlers::experts::list_experts).post(handlers::experts::invite_expert))
    }

/// `POST /experts/response` — submit an expert response; embeds it into
///                            Pinecone and writes an ExpertResponse node to Neo4j
///                            so future identical questions are answered instantly 
    fn expert_public_routes() -> Router<AppState> {
        use axum::routing::post;
        Router::new()
        .route("/response", post(handlers::experts::submit_expert_response))
}