//! Plain data types shared by the core, the desktop app and the CLI.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    /// Sensitive tools (email, deletes, host commands, destructive MCP tools) ask first.
    #[default]
    Ask,
    /// Everything runs without asking.
    Autonomous,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentSettings {
    #[serde(default)]
    pub approval_mode: ApprovalMode,
    #[serde(default = "yes")]
    pub web_access: bool,
    /// When true every enabled MCP server is available; otherwise only `mcp_servers`.
    #[serde(default = "yes")]
    pub mcp_all: bool,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    #[serde(default = "yes")]
    pub use_global_memory: bool,
    /// Extract durable facts into memory automatically after each reply.
    #[serde(default)]
    pub auto_memory: bool,
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Where `run_command` and the file tools run.
    #[serde(default)]
    pub computer: crate::computer::ComputerBackend,
    /// Skills listed in the prompt and loadable with `use_skill`.
    #[serde(default = "yes")]
    pub skills: bool,
    /// Composio actions (needs a Composio API key in Settings → Apps).
    #[serde(default)]
    pub composio: bool,
    /// ElevenLabs voice id for spoken replies; empty uses the app default.
    #[serde(default)]
    pub voice_id: Option<String>,
}

impl Default for AgentSettings {
    fn default() -> Self {
        serde_json::from_str("{}").expect("defaults deserialize")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Avatar {
    /// Face id drawn on the shape (older versions stored an emoji here).
    #[serde(alias = "emoji")]
    pub face: String,
    pub color: String,
    pub shape: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub avatar: Avatar,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub settings: AgentSettings,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewAgent {
    pub name: String,
    pub avatar: Option<Avatar>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub instructions: Option<String>,
    pub settings: Option<AgentSettings>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentPatch {
    pub name: Option<String>,
    pub avatar: Option<Avatar>,
    /// `Some(None)` clears the provider.
    #[serde(default, with = "double_option")]
    pub provider_id: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    pub model: Option<Option<String>>,
    pub settings: Option<AgentSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMember {
    pub agent_id: String,
    pub is_owner: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub max_auto_turns: i64,
    pub members: Vec<GroupMember>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
    Tool,
    Event,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
            Role::Event => "event",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "assistant" => Role::Assistant,
            "tool" => Role::Tool,
            "event" => Role::Event,
            _ => Role::User,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub thread_id: String,
    pub role: Role,
    pub agent_id: Option<String>,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub status: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadKind {
    Agent,
    Group,
}

/// One row of the sidebar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSummary {
    pub kind: ThreadKind,
    pub id: String,
    pub name: String,
    pub avatars: Vec<Avatar>,
    pub last_message: Option<String>,
    pub last_at: Option<String>,
    pub busy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub transport: McpTransport,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: std::collections::BTreeMap<String, String>,
    pub url: Option<String>,
    pub headers: std::collections::BTreeMap<String, String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpTransport {
    Stdio,
    Http,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerStatus {
    pub id: String,
    pub name: String,
    pub state: String, // stopped | connecting | ready | error
    pub error: Option<String>,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageRow {
    pub agent_id: Option<String>,
    pub agent_name: Option<String>,
    pub messages: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
}

/// Serde helper so patches can distinguish "absent" from "set to null".
mod double_option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer, T: Serialize>(
        v: &Option<Option<T>>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        match v {
            None => s.serialize_none(),
            Some(inner) => inner.serialize(s),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
        d: D,
    ) -> Result<Option<Option<T>>, D::Error> {
        Option::<T>::deserialize(d).map(Some)
    }
}
