//! The AI council: a group of agents answers a question the way a committee would.
//!
//! 1. **Opinions** — every member answers on their own, at the same time, without seeing the others.
//! 2. **Review** — every member reads all the answers (unnamed, as A, B, C…), ranks them and points
//!    out mistakes and gaps.
//! 3. **Verdict** — the chair (the group's owner) writes the final answer from the opinions, the
//!    reviews and the combined ranking.
//!
//! It runs on `/ai-council <question>` in any group, and on every message of a group created as a
//! council. Members keep their own model, so a council can mix providers.

use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::events::CoreEvent;
use crate::models::*;
use crate::providers::{ChatMessage, ChatRequest, LlmProvider};
use crate::runtime::message;
use crate::Core;

const MEMBER_TIMEOUT: Duration = Duration::from_secs(240);
/// How much of the conversation before the question each member gets.
const CONTEXT_MESSAGES: usize = 6;

/// A council question found in a message, if there is one.
pub struct CouncilRequest {
    pub question: String,
    /// Skip the review round: opinions, then the verdict.
    pub quick: bool,
}

/// `/ai-council [--quick] <question>`, or any plain message to a group made as a council.
pub fn parse_request(group: &Group, text: &str, mentions_someone: bool) -> Option<CouncilRequest> {
    let t = text.trim();
    let lower = t.to_ascii_lowercase();
    if lower == "/ai-council"
        || lower.starts_with("/ai-council ")
        || lower.starts_with("/ai-council\n")
    {
        let mut rest = t["/ai-council".len()..].trim();
        let mut quick = false;
        if let Some(r) = rest.strip_prefix("--quick") {
            quick = true;
            rest = r.trim();
        }
        return Some(CouncilRequest {
            question: rest.to_string(),
            quick,
        });
    }
    (group.kind == "council" && !mentions_someone && !t.starts_with('/')).then(|| CouncilRequest {
        question: t.to_string(),
        quick: false,
    })
}

/// Ranking letters in the order a reviewer wrote them: `Ranking: B > A > C` → `[1, 0, 2]`.
pub fn parse_ranking(text: &str, n: usize) -> Vec<usize> {
    let Some(line) = text
        .lines()
        .rev()
        .find(|l| l.to_ascii_lowercase().contains("ranking:"))
    else {
        return vec![];
    };
    let after = &line[line.to_ascii_lowercase().rfind("ranking:").unwrap_or(0) + 8..];
    let mut out = vec![];
    for tok in after.split(['>', ',', ' ']) {
        let tok = tok.trim().trim_matches(|c: char| !c.is_ascii_alphabetic());
        if tok.len() == 1 {
            let i = (tok.as_bytes()[0].to_ascii_uppercase() - b'A') as usize;
            if i < n && !out.contains(&i) {
                out.push(i);
            }
        }
    }
    out
}

/// Borda count: the answer ranked first by a reviewer gets `n - 1` points, the last gets 0.
/// Returns answer indexes, best first.
pub fn combined_ranking(rankings: &[Vec<usize>], n: usize) -> Vec<(usize, usize)> {
    let mut points = vec![0usize; n];
    for r in rankings {
        for (place, &answer) in r.iter().enumerate() {
            points[answer] += n.saturating_sub(1 + place);
        }
    }
    let mut order: Vec<(usize, usize)> = points.into_iter().enumerate().collect();
    order.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    order
}

fn letter(i: usize) -> char {
    (b'A' + (i % 26) as u8) as char
}

impl Core {
    async fn agent_model(&self, agent: &Agent) -> anyhow::Result<(Arc<dyn LlmProvider>, String)> {
        let pid = match agent.provider_id.clone() {
            Some(p) => p,
            None => self.store.default_provider_id().await?.ok_or_else(|| {
                anyhow::anyhow!("No model provider is configured yet: Settings → Providers.")
            })?,
        };
        let (cfg, provider) = self.provider_for(&pid).await?;
        let model = agent
            .model
            .clone()
            .or(cfg.default_model)
            .filter(|m| !m.is_empty())
            .ok_or_else(|| anyhow::anyhow!("Pick a model for {} in its profile.", agent.name))?;
        Ok((provider, model))
    }

