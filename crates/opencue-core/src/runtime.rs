//! The agent loop: build context, stream the model, run tools (asking for approval when needed),
//! and coordinate several agents inside a group chat.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::db::{new_id, now};
use crate::events::{ApprovalRequest, CoreEvent};
use crate::models::*;
use crate::providers::{ChatMessage, ChatRequest, ChatRole, LlmProvider};
use crate::tools::{self, arg_str, arg_u64, Toolbox};
use crate::Core;

pub const MAX_TOOL_STEPS: usize = 16;
const HISTORY_LIMIT: i64 = 80;
const MAX_TOOL_OUTPUT: usize = 30_000;

#[derive(Default)]
struct AgentOutcome {
    text: String,
    handoffs: Vec<String>,
}

fn message(
    thread_id: &str,
    role: Role,
    agent_id: Option<&str>,
    content: impl Into<String>,
) -> Message {
    Message {
        id: new_id(),
        thread_id: thread_id.to_string(),
        role,
        agent_id: agent_id.map(str::to_string),
        content: content.into(),
        tool_calls: None,
        tool_call_id: None,
        tool_name: None,
        status: "done".into(),
        input_tokens: 0,
        output_tokens: 0,
        created_at: now(),
    }
}

fn truncate(s: String, max: usize) -> String {
    if s.chars().count() <= max {
        return s;
    }
    let mut out: String = s.chars().take(max).collect();
    out.push_str("\n…[output truncated]");
    out
}

/// Agents mentioned as `@Name` in `text`, in order of appearance.
fn mentions(text: &str, agents: &[Agent], exclude: Option<&str>) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut found: Vec<(usize, String)> = agents
        .iter()
        .filter(|a| Some(a.id.as_str()) != exclude)
        .filter_map(|a| {
            let name = a.name.to_lowercase();
            let compact = name.replace(' ', "");
            lower
                .find(&format!("@{name}"))
                .or_else(|| lower.find(&format!("@{compact}")))
                .map(|pos| (pos, a.id.clone()))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, id)| id).collect()
}

/// Make the transcript valid for strict APIs: start with a user turn, every tool call answered,
/// no orphan tool results.
fn sanitize(history: Vec<ChatMessage>) -> Vec<ChatMessage> {
    let start = history
        .iter()
        .position(|m| m.role == ChatRole::User)
        .unwrap_or(history.len());
    let msgs = &history[start..];
    let mut out = Vec::with_capacity(msgs.len());
    let mut i = 0;
    while i < msgs.len() {
        let m = &msgs[i];
        match m.role {
            ChatRole::Tool => {} // orphan
            ChatRole::Assistant if !m.tool_calls.is_empty() => {
                let mut j = i + 1;
                let mut results: HashMap<String, ChatMessage> = HashMap::new();
                while j < msgs.len() && msgs[j].role == ChatRole::Tool {
                    if let Some(id) = &msgs[j].tool_call_id {
                        results.insert(id.clone(), msgs[j].clone());
                    }
                    j += 1;
                }
                out.push(m.clone());
                for call in &m.tool_calls {
                    out.push(results.remove(&call.id).unwrap_or_else(|| {
                        ChatMessage::tool(&call.id, "(no result: interrupted)")
                    }));
                }
                i = j;
                continue;
            }
            _ => out.push(m.clone()),
        }
        i += 1;
    }
    out
}

fn extract_json_array(text: &str) -> Option<Vec<String>> {
    let start = text.find('[')?;
    let end = text.rfind(']')?;
    let v: Value = serde_json::from_str(text.get(start..=end)?).ok()?;
    Some(
        v.as_array()?
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect(),
    )
}

