use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    AppState,
    agents::{retrieval::RetrievalAgent, gap_detector::GapDetectorAgent, expert_router::ExpertRouterAgent},
    services::llm::ChatMessage,
};

pub struct ChatAgent { state: Arc<AppState> }

pub struct ChatAgentResponse {
    pub content: String,
    pub sources: serde_json::Value,
    pub escalated: bool,
    pub escalation_message: Option<String>,
    pub trace_id: String,
    pub notified_experts: Vec<String>,
}

impl ChatAgent {
    pub fn new(state: Arc<AppState>) -> Self { Self { state } }

    pub async fn process(
        &self,
        question: &str,
        session_id: Uuid,
        user_id: Uuid,
        history: &[ChatMessage],
    ) -> Result<ChatAgentResponse> {
        let trace_id = self.state.langfuse.create_trace(
            "chat_agent", &session_id.to_string(), &user_id.to_string(),
            json!({"question": question}),
        ).await.unwrap_or_else(|_| Uuid::new_v4().to_string());

        // ── Step 1: Retrieval ─────────────────────────────────────────────
        let retrieval_agent = RetrievalAgent::new(self.state.clone());
        let top_k = self.state.config.top_k_results;
        let retrieval = retrieval_agent.retrieve(question, top_k, &trace_id).await?;
        let graph_ctx = retrieval_agent.get_graph_context(&retrieval.matches).await.unwrap_or_default();
        let context = RetrievalAgent::build_context_string(&retrieval.matches, &graph_ctx);

        // ── Step 2: Gap detection ─────────────────────────────────────────
        let gap_detector = GapDetectorAgent::new(self.state.clone());
        let gap = gap_detector.assess(question, &retrieval, &trace_id).await?;

        // ── Step 3: Escalate BEFORE generating the answer ─────────────────
        //
        // This is intentional: the LLM must know whether experts were actually
        // notified so it can tell the user truthfully. If we generate the answer
        // first, the model says "I cannot contact experts" — because at that
        // point it's true. By escalating first we can pass the real outcome
        // ("3 experts notified") into the system prompt.
        let mut notified: Vec<String> = Vec::new();
        let mut escalation_message: Option<String> = None;

        if gap.has_gap {
            tracing::info!("[ChatAgent] Gap detected (confidence {:.2}) — routing to experts", gap.confidence);
            let router = ExpertRouterAgent::new(self.state.clone());
            match router.route(question, &gap, &trace_id).await {
                Ok(routing) => {
                    notified = routing.notified.clone();
                    self.state.langfuse
                        .log_escalation(&trace_id, question, gap.confidence, &routing.notified)
                        .await.ok();

                    if !routing.notified.is_empty() {
                        escalation_message = Some(format!(
                            "{} expert(s) have been notified and will respond by email.",
                            routing.notified.len()
                        ));
                    } else {
                        // Gap detected but no experts/authors on file for these concepts
                        escalation_message = Some(
                            "No registered experts were found for this topic. \
                             Consider inviting relevant authors via the dashboard."
                            .into(),
                        );
                    }
                }
                Err(e) => tracing::error!("[ChatAgent] Expert routing failed: {e}"),
            }
        }

        // ── Step 4: Generate answer — with full escalation context ────────
        //
        // The system prompt now reflects ground truth: whether a gap exists,
        // whether experts were notified, and how many. The LLM will never
        // claim it "cannot contact experts" because by this point it either
        // has or hasn't, and we tell it exactly what happened.
        let system = self.build_system_prompt(&context, gap.has_gap, &notified, &escalation_message);
        let answer = self.generate_answer(question, &system, history, &trace_id).await?;

        let sources = json!({
            "passages": retrieval.matches.iter().map(|m| json!({
                "paper_id":    m.metadata.paper_id,
                "paper_title": m.metadata.paper_title,
                "score":       m.score,
                "chunk_type":  m.metadata.chunk_type,
                "preview":     m.metadata.text.chars().take(150).collect::<String>(),
            })).collect::<Vec<_>>(),
            "graph_context": graph_ctx,
            "confidence": retrieval.top_confidence,
        });

        Ok(ChatAgentResponse {
            content: answer,
            sources,
            escalated: gap.has_gap,
            escalation_message,
            trace_id,
            notified_experts: notified,
        })
    }

    async fn generate_answer(
        &self,
        question: &str,
        system: &str,
        history: &[ChatMessage],
        trace_id: &str,
    ) -> Result<String> {
        let mut msgs = history.to_vec();
        msgs.push(ChatMessage { role: "user".into(), content: question.to_string() });
        let answer = self.state.llm.chat(system, msgs).await?;
        self.state.langfuse.log_generation(
            trace_id, "answer_generation",
            &self.state.config.llm_model, question, &answer,
        ).await.ok();
        Ok(answer)
    }

    fn build_system_prompt(
        &self,
        context: &str,
        has_gap: bool,
        notified: &[String],
        escalation_message: &Option<String>,
    ) -> String {
        let escalation_block = if has_gap {
            if !notified.is_empty() {
                format!(
                    "\n\n## Knowledge Gap — Expert Escalation Triggered\n\
                     The knowledge base does not fully answer this question. \
                     {} expert(s) have already been notified by email and will respond directly to the user.\n\
                     In your answer:\n\
                     - Share whatever partial information exists in the context below\n\
                     - Clearly state that this question has been escalated to {} expert(s)\n\
                     - Do NOT say you cannot contact experts — you already have\n\
                     - Do NOT suggest the user contact experts themselves — that is already done\n\
                     - Do NOT list example questions to ask — answer what you can and confirm escalation",
                    notified.len(),
                    notified.len(),
                )
            } else {
                "\n\n## Knowledge Gap — No Experts on File\n\
                 The knowledge base does not fully answer this question and no registered experts \
                 were found for the relevant topics.\n\
                 In your answer:\n\
                 - Share whatever partial information exists in the context below\n\
                 - Acknowledge the gap honestly\n\
                 - Suggest the user invite relevant paper authors via the LivePaper dashboard"
                .to_string()
            }
        } else {
            String::new()
        };

        format!(
            r#"You are LivePaper, an AI research assistant that helps researchers understand academic papers.
You have access to a knowledge base of ingested papers with their concepts, citations, and expert responses.
Always cite which paper or source you are drawing from. Never hallucinate.
{escalation_block}

--- KNOWLEDGE BASE ---
{ctx}
--- END KNOWLEDGE BASE ---

Structure your response:
1. Direct answer using evidence from the knowledge base
2. Supporting citations (paper title or source)
3. If escalated: confirm that expert(s) have been notified and the user will hear back by email"#,
            escalation_block = escalation_block,
            ctx = if context.is_empty() { "No relevant passages found in the knowledge base." } else { context },
        )
    }
}