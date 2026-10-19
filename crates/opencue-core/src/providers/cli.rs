//! "Backend CLI" providers: run the user's own logged-in `claude` / `codex` / `grok` binary so
//! their subscription is used through the official client. OpenCue never sees or stores those
//! tokens.

use std::process::Stdio;

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use super::{ChatRequest, ChatResponse, ChatRole, DeltaSink, ExtraMcp, LlmProvider};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliKind {
    Claude,
    Codex,
    Grok,
}

pub struct CliProvider {
    kind: CliKind,
}

impl CliProvider {
    pub fn new(kind: CliKind) -> Self {
        Self { kind }
    }

    /// How the CLI starts OpenCue's MCP server for an agent: this same executable with
    /// `mcp-serve --agent <id>` (both the desktop app and `opencue-cli` understand it).
    fn mcp_server(agent_id: &str) -> Option<(String, Vec<String>)> {
        // Inside an AppImage the binary lives on a temporary mount; the AppImage itself is stable.
        let exe = std::env::var_os("APPIMAGE")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::current_exe().ok())?;
        Some((
            exe.to_string_lossy().to_string(),
            vec!["mcp-serve".into(), "--agent".into(), agent_id.to_string()],
        ))
    }

    fn binary(&self) -> &'static str {
        match self.kind {
            CliKind::Claude => "claude",
            CliKind::Codex => "codex",
            CliKind::Grok => "grok",
        }
    }

    pub fn locate(&self) -> anyhow::Result<std::path::PathBuf> {
        which::which(self.binary()).map_err(|_| {
            anyhow::anyhow!(
                "`{}` was not found. Install it and sign in once from a terminal ({}), then try again.",
                self.binary(),
                match self.kind {
                    CliKind::Claude => "`claude`, then /login",
                    CliKind::Codex => "`codex login`",
                    CliKind::Grok => "`grok login`, or set XAI_API_KEY",
                }
            )
        })
    }
}

/// Models offered for a CLI provider. The CLIs have no "list models" command, so this is what
/// each one accepts: aliases and ids for Claude, the visible models in Codex's own cache, and the
/// default plus any `[model.*]` you defined in Grok's config.
fn cli_models(kind: CliKind) -> Vec<String> {
    let mut out = vec!["default".to_string()];
    match kind {
        CliKind::Claude => {
            out.extend(
                [
                    "opus",
                    "sonnet",
                    "haiku",
                    "claude-fable-5-1",
                    "claude-fable-5",
                    "claude-opus-5-5",
                    "claude-opus-5",
                    "claude-sonnet-5-5",
                    "claude-sonnet-5",
                    "claude-haiku-5-5",
                    "claude-haiku-4-5",
                ]
                .map(String::from),
            );
            out.extend(claude_extra_models());
        }
        CliKind::Codex => {
            let home = std::env::var_os("CODEX_HOME")
                .map(std::path::PathBuf::from)
                .or_else(|| directories::UserDirs::new().map(|d| d.home_dir().join(".codex")));
            let cache =
                home.and_then(|h| std::fs::read_to_string(h.join("models_cache.json")).ok());
            if let Some(v) = cache.and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
                for m in v["models"].as_array().into_iter().flatten() {
                    let listed = m["visibility"].as_str().is_none_or(|v| v == "list");
                    if let (true, Some(slug)) = (listed, m["slug"].as_str()) {
                        out.push(slug.to_string());
                    }
                }
            }
        }
        CliKind::Grok => {
            out.extend(["grok-4.7", "grok-4.6", "grok-4.5"].map(String::from));
            let cfg = directories::UserDirs::new()
                .map(|d| d.home_dir().join(".grok").join("config.toml"))
                .and_then(|p| std::fs::read_to_string(p).ok())
                .and_then(|t| t.parse::<toml::Table>().ok());
            if let Some(toml::Value::Table(models)) = cfg.as_ref().and_then(|c| c.get("model")) {
                out.extend(models.keys().cloned());
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|m| seen.insert(m.clone()));
    out
}

/// Models Claude Code itself offers beyond the built-in ones: `availableModels` /
/// `customModels` / `extraModels` in its settings.json and the `ANTHROPIC_*MODEL*` variables.
fn claude_extra_models() -> Vec<String> {
    let dir = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| directories::UserDirs::new().map(|d| d.home_dir().join(".claude")));
    let settings = dir
        .and_then(|d| std::fs::read_to_string(d.join("settings.json")).ok())
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .unwrap_or(Value::Null);
    let ok = |id: &str| {
        !id.is_empty()
            && id.len() <= 80
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "._:/-[]".contains(c))
    };
    let mut out = Vec::new();
    for key in ["availableModels", "customModels", "extraModels"] {
        for m in settings[key].as_array().into_iter().flatten() {
            let id = m
                .as_str()
                .or_else(|| m["id"].as_str())
                .or_else(|| m["model"].as_str());
            if let Some(id) = id.filter(|i| ok(i)) {
                out.push(id.to_string());
            }
        }
    }
    let env = |name: &str| {
        settings["env"][name]
            .as_str()
            .map(str::to_string)
            .or_else(|| std::env::var(name).ok())
    };
    for name in [
        "ANTHROPIC_MODEL",
        "ANTHROPIC_CUSTOM_MODEL_OPTION",
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
        "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
    ] {
        if let Some(id) = env(name).map(|s| s.trim().to_string()).filter(|i| ok(i)) {
            out.push(id);
        }
    }
    out
}

