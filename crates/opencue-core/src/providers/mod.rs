//! LLM provider abstraction. Every backend implements [`LlmProvider`]; the catalog below
//! describes the built-in kinds and their defaults.

mod anthropic;
mod cli;
mod openai;

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

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
    Cheapestinference,
    Ollama,
    OllamaCloud,
    Lmstudio,
    OpenaiCompatible,
    CodexCli,
    ClaudeCli,
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
        kind: ProviderKind::Cheapestinference,
        label: "CheapestInference",
        api: ApiStyle::Openai,
        default_base_url: Some("https://api.cheapestinference.com/v1"),
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
}

impl ChatMessage {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: text.into(),
            tool_calls: vec![],
            tool_call_id: None,
        }
    }

    pub fn assistant(text: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: text.into(),
            tool_calls,
            tool_call_id: None,
        }
    }

    pub fn tool(call_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Tool,
            content: text.into(),
            tool_calls: vec![],
            tool_call_id: Some(call_id.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub system: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolSpec>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct ChatResponse {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub input_tokens: i64,
    pub output_tokens: i64,
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
