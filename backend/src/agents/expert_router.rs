use anyhow::Result;
use serde_json::json;
use std::sync::Arc;

use crate::{
    AppState,
    agents::gap_detector::GapDetectionResult,
    tools::SendEmailTool,
};

pub struct ExpertRouterAgent {
    state: Arc<AppState>,
}

#[derive(Debug, Clone)]
pub struct RoutingResult {
    pub notified: Vec<String>, // emails that were actually sent
    pub candidates: Vec<ExpertCandidate>,
}

#[derive(Debug, Clone)]
pub struct ExpertCandidate {
    pub name: String,
    pub email: String,
    pub source: CandidateSource,
}

#[derive(Debug, Clone)]
pub enum CandidateSource {
    RegisteredExpert,
    PaperAuthor,
}

impl ExpertRouterAgent {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Route a gap to the best available experts/authors and notify them
    pub async fn route(
        &self,
        question: &str,
        gap: &GapDetectionResult,
        trace_id: &str,
    ) -> Result<RoutingResult> {
        tracing::info!("[ExpertRouter] Routing gap for question: {}", &question[..question.len().min(80)]);

        let max_emails = self.state.config.max_expert_emails_per_query;
        let mut candidates: Vec<ExpertCandidate> = Vec::new();

        // Step 1: Find registered experts via Neo4j (prefer these)
        let experts = self.state.neo4j
            .find_experts_by_concept(&gap.relevant_concepts)
            .await
            .unwrap_or_default();

        for (name, email) in experts {
            candidates.push(ExpertCandidate {
                name,
                email,
                source: CandidateSource::RegisteredExpert,
            });
        }

        // Step 2: If not enough experts, fall back to paper authors
        if candidates.len() < max_emails {
            let authors = self.state.neo4j
                .find_authors_by_concept(&gap.relevant_concepts)
                .await
                .unwrap_or_default();

            for (name, email) in authors {
                // don't duplicate
                if !candidates.iter().any(|c| c.email == email) {
                    candidates.push(ExpertCandidate {
                        name,
                        email,
                        source: CandidateSource::PaperAuthor,
                    });
                }
            }
        }

        // Step 3: Use LLM to rank candidates if we have too many
        let ranked = if candidates.len() > max_emails {
            self.rank_candidates(question, &candidates, trace_id).await
        } else {
            candidates.clone()
        };

        // Step 4: Send emails using the SendEmailTool
        let email_tool = SendEmailTool::new(self.state.clone());
        let mut notified = Vec::new();

        for candidate in ranked.iter().take(max_emails) {
            // Find a relevant paper title from the matched papers
            let paper_title = self.get_paper_title(&gap.relevant_paper_ids).await
                .unwrap_or_else(|| "Research Paper".to_string());

            let result = email_tool.execute(
                &candidate.name,
                &candidate.email,
                &paper_title,
                question,
                gap.relevant_paper_ids.first().map(|s| s.as_str()).unwrap_or(""),
            ).await?;

            if result.success {
                notified.push(candidate.email.clone());
                tracing::info!("[ExpertRouter] Email sent to {} ({})", candidate.name, candidate.email);
            }
        }

        self.state.langfuse.log_span(
            trace_id,
            "expert_router_routing",
            json!({
                "question_preview": &question[..question.len().min(100)],
                "concepts": gap.relevant_concepts,
            }),
            json!({
                "candidates_found": candidates.len(),
                "emails_sent": notified.len(),
                "notified": notified,
            }),
            None,
        ).await.ok();

        Ok(RoutingResult { notified, candidates: ranked })
    }

    async fn rank_candidates(
        &self,
        question: &str,
        candidates: &[ExpertCandidate],
        _trace_id: &str,
    ) -> Vec<ExpertCandidate> {
        let candidate_list = candidates.iter()
            .enumerate()
            .map(|(i, c)| format!("{}. {} <{}>", i + 1, c.name, c.email))
            .collect::<Vec<_>>()
            .join("\n");

        let system = r#"You are an expert routing assistant. 
Given a research question and a list of potential experts/authors,
return the indices (1-based) of the top 3 most relevant candidates, comma-separated.
Respond ONLY with numbers like: 1,3,5"#;

        let user = format!(
            "Question: {}\n\nCandidates:\n{}",
            question, candidate_list
        );

        match self.state.llm.complete(system, &user).await {
            Ok(response) => {
                let indices: Vec<usize> = response
                    .split(',')
                    .filter_map(|s| s.trim().parse::<usize>().ok())
                    .filter(|&i| i > 0 && i <= candidates.len())
                    .map(|i| i - 1)
                    .collect();

                if indices.is_empty() {
                    candidates.to_vec()
                } else {
                    indices.iter()
                        .filter_map(|&i| candidates.get(i))
                        .cloned()
                        .collect()
                }
            }
            Err(e) => {
                tracing::warn!("[ExpertRouter] Ranking failed: {}", e);
                candidates.to_vec()
            }
        }
    }

    async fn get_paper_title(&self, paper_ids: &[String]) -> Option<String> {
        for paper_id_str in paper_ids {
            if let Ok(uuid) = uuid::Uuid::parse_str(paper_id_str) {
                if let Ok(Some(paper)) = self.state.postgres.get_paper(uuid).await {
                    return Some(paper.title);
                }
            }
        }
        None
    }
}
