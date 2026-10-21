//! Questions to the user and chats between agents.
//!
//! Both go through the `bridge` table so they also work for agents backed by a CLI: their tool
//! server runs in a separate process and can neither show a card nor run another agent itself, so
//! it writes a row and the app (the primary process) picks it up, then waits for the answer.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::db::{new_id, now};
use crate::events::CoreEvent;
use crate::models::{Agent, Role};
use crate::Core;

/// Longest a question waits for the user.
const ASK_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const POLL: Duration = Duration::from_millis(400);

/// A question for the user. `kind` is "question" (free text or choices) or "permission"
/// (Accept / Reject).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub thread_id: String,
    pub agent_id: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub options: Vec<String>,
}

pub const APPROVED: &str = "approved";
pub const REJECTED: &str = "rejected";

/// Id of the conversation two agents share; the same for both directions.
pub fn dm_thread_id(a: &str, b: &str) -> String {
    let (x, y) = if a <= b { (a, b) } else { (b, a) };
    format!("dm_{x}_{y}")
}

pub fn is_dm_thread(thread_id: &str) -> bool {
    thread_id.starts_with("dm_")
}

fn row_to_question(id: &str, agent_id: &str, thread_id: &str, payload: &str) -> Question {
    let v: Value = serde_json::from_str(payload).unwrap_or_default();
    let text = |k: &str| {
        v.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    Question {
        id: id.into(),
        thread_id: thread_id.into(),
        agent_id: agent_id.into(),
        kind: if text("kind") == "permission" {
            "permission".into()
        } else {
            "question".into()
        },
        title: text("title"),
        body: text("body"),
        options: v
            .get("options")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|o| o.as_str().map(str::to_string))
                    .filter(|o| !o.trim().is_empty())
                    .collect()
            })
            .unwrap_or_default(),
    }
}

