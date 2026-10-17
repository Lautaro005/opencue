//! "Backend CLI" providers: run the user's own logged-in `claude` / `codex` binary so their
//! subscription is used through the official client. OpenCue never sees or stores those tokens.

use std::process::Stdio;

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use super::{ChatRequest, ChatResponse, ChatRole, DeltaSink, LlmProvider};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliKind {
    Claude,
    Codex,
}

pub struct CliProvider {
    kind: CliKind,
}

impl CliProvider {
    pub fn new(kind: CliKind) -> Self {
        Self { kind }
    }

    fn binary(&self) -> &'static str {
        match self.kind {
            CliKind::Claude => "claude",
            CliKind::Codex => "codex",
        }
    }

    pub fn locate(&self) -> anyhow::Result<std::path::PathBuf> {
        which::which(self.binary()).map_err(|_| {
            anyhow::anyhow!(
                "`{}` was not found on PATH. Install it and sign in once from a terminal.",
                self.binary()
            )
        })
    }
}

/// The CLIs take a single prompt, so the transcript is flattened into text.
fn transcript(req: &ChatRequest, include_system: bool) -> String {
    let mut s = String::new();
    if include_system && !req.system.is_empty() {
        s.push_str("<instructions>\n");
        s.push_str(&req.system);
        s.push_str("\n</instructions>\n\n");
    }
    let history = &req.messages[..req.messages.len().saturating_sub(1)];
    if !history.is_empty() {
        s.push_str("<conversation>\n");
        for m in history {
            match m.role {
                ChatRole::User => s.push_str(&format!("User: {}\n", m.content)),
                ChatRole::Assistant if !m.content.is_empty() => {
                    s.push_str(&format!("Assistant: {}\n", m.content))
                }
                _ => {}
            }
        }
        s.push_str("</conversation>\n\n");
    }
    if let Some(last) = req.messages.last() {
        s.push_str(&last.content);
    }
    s
}

#[async_trait]
impl LlmProvider for CliProvider {
    async fn chat(
        &self,
        req: ChatRequest,
        on_delta: DeltaSink<'_>,
    ) -> anyhow::Result<ChatResponse> {
        let bin = self.locate()?;
        let workdir = std::env::temp_dir().join("opencue-cli");
        std::fs::create_dir_all(&workdir)?;
        let last_message_file = workdir.join(format!("codex-{}.txt", crate::db::new_id()));

        let mut cmd = Command::new(&bin);
        cmd.current_dir(&workdir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let prompt = match self.kind {
            CliKind::Claude => {
                cmd.args([
                    "-p",
                    "--output-format",
                    "stream-json",
                    "--verbose",
                    "--include-partial-messages",
                ]);
                if !req.system.is_empty() {
                    cmd.arg("--append-system-prompt").arg(&req.system);
                }
                if !req.model.is_empty() && req.model != "default" {
                    cmd.arg("--model").arg(&req.model);
                }
                transcript(&req, false)
            }
            CliKind::Codex => {
                cmd.args([
                    "exec",
                    "--skip-git-repo-check",
                    "--json",
                    "--output-last-message",
                ])
                .arg(&last_message_file);
                if !req.model.is_empty() && req.model != "default" {
                    cmd.arg("-m").arg(&req.model);
                }
                cmd.arg("-");
                transcript(&req, true)
            }
        };

        let mut child = cmd.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).await?;
            stdin.shutdown().await?;
        }
        let stdout = child.stdout.take().expect("piped stdout");
        let mut lines = BufReader::new(stdout).lines();
        let mut out = ChatResponse::default();
        let mut streamed = String::new();
        let mut final_text: Option<String> = None;

        while let Some(line) = lines.next_line().await? {
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            match self.kind {
                CliKind::Claude => match v["type"].as_str() {
                    Some("stream_event") => {
                        if v.pointer("/event/delta/type").and_then(Value::as_str)
                            == Some("text_delta")
                        {
                            if let Some(t) = v.pointer("/event/delta/text").and_then(Value::as_str)
                            {
                                streamed.push_str(t);
                                on_delta(t);
                            }
                        }
                    }
                    Some("result") => {
                        final_text = v["result"].as_str().map(str::to_string);
                        out.input_tokens = v
                            .pointer("/usage/input_tokens")
                            .and_then(Value::as_i64)
                            .unwrap_or(0);
                        out.output_tokens = v
                            .pointer("/usage/output_tokens")
                            .and_then(Value::as_i64)
                            .unwrap_or(0);
                        if v["is_error"].as_bool() == Some(true) {
                            anyhow::bail!("claude: {}", final_text.unwrap_or_default());
                        }
                    }
                    _ => {}
                },
                CliKind::Codex => {
                    let item = v.get("item").or_else(|| v.get("msg"));
                    let is_message = item
                        .and_then(|i| i.get("type").or_else(|| i.get("item_type")))
                        .and_then(Value::as_str)
                        .is_some_and(|t| t == "agent_message" || t == "assistant_message");
                    if is_message {
                        if let Some(t) = item
                            .and_then(|i| i.get("text").or_else(|| i.get("message")))
                            .and_then(Value::as_str)
                        {
                            if !streamed.is_empty() {
                                streamed.push_str("\n\n");
                                on_delta("\n\n");
                            }
                            streamed.push_str(t);
                            on_delta(t);
                        }
                    }
                    if let Some(usage) = v.get("usage") {
                        out.input_tokens =
                            usage["input_tokens"].as_i64().unwrap_or(out.input_tokens);
                        out.output_tokens =
                            usage["output_tokens"].as_i64().unwrap_or(out.output_tokens);
                    }
                }
            }
        }

        let output = child.wait_with_output().await?;
        if self.kind == CliKind::Codex {
            if let Ok(text) = std::fs::read_to_string(&last_message_file) {
                final_text = Some(text.trim().to_string());
            }
            let _ = std::fs::remove_file(&last_message_file);
        }
        if !output.status.success() && streamed.is_empty() && final_text.is_none() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "{} exited with {}: {}",
                self.binary(),
                output.status,
                stderr.chars().take(600).collect::<String>()
            );
        }
        out.text = final_text.filter(|t| !t.is_empty()).unwrap_or(streamed);
        Ok(out)
    }

    async fn list_models(&self) -> anyhow::Result<Vec<String>> {
        self.locate()?;
        Ok(match self.kind {
            CliKind::Claude => vec![
                "default".into(),
                "opus".into(),
                "sonnet".into(),
                "haiku".into(),
            ],
            CliKind::Codex => vec!["default".into()],
        })
    }

    fn supports_tools(&self) -> bool {
        false
    }
}