    fn council_identity(&self, agent: &Agent) -> String {
        let mut s = format!("You are {}.", agent.name);
        let ins = self.store.instructions(&agent.id);
        if !ins.trim().is_empty() {
            s.push_str(&format!("\n\n## Your instructions\n{}", ins.trim()));
        }
        let soul = self.store.soul(&agent.id);
        if !soul.trim().is_empty() {
            s.push_str(&format!("\n\n## Who you are\n{}", soul.trim()));
        }
        s
    }

    /// One answer from one member, shown in the chat as it is written.
    async fn council_turn(
        self: &Arc<Self>,
        group_id: &str,
        agent: &Agent,
        system: String,
        user: String,
        token: &CancellationToken,
    ) -> Option<String> {
        let mut msg = message(group_id, Role::Assistant, Some(&agent.id), "");
        msg.status = "streaming".into();
        let _ = self.store.insert_message(&msg).await;
        self.emit(CoreEvent::Message {
            message: msg.clone(),
        });
        self.emit(CoreEvent::Busy {
            thread_id: group_id.into(),
            agent_id: agent.id.clone(),
            busy: true,
        });
        let result = async {
            let (provider, model) = self.agent_model(agent).await?;
            let buffer = Arc::new(std::sync::Mutex::new(String::new()));
            let sink = {
                let (core, buffer, thread, mid, aid) = (
                    self.clone(),
                    buffer.clone(),
                    group_id.to_string(),
                    msg.id.clone(),
                    agent.id.clone(),
                );
                move |t: &str| {
                    buffer.lock().unwrap().push_str(t);
                    core.emit(CoreEvent::Delta {
                        thread_id: thread.clone(),
                        message_id: mid.clone(),
                        agent_id: aid.clone(),
                        text: t.to_string(),
                    });
                }
            };
            let req = ChatRequest {
                model,
                system,
                messages: vec![ChatMessage::user(user)],
                temperature: agent.settings.temperature,
                ..Default::default()
            };
            let resp = tokio::select! {
                r = tokio::time::timeout(MEMBER_TIMEOUT, provider.chat(req, &sink)) => {
                    r.map_err(|_| anyhow::anyhow!("took too long"))??
                }
                _ = token.cancelled() => anyhow::bail!("stopped"),
            };
            anyhow::Ok(resp)
        }
        .await;
        self.emit(CoreEvent::Busy {
            thread_id: group_id.into(),
            agent_id: agent.id.clone(),
            busy: false,
        });
        let text = match result {
            Ok(resp) => {
                msg.content = resp.text.clone();
                msg.input_tokens = resp.input_tokens;
                msg.output_tokens = resp.output_tokens;
                msg.status = "done".into();
                Some(crate::providers::strip_think(&resp.text)).filter(|t| !t.trim().is_empty())
            }
            Err(e) => {
                msg.content = format!("{e:#}");
                msg.status = if token.is_cancelled() {
                    "stopped"
                } else {
                    "error"
                }
                .into();
                None
            }
        };
        let _ = self.store.update_message(&msg).await;
        self.emit(CoreEvent::Message { message: msg });
        text
    }

    async fn council_event(&self, group_id: &str, text: String) {
        let ev = message(group_id, Role::Event, None, text);
        if self.store.insert_message(&ev).await.is_ok() {
            self.emit(CoreEvent::Message { message: ev });
        }
    }