impl Core {
    /// Put a question to the user and wait for the answer.
    pub async fn ask_user(
        &self,
        thread_id: &str,
        agent: &Agent,
        payload: Value,
        token: Option<&CancellationToken>,
    ) -> anyhow::Result<String> {
        // A WhatsApp turn has no chat the owner is watching: its cards go to the agent's own chat.
        let thread_id = if crate::messenger::is_wa_thread(thread_id) {
            agent.id.as_str()
        } else {
            thread_id
        };
        let id = new_id();
        // The primary process shows the card itself; a secondary one leaves the row for it.
        let status = if self.primary { "announced" } else { "pending" };
        sqlx::query("INSERT INTO bridge (id, kind, agent_id, thread_id, payload, status, created_at) VALUES (?, 'ask', ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(&agent.id)
            .bind((!thread_id.is_empty()).then_some(thread_id))
            .bind(payload.to_string())
            .bind(status)
            .bind(now())
            .execute(&self.store.db)
            .await?;
        if self.primary {
            let thread = if thread_id.is_empty() {
                agent.id.as_str()
            } else {
                thread_id
            };
            self.emit(CoreEvent::QuestionRequested {
                question: row_to_question(&id, &agent.id, thread, &payload.to_string()),
            });
        }
        let started = std::time::Instant::now();
        loop {
            let row: Option<(String, Option<String>)> =
                sqlx::query_as("SELECT status, result FROM bridge WHERE id = ?")
                    .bind(&id)
                    .fetch_optional(&self.store.db)
                    .await?;
            match row {
                Some((s, result)) if s == "done" => return Ok(result.unwrap_or_default()),
                Some((s, _)) if s == "cancelled" => anyhow::bail!("The question was cancelled."),
                None => anyhow::bail!("The question disappeared."),
                _ => {}
            }
            let cancelled = match token {
                Some(t) => {
                    tokio::select! {
                        _ = t.cancelled() => true,
                        _ = tokio::time::sleep(POLL) => false,
                    }
                }
                None => {
                    tokio::time::sleep(POLL).await;
                    false
                }
            };
            if cancelled || started.elapsed() > ASK_TIMEOUT {
                self.cancel_question(&id).await;
                anyhow::bail!(if cancelled {
                    "Cancelled."
                } else {
                    "The user did not answer in time."
                });
            }
        }
    }

    /// Ask for permission to do something; true when the user accepts.
    pub async fn ask_permission(
        &self,
        thread_id: &str,
        agent: &Agent,
        title: &str,
        body: &str,
        token: Option<&CancellationToken>,
    ) -> anyhow::Result<bool> {
        let answer = self
            .ask_user(
                thread_id,
                agent,
                json!({ "kind": "permission", "title": title, "body": body }),
                token,
            )
            .await?;
        Ok(answer == APPROVED)
    }

    async fn cancel_question(&self, id: &str) {
        let _ = sqlx::query(
            "UPDATE bridge SET status = 'cancelled' WHERE id = ? AND status IN ('pending', 'announced')",
        )
        .bind(id)
        .execute(&self.store.db)
        .await;
        self.emit(CoreEvent::QuestionResolved {
            question_id: id.into(),
            answer: String::new(),
        });
    }

    /// The user answered a question card. False when it was already resolved.
    pub async fn answer_question(&self, id: &str, answer: &str) -> bool {
        let done = sqlx::query(
            "UPDATE bridge SET status = 'done', result = ? WHERE id = ? AND kind = 'ask' AND status IN ('pending', 'announced')",
        )
        .bind(answer)
        .bind(id)
        .execute(&self.store.db)
        .await
        .map(|r| r.rows_affected() > 0)
        .unwrap_or(false);
        if done {
            self.emit(CoreEvent::QuestionResolved {
                question_id: id.into(),
                answer: answer.into(),
            });
        }
        done
    }

    /// Questions still waiting for an answer (so a card survives a reload of the window).
    pub async fn pending_questions(&self) -> Vec<Question> {
        let rows: Vec<(String, String, Option<String>, String)> = sqlx::query_as(
            "SELECT id, agent_id, thread_id, payload FROM bridge WHERE kind = 'ask' AND status IN ('pending', 'announced') ORDER BY created_at",
        )
        .fetch_all(&self.store.db)
        .await
        .unwrap_or_default();
        rows.into_iter()
            .map(|(id, agent, thread, payload)| {
                let thread = thread.unwrap_or_else(|| agent.clone());
                row_to_question(&id, &agent, &thread, &payload)
            })
            .collect()
    }

    /// Talk to another agent: their reply comes back as the result. The exchange is kept in a
    /// conversation of its own that the user can open from the tool card.
    pub async fn chat_with_agent(
        self: &Arc<Self>,
        from: &Agent,
        to_name: &str,
        text: &str,
        token: &CancellationToken,
    ) -> anyhow::Result<String> {
        let name = to_name.trim().trim_start_matches('@');
        let target = self
            .store
            .find_agent_by_name(name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no agent named {name}"))?;
        anyhow::ensure!(target.id != from.id, "you can't send a message to yourself");
        let dm = dm_thread_id(&from.id, &target.id);
        let msg = crate::runtime::message(&dm, Role::Assistant, Some(&from.id), text);
        self.store.insert_message(&msg).await?;
        self.emit(CoreEvent::Message { message: msg });
        let outcome = self
            .run_agent(&dm, &target.id, None, &token.child_token())
            .await;
        let reply = outcome.text.trim().to_string();
        Ok(if reply.is_empty() {
            format!("{} did not reply.", target.name)
        } else {
            format!("[{}]: {reply}", target.name)
        })
    }

    /// Start the loop that serves requests written by tool servers in other processes.
    pub(crate) fn start_bridge(self: &Arc<Self>) {
        if !self.primary
            || self
                .bridge_started
                .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            return;
        }
        let core = self.clone();
        tokio::spawn(async move {
            // Whatever was waiting when the app last closed has no one to answer it.
            let _ = sqlx::query(
                "UPDATE bridge SET status = 'cancelled' WHERE status IN ('pending', 'announced', 'working')",
            )
            .execute(&core.store.db)
            .await;
            loop {
                tokio::time::sleep(POLL).await;
                core.serve_bridge().await;
            }
        });
    }

    async fn serve_bridge(self: &Arc<Self>) {
        let rows: Vec<(String, String, String, Option<String>, String)> = sqlx::query_as(
            "SELECT id, kind, agent_id, thread_id, payload FROM bridge WHERE status = 'pending' ORDER BY created_at",
        )
        .fetch_all(&self.store.db)
        .await
        .unwrap_or_default();
        for (id, kind, agent_id, thread, payload) in rows {
            let thread = thread
                .or_else(|| self.active_threads.lock().unwrap().get(&agent_id).cloned())
                .unwrap_or_else(|| agent_id.clone());
            if kind == "browser" {
                let _ = sqlx::query("UPDATE bridge SET status = 'working' WHERE id = ?")
                    .bind(&id)
                    .execute(&self.store.db)
                    .await;
                let core = self.clone();
                tokio::spawn(async move {
                    let text = core.browser_serve(&payload).await;
                    let _ =
                        sqlx::query("UPDATE bridge SET status = 'done', result = ? WHERE id = ?")
                            .bind(text)
                            .bind(&id)
                            .execute(&core.store.db)
                            .await;
                });
            } else if kind == "ask" {
                let _ = sqlx::query("UPDATE bridge SET status = 'announced' WHERE id = ?")
                    .bind(&id)
                    .execute(&self.store.db)
                    .await;
                self.emit(CoreEvent::QuestionRequested {
                    question: row_to_question(&id, &agent_id, &thread, &payload),
                });
            } else {
                let _ = sqlx::query("UPDATE bridge SET status = 'working' WHERE id = ?")
                    .bind(&id)
                    .execute(&self.store.db)
                    .await;
                let core = self.clone();
                tokio::spawn(async move {
                    let args: Value = serde_json::from_str(&payload).unwrap_or_default();
                    // A CLI agent's tool call isn't in the chat by itself: add the same
                    // "Chat with …" card an API agent gets, so the conversation can be followed.
                    let mut card = None;
                    if !thread.is_empty() && !is_dm_thread(&thread) {
                        let call = crate::models::ToolCall {
                            id: new_id(),
                            name: "send_to_agent".into(),
                            arguments: args.clone(),
                        };
                        let mut m = crate::runtime::message(
                            &thread,
                            crate::models::Role::Tool,
                            Some(&agent_id),
                            "",
                        );
                        m.tool_call_id = Some(call.id.clone());
                        m.tool_name = Some(call.name.clone());
                        m.tool_calls = Some(vec![call]);
                        m.status = "running".into();
                        if core.store.insert_message(&m).await.is_ok() {
                            core.emit(CoreEvent::Message { message: m.clone() });
                            card = Some(m);
                        }
                    }
                    let result = match core.store.get_agent(&agent_id).await {
                        Ok(from) => {
                            let to = args
                                .get("agent")
                                .and_then(Value::as_str)
                                .unwrap_or_default();
                            let text = args
                                .get("message")
                                .and_then(Value::as_str)
                                .unwrap_or_default();
                            core.chat_with_agent(&from, to, text, &CancellationToken::new())
                                .await
                        }
                        Err(e) => Err(e),
                    };
                    if let Some(mut m) = card {
                        match &result {
                            Ok(text) => {
                                m.content = text.clone();
                                m.status = "done".into();
                            }
                            Err(e) => {
                                m.content = format!("Error: {e:#}");
                                m.status = "error".into();
                            }
                        }
                        if core.store.update_message(&m).await.is_ok() {
                            core.emit(CoreEvent::Message { message: m });
                        }
                    }
                    let text = result.unwrap_or_else(|e| format!("Error: {e:#}"));
                    let _ =
                        sqlx::query("UPDATE bridge SET status = 'done', result = ? WHERE id = ?")
                            .bind(text)
                            .bind(&id)
                            .execute(&core.store.db)
                            .await;
                });
            }
        }
    }

    /// Same as `chat_with_agent`, from a tool server in another process.
    pub async fn chat_with_agent_remote(
        &self,
        from: &Agent,
        to_name: &str,
        text: &str,
    ) -> anyhow::Result<String> {
        let id = new_id();
        sqlx::query("INSERT INTO bridge (id, kind, agent_id, thread_id, payload, status, created_at) VALUES (?, 'agent', ?, NULL, ?, 'pending', ?)")
            .bind(&id)
            .bind(&from.id)
            .bind(json!({ "agent": to_name, "message": text }).to_string())
            .bind(now())
            .execute(&self.store.db)
            .await?;
        let started = std::time::Instant::now();
        loop {
            tokio::time::sleep(POLL).await;
            let row: Option<(String, Option<String>)> =
                sqlx::query_as("SELECT status, result FROM bridge WHERE id = ?")
                    .bind(&id)
                    .fetch_optional(&self.store.db)
                    .await?;
            match row {
                Some((s, result)) if s == "done" => return Ok(result.unwrap_or_default()),
                Some((s, _)) if s == "cancelled" => {
                    anyhow::bail!("The conversation was cancelled.")
                }
                None => anyhow::bail!("The request disappeared."),
                _ => {}
            }
            anyhow::ensure!(
                started.elapsed() < Duration::from_secs(15 * 60),
                "The other agent took too long to answer."
            );
        }
    }
}