/// Add how to sign in again when a CLI says its login is gone or expired.
fn with_login_hint(kind: CliKind, msg: &str) -> String {
    let lower = msg.to_lowercase();
    let auth = [
        "oauth",
        "authenticate",
        "not logged in",
        "login",
        "unauthorized",
        "401",
        "expired",
    ]
    .iter()
    .any(|k| lower.contains(k));
    if !auth {
        return msg.to_string();
    }
    let how = match kind {
        CliKind::Claude => "open a terminal, run `claude` and type /login",
        CliKind::Codex => "open a terminal and run `codex login`",
        CliKind::Grok => "open a terminal and run `grok login` (or set XAI_API_KEY)",
    };
    format!("{msg}\n\nYour sign-in on this computer needs renewing: {how}, then send your message again.")
}

/// Pass OpenCue's extra MCP servers (VMPal machines) to a Claude Code / Codex run.
fn extra_claude(
    servers: &mut serde_json::Map<String, Value>,
    allowed: &mut Vec<String>,
    extra: &[ExtraMcp],
) {
    for x in extra {
        let key = crate::mcp::sanitize(&x.name);
        servers.insert(
            key.clone(),
            serde_json::json!({ "command": x.command, "args": x.args, "env": x.env }),
        );
        allowed.push(format!("mcp__{key}"));
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
                cmd.arg("--no-session-persistence");
                let mut servers = serde_json::Map::new();
                let mut allowed: Vec<String> = vec![];
                if let Some((exe, args)) = req.agent_id.as_deref().and_then(Self::mcp_server) {
                    servers.insert(
                        "opencue".into(),
                        serde_json::json!({ "command": exe, "args": args }),
                    );
                    allowed.push("mcp__opencue".into());
                }
                extra_claude(&mut servers, &mut allowed, &req.mcp_extra);
                if !servers.is_empty() {
                    // Variadic flags go last so they can't swallow anything else.
                    // Only these servers: the CLI's own ones (from ~/.claude) can't be allowed in a
                    // headless run, so calling them fails with a permission error nobody can answer.
                    cmd.arg("--strict-mcp-config")
                        .arg("--mcp-config")
                        .arg(serde_json::json!({ "mcpServers": servers }).to_string())
                        .arg("--allowedTools")
                        .args(&allowed);
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
                if let Some((exe, args)) = req.agent_id.as_deref().and_then(Self::mcp_server) {
                    // `-c key=value` takes TOML values; JSON strings and arrays are valid TOML.
                    cmd.arg("-c")
                        .arg(format!("mcp_servers.opencue.command={}", Value::from(exe)))
                        .arg("-c")
                        .arg(format!("mcp_servers.opencue.args={}", Value::from(args)));
                }
                for x in &req.mcp_extra {
                    let key = crate::mcp::sanitize(&x.name);
                    cmd.arg("-c")
                        .arg(format!(
                            "mcp_servers.{key}.command={}",
                            Value::from(x.command.clone())
                        ))
                        .arg("-c")
                        .arg(format!(
                            "mcp_servers.{key}.args={}",
                            Value::from(x.args.clone())
                        ));
                }
                cmd.arg("-");
                transcript(&req, true)
            }
            CliKind::Grok => {
                // Headless: one prompt, plain text out. Tools stay off (no auto-approve), so it
                // answers in text instead of editing files in the scratch folder.
                cmd.args(["-p"]);
                cmd.args([
                    "--output-format",
                    "plain",
                    "--no-alt-screen",
                    "--no-auto-update",
                ]);
                if !req.model.is_empty() && req.model != "default" {
                    cmd.arg("-m").arg(&req.model);
                }
                let mut p = transcript(&req, true);
                p.insert_str(
                    0,
                    "Answer in plain text only, without using tools or touching files.\n\n",
                );
                cmd.arg(p);
                String::new()
            }
        };

        let mut child = cmd.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).await?;
            stdin.shutdown().await?;
        }
        let mut stdout = child.stdout.take().expect("piped stdout");
        let mut out = ChatResponse::default();
        let mut streamed = String::new();
        let mut final_text: Option<String> = None;

        if self.kind == CliKind::Grok {
            // Plain text: forward it as it arrives, holding back a cut-off UTF-8 character.
            let mut pending: Vec<u8> = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = stdout.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                pending.extend_from_slice(&buf[..n]);
                let valid = match std::str::from_utf8(&pending) {
                    Ok(_) => pending.len(),
                    Err(e) => e.valid_up_to(),
                };
                if valid > 0 {
                    let text = String::from_utf8_lossy(&pending[..valid]).to_string();
                    streamed.push_str(&text);
                    on_delta(&text);
                    pending.drain(..valid);
                }
            }
            if !pending.is_empty() {
                let text = String::from_utf8_lossy(&pending).to_string();
                streamed.push_str(&text);
                on_delta(&text);
            }
            let output = child.wait_with_output().await?;
            if !output.status.success() && streamed.trim().is_empty() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!(with_login_hint(
                    CliKind::Grok,
                    &format!(
                        "grok exited with {}: {}",
                        output.status,
                        stderr.chars().take(600).collect::<String>()
                    )
                ));
            }
            out.text = streamed.trim().to_string();
            return Ok(out);
        }
        let mut lines = BufReader::new(stdout).lines();

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
                            anyhow::bail!(with_login_hint(
                                CliKind::Claude,
                                &format!("claude: {}", final_text.unwrap_or_default())
                            ));
                        }
                    }
                    _ => {}
                },
                CliKind::Grok => {}
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
            anyhow::bail!(with_login_hint(
                self.kind,
                &format!(
                    "{} exited with {}: {}",
                    self.binary(),
                    output.status,
                    stderr.chars().take(600).collect::<String>()
                )
            ));
        }
        out.text = final_text.filter(|t| !t.is_empty()).unwrap_or(streamed);
        Ok(out)
    }

    async fn list_models(&self) -> anyhow::Result<Vec<String>> {
        self.locate()?;
        Ok(cli_models(self.kind))
    }

    fn supports_tools(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_logins_say_how_to_sign_in_again() {
        let msg = with_login_hint(
            CliKind::Claude,
            "claude: Failed to authenticate: OAuth session expired and could not be refreshed",
        );
        assert!(msg.contains("run `claude` and type /login"));
        assert!(with_login_hint(CliKind::Codex, "codex login required").contains("codex login"));
        // Other errors are left alone.
        assert_eq!(
            with_login_hint(CliKind::Grok, "rate limited"),
            "rate limited"
        );
    }

    #[test]
    fn claude_models_include_the_current_ids_once() {
        let models = cli_models(CliKind::Claude);
        assert_eq!(models[0], "default");
        assert!(models.contains(&"claude-opus-5-5".to_string()));
        let mut sorted = models.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), models.len());
    }
}