impl Core {
    /// Post a user message to an agent or group thread and start the agents.
    pub async fn send_message(
        self: &Arc<Self>,
        thread_id: &str,
        text: &str,
    ) -> anyhow::Result<Message> {
        let text = text.trim();
        anyhow::ensure!(!text.is_empty(), "message is empty");
        let group = self.store.get_group(thread_id).await?;
        if group.is_none() {
            self.store.get_agent(thread_id).await?;
        }
        // A new message interrupts whatever was running.
        self.stop(thread_id);

        let msg = message(thread_id, Role::User, None, text);
        self.store.insert_message(&msg).await?;
        self.emit(CoreEvent::Message {
            message: msg.clone(),
        });

        let run_id = new_id();
        let token = CancellationToken::new();
        self.runs
            .lock()
            .unwrap()
            .insert(thread_id.to_string(), (run_id.clone(), token.clone()));
        self.emit(CoreEvent::ThreadsChanged);

        let core = self.clone();
        let thread = thread_id.to_string();
        let user_text = text.to_string();
        tokio::spawn(async move {
            match group {
                None => {
                    core.run_agent(&thread, &thread, None, &token).await;
                }
                Some(g) => {
                    if let Err(e) = core.run_group(&g, &user_text, &token).await {
                        core.emit(CoreEvent::Error {
                            thread_id: Some(thread.clone()),
                            message: format!("{e:#}"),
                        });
                    }
                }
            }
            {
                let mut runs = core.runs.lock().unwrap();
                if runs.get(&thread).is_some_and(|(id, _)| *id == run_id) {
                    runs.remove(&thread);
                }
            }
            core.emit(CoreEvent::ThreadsChanged);
        });
        Ok(msg)
    }

    async fn run_group(
        self: &Arc<Self>,
        group: &Group,
        user_text: &str,
        token: &CancellationToken,
    ) -> anyhow::Result<()> {
        let all = self.store.list_agents().await?;
        let members: Vec<Agent> = group
            .members
            .iter()
            .filter_map(|m| all.iter().find(|a| a.id == m.agent_id).cloned())
            .collect();
        anyhow::ensure!(!members.is_empty(), "this group has no members");

        let mut queue: VecDeque<String> = mentions(user_text, &members, None).into();
        if queue.is_empty() {
            queue = self.route(group, &members, user_text).await.into();
        }
        let mut auto_turns = 0i64;
        let mut limit_hit = false;
        while let Some(agent_id) = queue.pop_front() {
            if token.is_cancelled() {
                break;
            }
            let outcome = self
                .run_agent(&group.id, &agent_id, Some(group), token)
                .await;
            let mut next = outcome.handoffs;
            next.extend(mentions(&outcome.text, &members, Some(&agent_id)));
            for id in next {
                if id == agent_id || queue.contains(&id) || !members.iter().any(|m| m.id == id) {
                    continue;
                }
                if auto_turns >= group.max_auto_turns {
                    limit_hit = true;
                    break;
                }
                auto_turns += 1;
                queue.push_back(id);
            }
        }
        if limit_hit && !token.is_cancelled() {
            let ev = message(
                &group.id,
                Role::Event,
                None,
                format!(
                    "Agents paused after {} automatic turns. Send a message to continue.",
                    group.max_auto_turns
                ),
            );
            self.store.insert_message(&ev).await?;
            self.emit(CoreEvent::Message { message: ev });
        }
        Ok(())
    }

    /// Ask the group owner's model who should answer a message with no @mention.
    async fn route(&self, group: &Group, members: &[Agent], text: &str) -> Vec<String> {
        let owner = group
            .members
            .iter()
            .find(|m| m.is_owner)
            .and_then(|m| members.iter().find(|a| a.id == m.agent_id))
            .unwrap_or(&members[0]);
        if members.len() == 1 {
            return vec![owner.id.clone()];
        }
        let attempt = async {
            let pid = owner
                .provider_id
                .clone()
                .or(self.store.default_provider_id().await?)
                .ok_or_else(|| anyhow::anyhow!("no provider"))?;
            let (cfg, provider) = self.provider_for(&pid).await?;
            let model = owner
                .model
                .clone()
                .or(cfg.default_model)
                .unwrap_or_default();
            let roster = members
                .iter()
                .map(|a| {
                    let ins: String = self.store.instructions(&a.id).chars().take(200).collect();
                    format!("- {}: {}", a.name, ins.replace('\n', " "))
                })
                .collect::<Vec<_>>()
                .join("\n");
            let req = ChatRequest {
                model,
                system: "You route messages in a group chat of AI agents. Reply with only a JSON array of the names of the 1-2 agents best suited to answer, most relevant first.".into(),
                messages: vec![ChatMessage::user(format!("Agents:\n{roster}\n\nMessage from the user:\n{text}"))],
                tools: vec![],
                temperature: Some(0.0),
            };
            let resp = provider.chat(req, &|_| {}).await?;
            anyhow::Ok(extract_json_array(&resp.text).unwrap_or_default())
        };
        let picked: Vec<String> = match attempt.await {
            Ok(names) => names
                .iter()
                .filter_map(|n| {
                    members.iter().find(|a| {
                        a.name
                            .eq_ignore_ascii_case(n.trim().trim_start_matches('@'))
                    })
                })
                .map(|a| a.id.clone())
                .take(2)
                .collect(),
            Err(e) => {
                tracing::debug!("group routing failed: {e:#}");
                vec![]
            }
        };
        if picked.is_empty() {
            vec![owner.id.clone()]
        } else {
            picked
        }
    }

