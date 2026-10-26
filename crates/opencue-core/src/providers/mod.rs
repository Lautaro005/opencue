//! LLM provider abstraction. Every backend implements [`LlmProvider`]; the catalog below
//! describes the built-in kinds and their defaults.

mod anthropic;
mod cli;
mod openai;

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::ToolCall;

pub use anthropic::AnthropicProvider;
pub use cli::{CliKind, CliProvider};
pub use openai::OpenAiProvider;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Openai,
    Anthropic,
    Xai,
    Openrouter,
    #[serde(alias = "cheapestinference")]
    Cheaperinference,
    Ollama,
    OllamaCloud,
    Lmstudio,
    OpenaiCompatible,
    CodexCli,
    ClaudeCli,
    GrokCli,
    MuseCli,
    Gemini,
    Muse,
    MistralApi,
    Groq,
    VibeCli,
    AntigravityCli,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiStyle {
    Openai,
    Anthropic,
    Cli,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub kind: ProviderKind,
    pub label: &'static str,
    pub api: ApiStyle,
    pub default_base_url: Option<&'static str>,
    pub default_model: Option<&'static str>,
    pub requires_key: bool,
    pub supports_tools: bool,
    pub note: &'static str,
}

pub const CATALOG: &[ProviderInfo] = &[
    ProviderInfo {
        kind: ProviderKind::Openai,
        label: "OpenAI (ChatGPT)",
        api: ApiStyle::Openai,
        default_base_url: Some("https://api.openai.com/v1"),
        default_model: Some("gpt-5"),
        requires_key: true,
        supports_tools: true,
        note: "API key from platform.openai.com",
    },
    ProviderInfo {
        kind: ProviderKind::Anthropic,
        label: "Anthropic (Claude)",
        api: ApiStyle::Anthropic,
        default_base_url: Some("https://api.anthropic.com"),
        default_model: Some("claude-sonnet-5-5"),
        requires_key: true,
        supports_tools: true,
        note: "API key from console.anthropic.com",
    },
    ProviderInfo {
        kind: ProviderKind::Xai,
        label: "xAI (Grok)",
        api: ApiStyle::Openai,
        default_base_url: Some("https://api.x.ai/v1"),
        default_model: Some("grok-4"),
        requires_key: true,
        supports_tools: true,
        note: "API key from console.x.ai",
    },
    ProviderInfo {
        kind: ProviderKind::Gemini,
        label: "Google (Gemini API)",
        api: ApiStyle::Openai,
        default_base_url: Some("https://generativelanguage.googleapis.com/v1beta/openai"),
        default_model: Some("gemini-3-flash-preview"),
        requires_key: true,
        supports_tools: true,
        note: "API key from aistudio.google.com (Gemini's OpenAI-compatible endpoint)",
    },
    ProviderInfo {
        kind: ProviderKind::Muse,
        label: "Meta (Muse API)",
        api: ApiStyle::Openai,
        default_base_url: Some("https://api.meta.ai/v1"),
        default_model: Some("muse-spark-1.3"),
        requires_key: true,
        supports_tools: true,
        note: "API key from dev.meta.ai (Muse Spark)",
    },
    ProviderInfo {
        kind: ProviderKind::MistralApi,
        label: "Mistral API",
        api: ApiStyle::Openai,
        default_base_url: Some("https://api.mistral.ai/v1"),
        default_model: Some("mistral-large-latest"),
        requires_key: true,
        supports_tools: true,
        note: "API key from console.mistral.ai",
    },
    ProviderInfo {
        kind: ProviderKind::Groq,
        label: "Groq",
        api: ApiStyle::Openai,
        default_base_url: Some("https://api.groq.com/openai/v1"),
        default_model: Some("llama-3.3-70b-versatile"),
        requires_key: true,
        supports_tools: true,
        note: "API key from console.groq.com (fast open models)",
    },
    ProviderInfo {
        kind: ProviderKind::Openrouter,
        label: "OpenRouter",
        api: ApiStyle::Openai,
        default_base_url: Some("https://openrouter.ai/api/v1"),
        default_model: Some("openrouter/auto"),
        requires_key: true,
        supports_tools: true,
        note: "Hundreds of models behind one key",
    },
    ProviderInfo {
        kind: ProviderKind::Cheaperinference,
        label: "CheaperInference",
        api: ApiStyle::Openai,
        default_base_url: Some("https://api.cheaperinference.com/v1"),
        default_model: None,
        requires_key: true,
        supports_tools: true,
        note: "Flat-rate open models (Kimi, GLM, DeepSeek…). Check the base URL in your dashboard.",
    },
    ProviderInfo {
        kind: ProviderKind::Ollama,
        label: "Ollama (local)",
        api: ApiStyle::Openai,
        default_base_url: Some("http://localhost:11434/v1"),
        default_model: Some("llama3.2"),
        requires_key: false,
        supports_tools: true,
        note: "Runs models on this machine",
    },
    ProviderInfo {
        kind: ProviderKind::OllamaCloud,
        label: "Ollama Cloud",
        api: ApiStyle::Openai,
        default_base_url: Some("https://ollama.com/v1"),
        default_model: None,
        requires_key: true,
        supports_tools: true,
        note: "API key from ollama.com",
    },
    ProviderInfo {
        kind: ProviderKind::Lmstudio,
        label: "LM Studio",
        api: ApiStyle::Openai,
        default_base_url: Some("http://localhost:1234/v1"),
        default_model: None,
        requires_key: false,
        supports_tools: true,
        note: "Start the local server in LM Studio first",
    },
    ProviderInfo {
        kind: ProviderKind::OpenaiCompatible,
        label: "Custom (OpenAI-compatible)",
        api: ApiStyle::Openai,
        default_base_url: None,
        default_model: None,
        requires_key: false,
        supports_tools: true,
        note: "Any server that speaks /v1/chat/completions",
    },
    ProviderInfo {
        kind: ProviderKind::CodexCli,
        label: "ChatGPT via Codex CLI",
        api: ApiStyle::Cli,
        default_base_url: None,
        default_model: None,
        requires_key: false,
        supports_tools: false,
        note: "Uses your installed `codex` CLI and its ChatGPT login. OpenCue tools are not available.",
    },
    ProviderInfo {
        kind: ProviderKind::GrokCli,
        label: "Grok via Grok Build CLI",
        api: ApiStyle::Cli,
        default_base_url: None,
        default_model: None,
        requires_key: false,
        supports_tools: false,
        note: "Uses your installed `grok` CLI (xAI Grok Build) and its login. Answers in text; OpenCue tools are not available.",
    },
    ProviderInfo {
        kind: ProviderKind::MuseCli,
        label: "Muse via Muse Code CLI",
        api: ApiStyle::Cli,
        default_base_url: None,
        default_model: None,
        requires_key: false,
        supports_tools: false,
        note: "Uses your installed `muse` CLI (Meta Muse Code) and its login. Answers in text; OpenCue tools are not available.",
    },
    ProviderInfo {
        kind: ProviderKind::VibeCli,
        label: "Mistral via Vibe CLI",
        api: ApiStyle::Cli,
        default_base_url: None,
        default_model: None,
        requires_key: false,
        supports_tools: false,
        note: "Uses your installed `vibe` CLI (Mistral Vibe) and its login or MISTRAL_API_KEY. Answers in text; OpenCue tools are not available.",
    },
    ProviderInfo {
        kind: ProviderKind::AntigravityCli,
        label: "Gemini via Antigravity CLI",
        api: ApiStyle::Cli,
        default_base_url: None,
        default_model: None,
        requires_key: false,
        supports_tools: false,
        note: "Uses your installed `agy` CLI (Google Antigravity) and its Google login. Answers in text; OpenCue tools are not available.",
    },
    ProviderInfo {
        kind: ProviderKind::ClaudeCli,
        label: "Claude via Claude Code",
        api: ApiStyle::Cli,
        default_base_url: None,
        default_model: None,
        requires_key: false,
        supports_tools: false,
        note: "Uses your installed `claude` CLI and its Claude login. OpenCue tools are not available.",
    },
];

