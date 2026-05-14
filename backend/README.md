# LivePaper — Rust/Axum Backend

Turn static research papers into live, queryable knowledge with GraphRAG, agentic escalation, and expert-in-the-loop learning.

## Architecture

```
                    ┌─────────────────────────────────────────────────┐
                    │                  Axum HTTP Server                │
                    │  POST /chat   GET|POST|PUT|DELETE /papers        │
                    │  GET|POST /experts    POST /experts/response     │
                    └─────────────────────┬───────────────────────────┘
                                          │ Clerk JWT auth middleware
                          ┌───────────────▼───────────────────┐
                          │           Agent Orchestration      │
                          │                                    │
                          │  ┌──────────────────────────────┐ │
                          │  │     Chat Agent               │ │
                          │  │  1. RetrievalAgent           │ │
                          │  │     ├─ Query expansion (LLM) │ │
                          │  │     ├─ Embed (OpenRouter)    │ │
                          │  │     ├─ Pinecone top-k        │ │
                          │  │     └─ Neo4j graph enrich    │ │
                          │  │  2. GapDetectorAgent         │ │
                          │  │     ├─ Confidence threshold  │ │
                          │  │     └─ LLM gap verification  │ │
                          │  │  3. ExpertRouterAgent        │ │  ─── escalation ──▶ Email
                          │  │     ├─ Neo4j multi-hop query │ │
                          │  │     └─ SendEmailTool         │ │
                          │  │  4. Answer generation (LLM)  │ │
                          │  └──────────────────────────────┘ │
                          │                                    │
                          │  ┌────────────────────────────┐   │
                          │  │   IngestionAgent           │   │  (background task)
                          │  │   PDF → LLM → Neo4j        │   │
                          │  │        └────→ Pinecone      │   │
                          │  └────────────────────────────┘   │
                          │                                    │
                          │  ┌────────────────────────────┐   │
                          │  │  ResponseIngestionAgent    │   │
                          │  │  Expert reply → embed      │   │
                          │  │  → Pinecone + Neo4j update │   │
                          │  └────────────────────────────┘   │
                          └───────────────────────────────────┘
                                          │
               ┌──────────────────────────┼──────────────────────┐
               │                          │                       │
        ┌──────▼──────┐          ┌────────▼───────┐      ┌───────▼──────┐
        │  PostgreSQL  │          │     Neo4j      │      │   Pinecone   │
        │  Users       │          │  Paper nodes   │      │  Embeddings  │
        │  Papers      │          │  Author nodes  │      │  Abstracts   │
        │  Experts     │          │  Expert nodes  │      │  Body chunks │
        │  Sessions    │          │  Concept nodes │      │  Expert resp │
        │  Escalations │          │  Multi-hop rels│      │              │
        └─────────────┘          └────────────────┘      └──────────────┘
```

## Quick Start

```bash
# 1. Clone and configure
cp .env.example .env
# Fill in: DATABASE_URL, NEO4J_*, PINECONE_*, OPENROUTER_API_KEY, CLERK_*, SMTP_*

# 2. Start databases
docker compose up postgres neo4j -d

# 3. Run the server
cargo run

# Or full stack
docker compose up
```

## API Reference

### Authentication
All endpoints require a Clerk JWT in `Authorization: Bearer <token>`.

### Chat
```http
POST /chat
{"message": "What is the core contribution of this paper?", "session_id": "uuid-optional"}
```

### Papers
```http
GET    /papers             # list (with authors)
POST   /papers             # create + trigger ingestion
GET    /papers/:id         # get (with authors)
PUT    /papers/:id         # update metadata
DELETE /papers/:id         # delete + clean vectors
```

**Create paper body:**
```json
{
  "title": "Attention Is All You Need",
  "abstract_text": "We propose a new ...",
  "paper_url": "https://arxiv.org/abs/1706.03762",
  "pdf_url": "https://arxiv.org/pdf/1706.03762",
  "authors": [{"name": "Ashish Vaswani", "email": "vaswani@google.com"}]
}
```

### Experts
```http
GET  /experts              # list with associated papers
POST /experts              # invite expert to paper
POST /experts/response     # submit expert response
```

**Invite expert:**
```json
{"email": "expert@uni.edu", "name": "Dr. Smith", "bio": "...", "paper_id": "uuid"}
```

**Expert response (updates knowledge base instantly):**
```json
{"paper_id": "uuid", "expert_email": "expert@uni.edu", "response": "The key insight is..."}
```

## Environment Variables

| Variable | Required | Description |
|---|---|---|
| `DATABASE_URL` | ✅ | PostgreSQL connection string |
| `NEO4J_URI` | ✅ | `bolt://host:7687` |
| `NEO4J_USER` | ✅ | Neo4j username |
| `NEO4J_PASSWORD` | ✅ | Neo4j password |
| `PINECONE_API_KEY` | ✅ | Pinecone API key |
| `PINECONE_HOST` | ✅ | Index host URL |
| `OPENROUTER_API_KEY` | ✅ | OpenRouter key (gpt-4o-mini via OpenRouter) |
| `CLERK_JWKS_URL` | ✅ | Clerk JWKS endpoint |
| `CLERK_SECRET_KEY` | ✅ | Clerk secret key |
| `SMTP_HOST` | ✅ | SMTP server |
| `SMTP_USER` | ✅ | SMTP username |
| `SMTP_PASSWORD` | ✅ | SMTP password |
| `LANGFUSE_SECRET_KEY` | ⬜ | LangFuse observability (optional) |
| `CONFIDENCE_THRESHOLD` | ⬜ | Gap detection threshold (default: `0.72`) |
| `TOP_K_RESULTS` | ⬜ | Vector search top-k (default: `8`) |

## Key Design Decisions

### GraphRAG with Neo4j
Papers, authors, experts, concepts, citations, and topics are stored as graph nodes with typed relationships. When a user asks a question, the retrieval pipeline:
1. Semantic search (Pinecone) finds relevant passages
2. The matched paper IDs are walked in Neo4j to collect related concepts, co-authors, and citations
3. This graph-enriched context is fed to the LLM — enabling multi-hop reasoning

### Gap Detector → Expert Bridge
When `top_confidence < CONFIDENCE_THRESHOLD`:
1. GapDetectorAgent confirms the gap via LLM
2. ExpertRouterAgent queries Neo4j for experts who `KNOWS` the relevant concepts
3. Falls back to paper `Author` nodes if no registered experts
4. Sends personalised email invitations via SendEmailTool
5. Full trace logged to LangFuse

### Expert Response Flywheel
When an expert responds:
1. Response embedded and stored in Pinecone (attributed to expert + paper)
2. ExpertResponse node written to Neo4j, linked to Paper and Expert
3. Next user asking the same question gets the expert's answer **instantly** — no LLM call needed, directly retrieved from the vector store