    async fn run_agent(
        self: &Arc<Self>,
        thread_id: &str,
        agent_id: &str,
        group: Option<&Group>,
        token: &CancellationToken,
    ) -> AgentOutcome {
        self.emit(CoreEvent::Busy {
            thread_id: thread_id.into(),
            agent_id: agent_id.into(),
            busy: true,
        });
        let result = self.agent_turns(thread_id, agent_id, group, token).await;
        self.emit(CoreEvent::Busy {
            thread_id: thread_id.into(),
            agent_id: agent_id.into(),
            busy: false,
        });
        match result {
            Ok(o) => o,
            Err(e) => {
                let mut m = message(thread_id, Role::Assistant, Some(agent_id), format!("{e:#}"));
                m.status = "error".into();
                if self.store.insert_message(&m).await.is_ok() {
                    self.emit(CoreEvent::Message { message: m });
                }
                AgentOutcome::default()
            }
        }
    }

    async fn agent_turns(
        self: &Arc<Self>,
        thread_id: &str,
        agent_id: &str,
        group: Option<&Group>,
        token: &CancellationToken,
    ) -> anyhow::Result<AgentOutcome> {
        let agent = self.store.get_agent(agent_id).await?;
        let provider_id = match agent.provider_id.clone() {
            Some(p) => p,
            None => self.store.default_provider_id().await?.ok_or_else(|| {
                anyhow::anyhow!(
                    "No model provider is configured yet. Open Settings → Models to add one."
                )
            })?,
        };
        let (pcfg, provider) = self.provider_for(&provider_id).await?;
        let model = agent
            .model
            .clone()
            .or(pcfg.default_model.clone())
            .filter(|m| !m.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Pick a model for {} in its profile (Model section).",
                    agent.name
                )
            })?;
        let supports_tools = provider.supports_tools();
        let global_mem = self.global_memory_enabled().await && agent.settings.use_global_memory;

        let toolbox = if supports_tools {
            let servers: Vec<McpServerConfig> = self
                .store
                .list_mcp_servers()
                .await?
                .into_iter()
                .filter(|s| {
                    s.enabled
                        && (agent.settings.mcp_all || agent.settings.mcp_servers.contains(&s.id))
                })
                .collect();
            Toolbox::build(&agent, group, global_mem, self.mcp.tools(&servers).await)
        } else {
            Toolbox {
                specs: vec![],
                mcp: vec![],
            }
        };

        let mut outcome = AgentOutcome::default();
        let mut last_user = String::new();
        for step in 0..MAX_TOOL_STEPS {
            let system = self
                .system_prompt(&agent, group, global_mem, supports_tools)
                .await;
            let history = self.history(thread_id, &agent, supports_tools).await?;
            if step == 0 {
                last_user = history
                    .iter()
                    .rev()
                    .find(|m| m.role == ChatRole::User)
                    .map(|m| m.content.clone())
                    .unwrap_or_default();
            }

            let mut msg = message(thread_id, Role::Assistant, Some(agent_id), "");
            msg.status = "streaming".into();
            self.store.insert_message(&msg).await?;
            self.emit(CoreEvent::Message {
                message: msg.clone(),
            });

            let buffer = Arc::new(Mutex::new(String::new()));
            let on_delta = {
                let buffer = buffer.clone();
                let (thread_id, message_id, agent_id) =
                    (thread_id.to_string(), msg.id.clone(), agent_id.to_string());
                move |t: &str| {
                    buffer.lock().unwrap().push_str(t);
                    self.emit(CoreEvent::Delta {
                        thread_id: thread_id.clone(),
                        message_id: message_id.clone(),
                        agent_id: agent_id.clone(),
                        text: t.to_string(),
                    });
                }
            };
            let req = ChatRequest {
                model: model.clone(),
                system,
                messages: history,
                tools: toolbox.specs.clone(),
                temperature: agent.settings.temperature,
            };
            let result = tokio::select! {
                r = provider.chat(req, &on_delta) => r,
                _ = token.cancelled() => {
                    msg.content = buffer.lock().unwrap().clone();
                    msg.status = "stopped".into();
                    self.store.update_message(&msg).await?;
                    self.emit(CoreEvent::Message { message: msg });
                    return Ok(outcome);
                }
            };
            let resp = match result {
                Ok(r) => r,
                Err(e) => {
                    let partial = buffer.lock().unwrap().clone();
                    msg.content = if partial.is_empty() {
                        format!("{e:#}")
                    } else {
                        format!("{partial}\n\n{e:#}")
                    };
                    msg.status = "error".into();
                    self.store.update_message(&msg).await?;
                    self.emit(CoreEvent::Message { message: msg });
                    return Ok(outcome);
                }
            };

            msg.content = resp.text.clone();
            msg.tool_calls = (!resp.tool_calls.is_empty()).then(|| resp.tool_calls.clone());
            msg.input_tokens = resp.input_tokens;
            msg.output_tokens = resp.output_tokens;
            msg.status = "done".into();
            self.store.update_message(&msg).await?;
            self.emit(CoreEvent::Message {
                message: msg.clone(),
            });
            if !resp.text.trim().is_empty() {
                outcome.text = resp.text.clone();
            }
            if resp.tool_calls.is_empty() {
                break;
            }

            for call in resp.tool_calls {
                let mut tmsg = message(thread_id, Role::Tool, Some(agent_id), "");
                tmsg.tool_call_id = Some(call.id.clone());
                tmsg.tool_name = Some(call.name.clone());
                tmsg.status = "running".into();

                let reason = toolbox
                    .approval_reason(&call)
                    .filter(|_| agent.settings.approval_mode == ApprovalMode::Ask);
                if let Some(reason) = reason {
                    tmsg.status = "awaiting_approval".into();
                    self.store.insert_message(&tmsg).await?;
                    self.emit(CoreEvent::Message {
                        message: tmsg.clone(),
                    });
                    let (tx, rx) = oneshot::channel();
                    let approval_id = new_id();
                    self.approvals
                        .lock()
                        .unwrap()
                        .insert(approval_id.clone(), tx);
                    self.emit(CoreEvent::ApprovalRequested {
                        approval: ApprovalRequest {
                            id: approval_id.clone(),
                            thread_id: thread_id.into(),
                            agent_id: agent_id.into(),
                            message_id: tmsg.id.clone(),
                            tool_call: call.clone(),
                            reason,
                        },
                    });
                    let approved = tokio::select! {
                        r = rx => r.unwrap_or(false),
                        _ = token.cancelled() => {
                            self.approvals.lock().unwrap().remove(&approval_id);
                            tmsg.status = "stopped".into();
                            tmsg.content = "Cancelled.".into();
                            self.store.update_message(&tmsg).await?;
                            self.emit(CoreEvent::Message { message: tmsg });
                            return Ok(outcome);
                        }
                    };
                    if !approved {
                        tmsg.status = "denied".into();
                        tmsg.content = "The user denied this action.".into();
                        self.store.update_message(&tmsg).await?;
                        self.emit(CoreEvent::Message { message: tmsg });
                        continue;
                    }
                    tmsg.status = "running".into();
                    self.store.update_message(&tmsg).await?;
                    self.emit(CoreEvent::Message {
                        message: tmsg.clone(),
                    });
                } else {
                    self.store.insert_message(&tmsg).await?;
                    self.emit(CoreEvent::Message {
                        message: tmsg.clone(),
                    });
                }

                let result = tokio::select! {
                    r = self.execute_tool(&agent, group, &toolbox, &call, &mut outcome.handoffs) => r,
                    _ = token.cancelled() => {
                        tmsg.status = "stopped".into();
                        tmsg.content = "Cancelled.".into();
                        self.store.update_message(&tmsg).await?;
                        self.emit(CoreEvent::Message { message: tmsg });
                        return Ok(outcome);
                    }
                };
                match result {
                    Ok(text) => {
                        tmsg.content = truncate(text, MAX_TOOL_OUTPUT);
                        tmsg.status = "done".into();
                    }
                    Err(e) => {
                        tmsg.content = format!("Error: {e:#}");
                        tmsg.status = "error".into();
                    }
                }
                self.store.update_message(&tmsg).await?;
                self.emit(CoreEvent::Message { message: tmsg });
            }

            if step + 1 == MAX_TOOL_STEPS {
                let ev = message(
                    thread_id,
                    Role::Event,
                    None,
                    format!("{} stopped after {MAX_TOOL_STEPS} tool steps.", agent.name),
                );
                self.store.insert_message(&ev).await?;
                self.emit(CoreEvent::Message { message: ev });
            }
        }

        if agent.settings.auto_memory && supports_tools && !outcome.text.is_empty() {
            let core = self.clone();
            let (agent, provider, model, reply) = (
                agent.clone(),
                provider.clone(),
                model.clone(),
                outcome.text.clone(),
            );
            tokio::spawn(async move {
                if let Err(e) = core
                    .extract_memories(&agent, provider, model, &last_user, &reply)
                    .await
                {
                    tracing::debug!("memory extraction failed: {e:#}");
                }
            });
        }
        Ok(outcome)
    }

    async fn system_prompt(
        &self,
        agent: &Agent,
        group: Option<&Group>,
        global_mem: bool,
        tools: bool,
    ) -> String {
        let today = chrono::Local::now().format("%A, %B %-d, %Y");
        let mut s = format!(
            "You are {name}, a personal AI agent living in OpenCue, a desktop app. Today is {today}.\n\
             Be concise, warm and proactive. Reply in the language the user writes in. Use Markdown when it helps.",
            name = agent.name
        );
        let instructions = self.store.instructions(&agent.id);
        if !instructions.trim().is_empty() {
            s.push_str(&format!(
                "\n\n## Your instructions\n{}",
                instructions.trim()
            ));
        }
        let mem = self.store.agent_memory(&agent.id).entries();
        if !mem.is_empty() {
            s.push_str("\n\n## What you remember (your memory)\n");
            for e in mem {
                s.push_str(&format!("- [#{}] {}\n", e.id, e.text));
            }
        }
        if global_mem {
            let shared = self.store.global_memory().entries();
            if !shared.is_empty() {
                s.push_str("\n\n## Shared memory (known to all agents)\n");
                for e in shared {
                    s.push_str(&format!("- [global #{}] {}\n", e.id, e.text));
                }
            }
        }
        if tools {
            s.push_str(
                "\n\n## Tools\n\
                 - When you learn a durable fact or preference about the user, save it with `remember`.\n\
                 - For anything current or factual you are unsure about, use `web_search` and `web_fetch`, and cite the URLs you used.\n\
                 - Some actions need the user's approval; if one is denied, adapt instead of retrying.",
            );
        }
        if let Some(g) = group {
            let all = self.store.list_agents().await.unwrap_or_default();
            let names = g
                .members
                .iter()
                .filter(|m| m.agent_id != agent.id)
                .filter_map(|m| all.iter().find(|a| a.id == m.agent_id))
                .map(|a| format!("@{}", a.name))
                .collect::<Vec<_>>()
                .join(", ");
            s.push_str(&format!(
                "\n\n## Group chat\nYou are in the group chat \"{}\" with the user and these agents: {names}. \
                 Messages from other agents appear as \"[Name]: …\". To hand work to someone, mention them (@Name) or call `send_to_agent`. \
                 Don't repeat what others already said; speak when you add something.",
                g.name
            ));
        }
        s
    }

    async fn history(
        &self,
        thread_id: &str,
        agent: &Agent,
        supports_tools: bool,
    ) -> anyhow::Result<Vec<ChatMessage>> {
        let msgs = self.store.messages(thread_id, HISTORY_LIMIT).await?;
        let names: HashMap<String, String> = self
            .store
            .list_agents()
            .await?
            .into_iter()
            .map(|a| (a.id, a.name))
            .collect();
        let mut out = Vec::with_capacity(msgs.len());
        for m in msgs {
            let mine = m.agent_id.as_deref() == Some(agent.id.as_str());
            match m.role {
                Role::User => out.push(ChatMessage::user(m.content)),
                Role::Event => out.push(ChatMessage::user(format!("[event] {}", m.content))),
                Role::Assistant if m.status == "error" => {}
                Role::Assistant if mine => {
                    let calls = if supports_tools {
                        m.tool_calls.unwrap_or_default()
                    } else {
                        vec![]
                    };
                    if !m.content.is_empty() || !calls.is_empty() {
                        out.push(ChatMessage::assistant(m.content, calls));
                    }
                }
                Role::Assistant => {
                    let who = m
                        .agent_id
                        .as_ref()
                        .and_then(|id| names.get(id))
                        .cloned()
                        .unwrap_or_else(|| "Agent".into());
                    let mut text = m.content.clone();
                    for call in m
                        .tool_calls
                        .iter()
                        .flatten()
                        .filter(|c| c.name == tools::SEND_TO_AGENT)
                    {
                        if arg_str(&call.arguments, "agent").is_some_and(|n| {
                            n.trim_start_matches('@').eq_ignore_ascii_case(&agent.name)
                        }) {
                            text.push_str(&format!(
                                "\n(to you) {}",
                                arg_str(&call.arguments, "message").unwrap_or_default()
                            ));
                        }
                    }
                    if !text.trim().is_empty() {
                        out.push(ChatMessage::user(format!("[{who}]: {}", text.trim())));
                    }
                }
                Role::Tool if mine && supports_tools => {
                    let content = if m.content.is_empty() {
                        format!("({})", m.status)
                    } else {
                        m.content
                    };
                    out.push(ChatMessage::tool(
                        m.tool_call_id.unwrap_or_default(),
                        content,
                    ));
                }
                Role::Tool => {}
            }
        }
        Ok(sanitize(out))
    }

    async fn execute_tool(
        &self,
        agent: &Agent,
        group: Option<&Group>,
        toolbox: &Toolbox,
        call: &ToolCall,
        handoffs: &mut Vec<String>,
    ) -> anyhow::Result<String> {
        let args = &call.arguments;
        let global_allowed = self.global_memory_enabled().await && agent.settings.use_global_memory;
        let scope_global = arg_str(args, "scope") == Some("global");
        match call.name.as_str() {
            tools::REMEMBER => {
                let text =
                    arg_str(args, "text").ok_or_else(|| anyhow::anyhow!("`text` is required"))?;
                let (file, label) = if scope_global && global_allowed {
                    (self.store.global_memory(), "shared memory")
                } else {
                    (self.store.agent_memory(&agent.id), "memory")
                };
                let e = file.add(text)?;
                Ok(format!("Saved to {label} as #{}: {}", e.id, e.text))
            }
            tools::RECALL => {
                let q = arg_str(args, "query").unwrap_or_default();
                let mut lines: Vec<String> = self
                    .store
                    .agent_memory(&agent.id)
                    .search(q)
                    .into_iter()
                    .map(|e| format!("[agent #{}] {}", e.id, e.text))
                    .collect();
                if global_allowed {
                    lines.extend(
                        self.store
                            .global_memory()
                            .search(q)
                            .into_iter()
                            .map(|e| format!("[global #{}] {}", e.id, e.text)),
                    );
                }
                Ok(if lines.is_empty() {
                    "No matching memories.".into()
                } else {
                    lines.join("\n")
                })
            }
            tools::FORGET => {
                let id = arg_u64(args, "id").ok_or_else(|| anyhow::anyhow!("`id` is required"))?
                    as usize;
                let file = if scope_global && global_allowed {
                    self.store.global_memory()
                } else {
                    self.store.agent_memory(&agent.id)
                };
                Ok(format!("Forgot: {}", file.remove(id)?))
            }
            tools::WEB_SEARCH => {
                let q =
                    arg_str(args, "query").ok_or_else(|| anyhow::anyhow!("`query` is required"))?;
                let max = arg_u64(args, "max_results").unwrap_or(8) as usize;
                let results = self.web_search(q, max).await?;
                if results.is_empty() {
                    return Ok("No results.".into());
                }
                Ok(results
                    .iter()
                    .enumerate()
                    .map(|(i, r)| format!("{}. [{}]({})\n   {}", i + 1, r.title, r.url, r.snippet))
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
            tools::WEB_FETCH => {
                let url =
                    arg_str(args, "url").ok_or_else(|| anyhow::anyhow!("`url` is required"))?;
                self.web_fetch(url, arg_u64(args, "offset").unwrap_or(0) as usize)
                    .await
            }
            tools::SEND_TO_AGENT => {
                let group = group
                    .ok_or_else(|| anyhow::anyhow!("send_to_agent only works in group chats"))?;
                let name =
                    arg_str(args, "agent").ok_or_else(|| anyhow::anyhow!("`agent` is required"))?;
                let target = self
                    .store
                    .find_agent_by_name(name)
                    .await?
                    .filter(|a| group.members.iter().any(|m| m.agent_id == a.id))
                    .ok_or_else(|| anyhow::anyhow!("no agent named {name} in this group"))?;
                anyhow::ensure!(
                    target.id != agent.id,
                    "you can't send a message to yourself"
                );
                handoffs.push(target.id.clone());
                Ok(format!(
                    "Delivered to {}. They will reply in the group after you finish.",
                    target.name
                ))
            }
            name => {
                let tool = toolbox
                    .mcp_tool(name)
                    .ok_or_else(|| anyhow::anyhow!("unknown tool `{name}`"))?;
                let (text, is_error) = self
                    .mcp
                    .call(&tool.server_id, &tool.tool.name, args.clone())
                    .await?;
                if is_error {
                    anyhow::bail!(text);
                }
                Ok(text)
            }
        }
    }

    async fn extract_memories(
        &self,
        agent: &Agent,
        provider: Arc<dyn LlmProvider>,
        model: String,
        user: &str,
        reply: &str,
    ) -> anyhow::Result<()> {
        let memory = self.store.agent_memory(&agent.id);
        let existing = memory
            .entries()
            .into_iter()
            .map(|e| format!("- {}", e.text))
            .collect::<Vec<_>>()
            .join("\n");
        let req = ChatRequest {
            model,
            system: "You maintain an assistant's long-term memory. From the exchange, extract only new, durable facts or preferences \
                     about the user that will matter in future conversations (not one-off requests). Return a JSON array of short \
                     self-contained strings, or [] if there is nothing worth keeping."
                .into(),
            messages: vec![ChatMessage::user(format!("Already remembered:\n{existing}\n\nUser: {user}\nAssistant: {reply}"))],
            tools: vec![],
            temperature: Some(0.0),
        };
        let resp = provider.chat(req, &|_| {}).await?;
        for fact in extract_json_array(&resp.text)
            .unwrap_or_default()
            .into_iter()
            .take(5)
        {
            memory.add(&fact)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(id: &str, name: &str) -> Agent {
        Agent {
            id: id.into(),
            name: name.into(),
            avatar: Avatar {
                emoji: String::new(),
                color: String::new(),
                shape: String::new(),
            },
            provider_id: None,
            model: None,
            settings: AgentSettings::default(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn finds_mentions_in_order() {
        let agents = vec![
            agent("1", "Travel Bot"),
            agent("2", "Chef"),
            agent("3", "Ana"),
        ];
        assert_eq!(
            mentions("hey @chef and @TravelBot", &agents, None),
            vec!["2", "1"]
        );
        assert_eq!(
            mentions("@Travel Bot pls", &agents, Some("1")),
            Vec::<String>::new()
        );
    }

    #[test]
    fn sanitize_fills_missing_tool_results_and_drops_orphans() {
        let call = ToolCall {
            id: "c1".into(),
            name: "x".into(),
            arguments: serde_json::json!({}),
        };
        let h = vec![
            ChatMessage::tool("zz", "orphan"),
            ChatMessage::assistant("hello", vec![]),
            ChatMessage::user("hi"),
            ChatMessage::assistant("", vec![call]),
            ChatMessage::user("next"),
        ];
        let out = sanitize(h);
        assert_eq!(out[0].role, ChatRole::User);
        assert_eq!(out.len(), 4);
        assert_eq!(out[2].tool_call_id.as_deref(), Some("c1"));
    }
}