    /// A little of the conversation before the question, so a council in a long chat is not blind.
    async fn council_context(&self, group: &Group, members: &[Agent]) -> String {
        let recent = self
            .store
            .messages(&group.id, 40)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|m| {
                matches!(m.role, Role::User | Role::Assistant)
                    && !m.content.trim().is_empty()
                    && m.status == "done"
            })
            .collect::<Vec<_>>();
        // The newest user message is the question itself.
        let skip = recent
            .iter()
            .rposition(|m| m.role == Role::User)
            .unwrap_or(0);
        let lines: Vec<String> = recent[..skip]
            .iter()
            .rev()
            .take(CONTEXT_MESSAGES)
            .rev()
            .map(|m| {
                let who = match m.role {
                    Role::User => "User".to_string(),
                    _ => m
                        .agent_id
                        .as_ref()
                        .and_then(|id| members.iter().find(|a| &a.id == id))
                        .map(|a| a.name.clone())
                        .unwrap_or_else(|| "Agent".into()),
                };
                let text: String = m.content.chars().take(600).collect();
                format!("{who}: {text}")
            })
            .collect();
        if lines.is_empty() {
            String::new()
        } else {
            format!("Conversation so far:\n{}\n\n", lines.join("\n"))
        }
    }

    pub(crate) async fn run_council(
        self: &Arc<Self>,
        group: &Group,
        members: &[Agent],
        req: CouncilRequest,
        token: &CancellationToken,
    ) -> anyhow::Result<()> {
        let question = req.question.trim().to_string();
        if question.is_empty() {
            self.council_event(
                &group.id,
                "⚖️ Council: ask a question after the command, e.g. /ai-council should we use Postgres or SQLite?".into(),
            )
            .await;
            return Ok(());
        }
        let chair = group
            .members
            .iter()
            .find(|m| m.is_owner)
            .and_then(|m| members.iter().find(|a| a.id == m.agent_id))
            .unwrap_or(&members[0])
            .clone();
        let context = self.council_context(group, members).await;
        self.council_event(
            &group.id,
            format!(
                "⚖️ Council convened · {} members · chair {}\nRound 1: each member answers on their own.",
                members.len(),
                chair.name
            ),
        )
        .await;

        // Round 1: opinions, in parallel.
        let opinions = futures::future::join_all(members.iter().map(|a| {
            let system = format!(
                "{}\n\n## The council\nYou are a member of an AI council. The same question goes to every member and nobody sees the others' answers yet. \
                 Give your own best answer from your expertise and perspective: your position first (one or two sentences), then your reasoning in at most 180 words, \
                 then the main risk, or what would change your mind. Be specific and honest about uncertainty; no filler. Reply in the language of the question.",
                self.council_identity(a)
            );
            let user = format!("{context}Question:\n{question}");
            async move { self.council_turn(&group.id, a, system, user, token).await }
        }))
        .await;
        let answered: Vec<(&Agent, String)> = members
            .iter()
            .zip(opinions)
            .filter_map(|(a, o)| o.map(|o| (a, o)))
            .collect();
        if token.is_cancelled() {
            return Ok(());
        }
        if answered.is_empty() {
            self.council_event(
                &group.id,
                "⚖️ Council: no member could answer. Check their models in their profiles.".into(),
            )
            .await;
            return Ok(());
        }

        // Round 2: anonymous review.
        let mut ranking_note = String::new();
        let mut reviews: Vec<(&Agent, String)> = vec![];
        if !req.quick && answered.len() > 1 {
            self.council_event(
                &group.id,
                "Round 2: each member reviews the answers without knowing who wrote them.".into(),
            )
            .await;
            let labeled = answered
                .iter()
                .enumerate()
                .map(|(i, (_, o))| format!("### Answer {}\n{}", letter(i), o.trim()))
                .collect::<Vec<_>>()
                .join("\n\n");
            let n = answered.len();
            let out = futures::future::join_all(answered.iter().map(|(a, _)| {
                let system = format!(
                    "{}\n\n## The council\nYou are reviewing your council's answers. They are unnamed (A, B, C…). Be fair and concrete.",
                    self.council_identity(a)
                );
                let user = format!(
                    "{context}Question:\n{question}\n\n{labeled}\n\nRank the answers from best to worst. Then, in at most 120 words: say which is strongest and why, \
                     name concrete mistakes, gaps or unsupported claims in any answer (refer to them by letter), and say if anything changed your mind. \
                     The last line must be exactly: Ranking: {}",
                    (0..n).map(|i| letter(i).to_string()).collect::<Vec<_>>().join(" > ")
                );
                async move { self.council_turn(&group.id, a, system, user, token).await }
            }))
            .await;
            for ((a, _), r) in answered.iter().zip(out) {
                if let Some(r) = r {
                    reviews.push((a, r));
                }
            }
            let rankings: Vec<Vec<usize>> =
                reviews.iter().map(|(_, r)| parse_ranking(r, n)).collect();
            if rankings.iter().any(|r| !r.is_empty()) {
                let order = combined_ranking(&rankings, n);
                ranking_note = order
                    .iter()
                    .map(|(i, pts)| format!("{} ({}, {pts} pts)", answered[*i].0.name, letter(*i)))
                    .collect::<Vec<_>>()
                    .join(" > ");
                self.council_event(&group.id, format!("Combined ranking: {ranking_note}"))
                    .await;
            }
        }
        if token.is_cancelled() {
            return Ok(());
        }

        // Round 3: the chair's verdict.
        self.council_event(
            &group.id,
            format!("Verdict: {} writes the council's answer.", chair.name),
        )
        .await;
        let opinions_text = answered
            .iter()
            .enumerate()
            .map(|(i, (a, o))| format!("### {} (Answer {})\n{}", a.name, letter(i), o.trim()))
            .collect::<Vec<_>>()
            .join("\n\n");
        let reviews_text = if reviews.is_empty() {
            String::new()
        } else {
            format!(
                "\n\n## Reviews\n{}",
                reviews
                    .iter()
                    .map(|(a, r)| format!("### Review by {}\n{}", a.name, r.trim()))
                    .collect::<Vec<_>>()
                    .join("\n\n")
            )
        };
        let system = format!(
            "{}\n\n## The council\nYou chair this AI council. Write the council's final answer for the user from the members' opinions and reviews. Don't just average them: \
             decide, using the strongest arguments and the mistakes the reviews found.",
            self.council_identity(&chair)
        );
        let user = format!(
            "{context}Question:\n{question}\n\n## Opinions\n{opinions_text}{reviews_text}{}\n\n\
             Write, in the user's language and in at most 300 words: **Verdict** (the answer or recommendation, 2-4 sentences), **Where the council agreed**, \
             **Where it disagreed** (and who held each view), **Recommended next step**, and **Confidence** (low, medium or high, and why). Credit members by name.",
            if ranking_note.is_empty() {
                String::new()
            } else {
                format!("\n\nCombined ranking by the members: {ranking_note}")
            }
        );
        self.council_turn(&group.id, &chair, system, user, token)
            .await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(kind: &str) -> Group {
        Group {
            id: "g".into(),
            name: "G".into(),
            max_auto_turns: 4,
            members: vec![],
            created_at: String::new(),
            kind: kind.into(),
            config: serde_json::json!({}),
        }
    }

    #[test]
    fn finds_the_command_and_council_groups() {
        let g = group("chat");
        let r = parse_request(&g, "/ai-council --quick which db?", false).unwrap();
        assert!(r.quick && r.question == "which db?");
        assert!(parse_request(&g, "hello", false).is_none());
        assert!(parse_request(&g, "/ai-council", false)
            .unwrap()
            .question
            .is_empty());
        let c = group("council");
        assert_eq!(
            parse_request(&c, "which db?", false).unwrap().question,
            "which db?"
        );
        assert!(parse_request(&c, "@Alice which db?", true).is_none());
        assert!(parse_request(&c, "/plan do x", false).is_none());
    }

    #[test]
    fn reads_rankings() {
        assert_eq!(
            parse_ranking("Reasons...\nRanking: B > A > C", 3),
            vec![1, 0, 2]
        );
        assert_eq!(parse_ranking("ranking: c, a", 3), vec![2, 0]);
        assert_eq!(parse_ranking("Ranking: A > Z > A", 3), vec![0]);
        assert!(parse_ranking("no ranking line", 3).is_empty());
    }

    #[test]
    fn combines_rankings_with_borda() {
        let order = combined_ranking(&[vec![1, 0, 2], vec![1, 2, 0], vec![0, 1, 2]], 3);
        assert_eq!(order[0].0, 1);
        assert_eq!(order.last().unwrap().0, 2);
    }
}