pub fn info(kind: ProviderKind) -> &'static ProviderInfo {
    CATALOG
        .iter()
        .find(|p| p.kind == kind)
        .expect("every kind is in the catalog")
}

/// A configured provider instance (stored in SQLite; the secret lives in the keyring).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub kind: ProviderKind,
    pub name: String,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
    pub enabled: bool,
    #[serde(default)]
    pub has_key: bool,
}

impl ProviderConfig {
    pub fn effective_base_url(&self) -> Option<String> {
        self.base_url
            .clone()
            .filter(|s| !s.trim().is_empty())
            // CheaperInference used to be saved under its old, misspelled address.
            .map(|u| u.replace("cheapestinference", "cheaperinference"))
            .or_else(|| info(self.kind).default_base_url.map(str::to_string))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
    /// Pictures shared by the user, for models that can see them.
    #[serde(default)]
    pub images: Vec<ImageData>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageData {
    pub mime: String,
    /// Base64-encoded bytes.
    pub data: String,
}

impl ChatMessage {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: text.into(),
            tool_calls: vec![],
            tool_call_id: None,
            images: vec![],
        }
    }

    pub fn user_with_images(text: impl Into<String>, images: Vec<ImageData>) -> Self {
        Self {
            images,
            ..Self::user(text)
        }
    }

