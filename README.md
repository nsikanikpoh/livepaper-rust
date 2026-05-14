# LivePaper

> Turn static research papers into a self-improving, conversational knowledge base.

LivePaper is an **Agentic GraphRAG** system that lets researchers ask questions across ingested academic papers, grounded in real content with citations. When the knowledge base can't answer a question, it automatically contacts the paper's authors or registered experts by email. Every expert reply is embedded back into the knowledge base — so the next researcher asking the same question gets the answer instantly.

---

## The Problem

1. **The answer gap** — papers rarely answer the specific question you have. You read an entire paper only to leave with more questions.
2. **The paywall tax** — you invest money and time accessing a paper, only to find it doesn't cover what you needed.
3. **The expert access problem** — the person who could answer your question in 30 seconds wrote the paper you're reading, but there's no practical way to reach them at scale.

Standard AI chatbots don't solve this — they hallucinate, lack domain depth, and have no connection to actual authors or the living state of a research field.

---

## How It Works

```
User Question
     │
     ▼
Retrieval Agent ──► Pinecone (vector search)
     │                    │
     │              Neo4j (graph enrichment)
     │                    │
     ▼                    ▼
Gap Detector Agent ◄── RetrievalResult
     │
     ├── Sufficient ──► LLM Answer Generation
     │
     └── Gap detected
              │
              ▼
       Expert Router Agent
              │
              ├── Query Neo4j for experts/authors by concept
              │
              └── SendEmailTool ──► SendGrid ──► Expert inbox
                                                      │
                                               Expert Response
                                                      │
                                                      ▼
                                         Response Ingestion Agent
                                                      │
                                            ┌─────────┴──────────┐
                                            ▼                     ▼
                                         Pinecone              Neo4j
                                      (new vector)       (ExpertResponse node)
```

---

## Tech Stack

| Layer | Technologies |
|---|---|
| **Backend** | Rust, Axum, Tokio, Tower, SQLx |
| **Databases** | PostgreSQL, Neo4j AuraDB, Pinecone |
| **AI / ML** | OpenRouter, GPT-4o-mini, text-embedding-3-small, LangFuse |
| **Architecture** | Agentic AI, GraphRAG, multi-hop graph reasoning, RAG pipeline |
| **Frontend** | Next.js, React, TypeScript, Tailwind CSS, react-markdown |
| **Auth** | Clerk, JWT, JWKS |
| **Email** | SendGrid |
| **Deployment** | Railway (backend), Vercel (frontend) |

---

## Project Structure

```
live_paper_rust/
├── backend/                  # Rust/Axum API
│   ├── src/
│   │   ├── main.rs           # Server bootstrap, router wiring
│   │   ├── models/           # Domain types
│   │   ├── db/
│   │   │   ├── postgres.rs   # PostgreSQL CRUD + migrations
│   │   │   ├── neo4j.rs      # GraphRAG nodes + relationships
│   │   │   └── pinecone.rs   # Vector upsert + semantic query
│   │   ├── services/
│   │   │   ├── llm.rs        # OpenRouter chat + JSON extraction
│   │   │   ├── embedding.rs  # Text → vector embeddings
│   │   │   ├── email.rs      # SendGrid email delivery
│   │   │   ├── langfuse.rs   # Observability traces + spans
│   │   │   └── pdf.rs        # PDF download + chunking
│   │   ├── agents/
│   │   │   ├── ingestion.rs          # PDF → Neo4j + Pinecone
│   │   │   ├── retrieval.rs          # Query → embed → search → graph
│   │   │   ├── gap_detector.rs       # Confidence check + LLM verification
│   │   │   ├── expert_router.rs      # Neo4j lookup → email experts
│   │   │   ├── chat.rs               # Full pipeline orchestration
│   │   │   └── response_ingestion.rs # Expert reply → knowledge base
│   │   ├── handlers/
│   │   │   ├── chat.rs       # POST /chat
│   │   │   ├── papers.rs     # CRUD /papers
│   │   │   └── experts.rs    # /experts endpoints
│   │   ├── middleware/
│   │   │   └── auth.rs       # Clerk JWT verification
│   │   └── utils/
│   │       ├── config.rs     # Environment config
│   │       └── errors.rs     # AppError → HTTP response
│   ├── Cargo.toml
│   ├── .env.example
│   └── Dockerfile
│
└── frontend/                 # Next.js app
    ├── components/
    │   └── ResearchChat.tsx  # Chat UI with markdown rendering
    ├── pages/
    │   ├── index.tsx         # Landing page
    │   ├── chat.tsx          # Chat interface
    │   ├── dashboard.tsx     # Admin paper management
    │   ├── experts.tsx       # Admin experts view
    │   └── expert-response.tsx # Public expert submission page
    ├── lib/
    │   └── api.ts            # Authenticated API client (Clerk JWT)
    └── hooks/
        └── useBackendSync.ts # User sync on sign-in
```

---

## API Reference

### Public Endpoints (no auth required)
```
GET  /papers         — list all papers with authors
GET  /papers/:id     — get a single paper with authors
POST /experts/response — submit an expert response
```

### Protected Endpoints (Clerk JWT required)
```
POST   /chat             — send a message, runs full agent pipeline
POST   /papers           — add a paper (triggers background ingestion)
PUT    /papers/:id       — update paper metadata
DELETE /papers/:id       — delete paper + remove vectors
GET    /experts          — list all experts with associated papers
POST   /experts          — invite an expert to a paper
```

