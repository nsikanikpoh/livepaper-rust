use anyhow::Result;
use serde_json::json;
use std::sync::Arc;

use crate::{
    AppState,
    models::RetrievalResult,
};

pub struct GapDetectorAgent {
    state: Arc<AppState>,
}

#[derive(Debug)]
pub struct GapDetectionResult {
    pub has_gap: bool,
    pub confidence: f64,
    pub reason: String,
    /// Concept names extracted from matched papers for expert routing
    pub relevant_concepts: Vec<String>,
    /// Paper IDs from matched results
    pub relevant_paper_ids: Vec<String>,
}

impl GapDetectorAgent {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Assess whether the retrieval result is sufficient or needs escalation
    pub async fn assess(
        &self,
        question: &str,
        retrieval: &RetrievalResult,
        trace_id: &str,
    ) -> Result<GapDetectionResult> {
        let threshold = self.state.config.confidence_threshold;
        let top_confidence = retrieval.top_confidence;

        tracing::info!(
            "[GapDetector] Top confidence: {:.3}, threshold: {:.3}",
            top_confidence,
            threshold
        );

        // Basic confidence check
        if top_confidence < threshold || retrieval.matches.is_empty() {
            // Use LLM to verify gap (avoid false positives)
            let llm_confirms_gap = self.llm_gap_check(question, retrieval, trace_id).await
                .unwrap_or(true);

            let relevant_concepts = self.extract_concepts(retrieval).await;
            let relevant_paper_ids = retrieval.matches.iter()
                .map(|m| m.metadata.paper_id.clone())
                .filter(|id| !id.is_empty())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();

            let result = GapDetectionResult {
                has_gap: llm_confirms_gap,
                confidence: top_confidence,
                reason: format!(
                    "Confidence {:.2} below threshold {:.2}; LLM confirms gap: {}",
                    top_confidence, threshold, llm_confirms_gap
                ),
                relevant_concepts,
                relevant_paper_ids,
            };

            self.state.langfuse.log_span(
                trace_id,
                "gap_detector_assessment",
                json!({ "question": question, "confidence": top_confidence }),
                json!({
                    "has_gap": result.has_gap,
                    "reason": result.reason,
                    "concepts": result.relevant_concepts,
                }),
                Some(json!({ "threshold": threshold })),
            ).await.ok();

            return Ok(result);
        }

        Ok(GapDetectionResult {
            has_gap: false,
            confidence: top_confidence,
            reason: format!("Sufficient confidence: {:.2}", top_confidence),
            relevant_concepts: vec![],
            relevant_paper_ids: vec![],
        })
    }

    async fn llm_gap_check(
        &self,
        question: &str,
        retrieval: &RetrievalResult,
        trace_id: &str,
    ) -> Result<bool> {
        if retrieval.matches.is_empty() {
            return Ok(true);
        }

        let context_preview = retrieval.matches.iter()
            .take(3)
            .map(|m| m.metadata.text.chars().take(200).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n---\n");

        let system = r#"You are a knowledge gap detector for research papers.
Given a question and retrieved context, determine if the context adequately answers the question.
Respond with ONLY "YES" if there is a gap (context does not answer the question) or "NO" if no gap."#;

        let user = format!(
            "Question: {}\n\nRetrieved context:\n{}\n\nIs there a knowledge gap?",
            question, context_preview
        );

        let response = self.state.llm.complete(system, &user).await?;

        self.state.langfuse.log_generation(
            trace_id,
            "gap_detector_llm_check",
            &self.state.config.llm_model,
            &user,
            &response,
        ).await.ok();

        Ok(response.trim().to_uppercase().starts_with("YES"))
    }

    async fn extract_concepts(&self, retrieval: &RetrievalResult) -> Vec<String> {
        let paper_ids: Vec<String> = retrieval.matches.iter()
            .map(|m| m.metadata.paper_id.clone())
            .filter(|id| !id.is_empty())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .take(3)
            .collect();

        let mut all_concepts = Vec::new();
        for paper_id in &paper_ids {
            if let Ok(concepts) = self.state.neo4j.get_paper_concepts(paper_id).await {
                all_concepts.extend(concepts);
            }
        }
        all_concepts.dedup();
        all_concepts.truncate(10);
        all_concepts
    }
}