    pub fn assistant(text: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: text.into(),
            tool_calls,
            tool_call_id: None,
            images: vec![],
        }
    }

    pub fn tool(call_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Tool,
            content: text.into(),
            tool_calls: vec![],
            tool_call_id: Some(call_id.into()),
            images: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// What a model has written so far of one tool call, while it is still being generated.
#[derive(Debug, Clone)]
pub struct ToolProgress {
    pub name: String,
    /// Characters of arguments received so far.
    pub chars: usize,
    /// The first part of the arguments (enough to read a title from).
    pub head: String,
}

/// Receives [`ToolProgress`] as a streamed tool call grows.
#[derive(Clone)]
pub struct ToolSink(pub Arc<dyn Fn(ToolProgress) + Send + Sync>);

impl std::fmt::Debug for ToolSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ToolSink")
    }
}

/// Throttles the progress of streamed tool calls: one report when a call starts, then one for
/// every few KB, so a 200 KB page doesn't send thousands of events.
#[derive(Default)]
pub struct ToolWatch {
    calls: std::collections::HashMap<u64, WatchedCall>,
}

#[derive(Default)]
struct WatchedCall {
    name: String,
    args: String,
    reported: usize,
    size: usize,
    started: bool,
}

const WATCH_HEAD: usize = 600;
const WATCH_STEP: usize = 2_048;