---

## Running Locally

### Prerequisites

- [Rust](https://rustup.rs/) (stable, 1.75+)
- [Node.js](https://nodejs.org/) (18+)
- [PostgreSQL](https://www.postgresql.org/) running locally or a connection string
- [Neo4j AuraDB](https://neo4j.com/cloud/aura/) free instance (or local Neo4j)
- [Pinecone](https://www.pinecone.io/) account with an index created (1024 or 1536 dims)
- [Clerk](https://clerk.com/) app for authentication
- [OpenRouter](https://openrouter.ai/) API key
- [SendGrid](https://sendgrid.com/) API key with a verified sender

---

### 1. Clone the repository

```bash
git clone https://github.com/yourname/live_paper_rust.git
cd live_paper_rust
```

---

### 2. Backend setup

```bash
cd backend
cp .env.example .env
```

Fill in your `.env`:

```dotenv
# Server
PORT=8080

# PostgreSQL
DATABASE_URL=postgres://user:password@localhost:5432/livepaper

# Neo4j
NEO4J_URI=neo4j+s://xxxxxxxx.databases.neo4j.io
NEO4J_USER=neo4j
NEO4J_PASSWORD=your_aura_password
NEO4J_DB=                          # leave blank for AuraDB (uses server default)

# Pinecone
PINECONE_API_KEY=your_pinecone_key
PINECONE_HOST=https://your-index-host.pinecone.io
PINECONE_INDEX=livepaper
PINECONE_NAMESPACE=papers
PINECONE_DIMENSIONS=1024           # must match your index dimension

# OpenRouter
OPENROUTER_API_KEY=your_openrouter_key
OPENROUTER_BASE_URL=https://openrouter.ai/api/v1
LLM_MODEL=openai/gpt-4o-mini
EMBEDDING_MODEL=openai/text-embedding-3-small

# Clerk
CLERK_JWKS_URL=https://your-clerk-domain.clerk.accounts.dev/.well-known/jwks.json
CLERK_SECRET_KEY=sk_test_xxx

# SendGrid
SENDGRID_API_KEY=SG.xxxxxxxxxx
EMAIL_FROM=you@yourdomain.com
EMAIL_FROM_NAME=LivePaper
APP_BASE_URL=http://localhost:3000

# LangFuse (optional — leave blank to disable)
LANGFUSE_SECRET_KEY=
LANGFUSE_PUBLIC_KEY=
LANGFUSE_HOST=https://cloud.langfuse.com

# Agent tuning
CONFIDENCE_THRESHOLD=0.55
TOP_K_RESULTS=8
MAX_EXPERT_EMAILS_PER_QUERY=3
```

Run the backend:

```bash
cargo run
```

The server starts on `http://localhost:8080`. Database tables are created automatically on first run.

---

### 3. Frontend setup

```bash
cd ../frontend
npm install
cp .env.example .env.local
```

Fill in your `.env.local`:

```dotenv
NEXT_PUBLIC_API_URL=http://localhost:8080
NEXT_PUBLIC_CLERK_PUBLISHABLE_KEY=pk_test_xxx
CLERK_SECRET_KEY=sk_test_xxx
```

Run the frontend:

```bash
npm run dev
```

The app is available at `http://localhost:3000`.

---

### 4. Create a Pinecone index

In your [Pinecone console](https://app.pinecone.io):
1. Create a new index
2. Set **Dimensions** to `1024` (or `1536` if not using `PINECONE_DIMENSIONS`)
3. Set **Metric** to `cosine`
4. Copy the index host URL into `PINECONE_HOST`

---

### 5. Set up Clerk

1. Create an app at [clerk.com](https://clerk.com)
2. Copy your publishable and secret keys into both `.env` files
3. Copy the JWKS URL from **API Keys → Advanced** into `CLERK_JWKS_URL`
4. To make a user an admin, set their `publicMetadata` to `{ "role": "admin" }` via the Clerk dashboard — admin users get access to the paper management dashboard

---

### 6. Add your first paper

Sign in, navigate to the dashboard, and add a paper with its title, abstract, PDF URL, and authors. The ingestion pipeline runs in the background — it downloads the PDF, extracts text, runs LLM enrichment, and stores embeddings in Pinecone and nodes in Neo4j. Then go to the chat and start asking questions.

---

## Deployment

### Backend → Railway

1. Connect your GitHub repo to [Railway](https://railway.app)
2. Set all backend environment variables in Railway's dashboard
3. Railway detects the `Dockerfile` and builds automatically
4. Set the `PORT` variable — Railway injects this automatically

### Frontend → Vercel

1. Import your repo at [vercel.com](https://vercel.com)
2. Set `NEXT_PUBLIC_API_URL` to your Railway backend URL
3. Set your Clerk environment variables
4. Vercel detects Next.js and deploys automatically

---

## Who Is This For?

| User | Use case |
|---|---|
| Academic researchers | Query across papers without reading each one fully |
| PhD students | Navigate literature reviews, surface cross-paper connections |
| R&D teams | Build internal knowledge bases from proprietary research |
| Research institutions | Shared knowledge layer across departments |
| Science journalists | Understand and verify paper claims quickly |
| Grant writers | Synthesize the state of a field for background sections |

---

## License

MIT