impl ToolWatch {
    pub fn feed(&mut self, sink: &Option<ToolSink>, index: u64, name: &str, args: &str) {
        let Some(sink) = sink else { return };
        let c = self.calls.entry(index).or_default();
        c.name.push_str(name);
        c.size += args.len();
        if c.args.len() < WATCH_HEAD {
            let room = WATCH_HEAD - c.args.len();
            c.args.extend(args.chars().take(room));
        }
        if c.name.is_empty() {
            return;
        }
        if !c.started || c.size >= c.reported + WATCH_STEP {
            c.started = true;
            c.reported = c.size;
            (sink.0)(ToolProgress {
                name: c.name.clone(),
                chars: c.size,
                head: c.args.clone(),
            });
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ChatRequest {
    pub model: String,
    pub system: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolSpec>,
    pub temperature: Option<f32>,
    /// Set for agent turns, so CLI providers can hand OpenCue's tools to the CLI over MCP.
    pub agent_id: Option<String>,
    /// MCP servers (VMPal machines) a CLI provider should also hand to its CLI.
    pub mcp_extra: Vec<ExtraMcp>,
    /// Told how far a tool call has got while the model is still writing it (the UI shows it).
    pub on_tool: Option<ToolSink>,
    /// The folder a CLI provider runs in (a coding group's repository or worktree).
    pub cwd: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ExtraMcp {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct ChatResponse {
    /// Rate-limit readings a subscription CLI reported while it ran.
    pub rate_limits: Vec<Value>,
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub input_tokens: i64,
    pub output_tokens: i64,
}

/// Reasoning travels inside the text as `<think>…</think>`; the chat shows it folded, and it is
/// dropped from the history, spoken replies and memory.
pub fn strip_think(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("<think>") {
        out.push_str(&rest[..i]);
        match rest[i..].find("</think>") {
            Some(j) => rest = &rest[i + j + "</think>".len()..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out.trim_start().to_string()
}

/// Wraps a model's reasoning stream in `<think>` tags as it arrives.
#[derive(Default)]
pub struct ThinkStream {
    open: bool,
}

impl ThinkStream {
    /// The text to add to the reply for a piece of reasoning.
    pub fn reasoning(&mut self, t: &str) -> String {
        if self.open {
            t.to_string()
        } else {
            self.open = true;
            format!("<think>{t}")
        }
    }

    /// The text to add for a piece of the answer (closes the reasoning first).
    pub fn answer(&mut self, t: &str) -> String {
        if self.open {
            self.open = false;
            format!("</think>{t}")
        } else {
            t.to_string()
        }
    }

    /// Closes reasoning that ended without an answer (a tool call followed).
    pub fn finish(&mut self) -> String {
        if self.open {
            self.open = false;
            "</think>".to_string()
        } else {
            String::new()
        }
    }
}

/// Receives streamed text as it arrives.
pub type DeltaSink<'a> = &'a (dyn Fn(&str) + Send + Sync);

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, req: ChatRequest, on_delta: DeltaSink<'_>)
        -> anyhow::Result<ChatResponse>;
    async fn list_models(&self) -> anyhow::Result<Vec<String>>;
    fn supports_tools(&self) -> bool {
        true
    }
}

pub fn build(
    config: &ProviderConfig,
    api_key: Option<String>,
) -> anyhow::Result<Arc<dyn LlmProvider>> {
    let meta = info(config.kind);
    if meta.requires_key && api_key.as_deref().unwrap_or("").is_empty() {
        anyhow::bail!(
            "{} needs an API key. Add it in Settings → Models.",
            config.name
        );
    }
    Ok(match meta.api {
        ApiStyle::Openai => {
            let base = config
                .effective_base_url()
                .ok_or_else(|| anyhow::anyhow!("{} has no base URL configured", config.name))?;
            Arc::new(OpenAiProvider::new(base, api_key, config.kind))
        }
        ApiStyle::Anthropic => Arc::new(AnthropicProvider::new(
            config
                .effective_base_url()
                .unwrap_or_else(|| "https://api.anthropic.com".into()),
            api_key.unwrap_or_default(),
        )),
        ApiStyle::Cli => Arc::new(CliProvider::new(match config.kind {
            ProviderKind::CodexCli => CliKind::Codex,
            ProviderKind::GrokCli => CliKind::Grok,
            ProviderKind::MuseCli => CliKind::Muse,
            ProviderKind::VibeCli => CliKind::Vibe,
            ProviderKind::AntigravityCli => CliKind::Antigravity,
            _ => CliKind::Claude,
        })),
    })
}

pub(crate) fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("OpenCue/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(20))
        .build()
        .expect("http client")
}

/// Turn a non-2xx response into a readable error.
pub(crate) async fn error_for_status(resp: reqwest::Response) -> anyhow::Result<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    let detail = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .or_else(|| v.get("error"))
                .or_else(|| v.get("message"))
                .map(|m| {
                    m.as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| m.to_string())
                })
        })
        .unwrap_or(body);
    anyhow::bail!(
        "provider returned {status}: {}",
        detail.chars().take(600).collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_and_muse_are_in_the_catalog() {
        let g = info(ProviderKind::Gemini);
        assert_eq!(g.api, ApiStyle::Openai);
        assert!(g.requires_key);
        assert_eq!(
            g.default_base_url,
            Some("https://generativelanguage.googleapis.com/v1beta/openai")
        );
        assert_eq!(
            info(ProviderKind::Muse).default_base_url,
            Some("https://api.meta.ai/v1")
        );
        assert_eq!(info(ProviderKind::MuseCli).api, ApiStyle::Cli);
        let kind: ProviderKind = serde_json::from_str("\"muse_cli\"").unwrap();
        assert_eq!(kind, ProviderKind::MuseCli);
    }

    #[test]
    fn old_cheapestinference_providers_still_load_and_move_to_the_right_address() {
        let kind: ProviderKind = serde_json::from_str("\"cheapestinference\"").unwrap();
        assert_eq!(kind, ProviderKind::Cheaperinference);
        let cfg = ProviderConfig {
            id: "p".into(),
            kind,
            name: "CheaperInference".into(),
            base_url: Some("https://api.cheapestinference.com/v1".into()),
            default_model: None,
            enabled: true,
            has_key: true,
        };
        assert_eq!(
            cfg.effective_base_url().as_deref(),
            Some("https://api.cheaperinference.com/v1")
        );
        assert_eq!(
            info(ProviderKind::Cheaperinference).label,
            "CheaperInference"
        );
    }

    #[test]
    fn mistral_groq_vibe_and_antigravity_are_in_the_catalog() {
        assert_eq!(
            info(ProviderKind::MistralApi).default_base_url,
            Some("https://api.mistral.ai/v1")
        );
        assert_eq!(
            info(ProviderKind::Groq).default_base_url,
            Some("https://api.groq.com/openai/v1")
        );
        assert_eq!(info(ProviderKind::VibeCli).api, ApiStyle::Cli);
        assert_eq!(info(ProviderKind::AntigravityCli).api, ApiStyle::Cli);
        let kind: ProviderKind = serde_json::from_str("\"mistral_api\"").unwrap();
        assert_eq!(kind, ProviderKind::MistralApi);
        let kind: ProviderKind = serde_json::from_str("\"antigravity_cli\"").unwrap();
        assert_eq!(kind, ProviderKind::AntigravityCli);
    }

    #[test]
    fn grok_cli_is_in_the_catalog() {
        assert_eq!(info(ProviderKind::GrokCli).api, ApiStyle::Cli);
    }
}

#[cfg(test)]
mod think_tests {
    use super::*;

    #[test]
    fn reasoning_is_wrapped_and_stripped() {
        let mut t = ThinkStream::default();
        let text = format!(
            "{}{}{}",
            t.reasoning("hmm "),
            t.reasoning("ok"),
            t.answer("Hi")
        );
        assert_eq!(text, "<think>hmm ok</think>Hi");
        assert_eq!(strip_think(&text), "Hi");
        assert_eq!(strip_think("<think>unfinished"), "");
        assert_eq!(strip_think("no tags"), "no tags");
        assert_eq!(t.finish(), "");
    }
}
