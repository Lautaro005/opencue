//! Plain data types shared by the core, the desktop app and the CLI.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    /// Nothing is executed: no commands, no writing files, no email, no automations. Reading,
    /// searching and answering still work.
    ReadOnly,
    /// Sensitive tools (email, deletes, host commands, destructive MCP tools) ask first.
    #[default]
    Ask,
    /// Only the critical ones ask: sending email, deleting, destructive commands, anything that
    /// runs on its own later. The rest runs.
    Critical,
    /// Everything runs without asking ("allow all").
    #[serde(alias = "allow_all")]
    Autonomous,
}

impl ApprovalMode {
    /// What happens to an action of this weight in this mode.
    pub fn verdict(&self, critical: bool, runs_things: bool) -> Verdict {
        match self {
            ApprovalMode::Autonomous => Verdict::Allow,
            ApprovalMode::ReadOnly if runs_things => Verdict::Block,
            ApprovalMode::ReadOnly => Verdict::Allow,
            ApprovalMode::Ask => Verdict::Ask,
            ApprovalMode::Critical if critical => Verdict::Ask,
            ApprovalMode::Critical => Verdict::Allow,
        }
    }
}

/// What to do with a tool call that needs a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Ask,
    Block,
}

/// Whether a shell command can do damage that is hard to undo.
pub fn dangerous_command(cmd: &str) -> bool {
    let c = format!(
        " {} ",
        cmd.to_lowercase()
            .replace(['\n', ';', '&', '|', '(', ')'], " ")
    );
    const WORDS: &[&str] = &[
        " rm ",
        " rmdir ",
        " sudo ",
        " su ",
        " dd ",
        " mkfs",
        " shred ",
        " chmod ",
        " chown ",
        " kill ",
        " killall ",
        " pkill ",
        " shutdown ",
        " reboot ",
        " mv ",
        " truncate ",
        " format ",
        " del ",
        " rd ",
        " rmdir /s",
        " drop ",
        " delete ",
        " ssh ",
        " scp ",
        " launchctl ",
        " systemctl ",
        " crontab ",
    ];
    const PHRASES: &[&str] = &[
        "git push",
        "git reset --hard",
        "git clean",
        "git checkout --",
        "git branch -d",
        "git branch -D",
        "--force",
        "npm publish",
        "cargo publish",
        "docker rm",
        "docker system prune",
        "docker volume rm",
        "gh pr merge",
        "gh release",
        "gh repo delete",
        "curl http",
        "wget http",
        "| sh",
        "| bash",
        "> /dev/",
        "brew uninstall",
        "pip uninstall",
        "npm uninstall",
    ];
    WORDS.iter().any(|w| c.contains(w)) || PHRASES.iter().any(|p| c.contains(&p.to_lowercase()))
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
    /// Extract durable facts into memory automatically after each reply (on by default).
    #[serde(default = "yes")]
    pub auto_memory: bool,
    /// Keep the agent's instructions and soul up to date from the conversation (on by default).
    #[serde(default = "yes")]
    pub auto_profile: bool,
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Where `run_command` and the file tools run.
    #[serde(default)]
    pub computer: crate::computer::ComputerBackend,
    /// Let the agent drive the built-in browser (Computer panel → Browser).
    #[serde(default)]
    pub browser: bool,
    /// Skills listed in the prompt and loadable with `use_skill`.
    #[serde(default = "yes")]
    pub skills: bool,
    /// Catalog apps (Settings → Apps → Catalog) this agent can use, by slug.
    #[serde(default)]
    pub catalog: Vec<String>,
    /// ElevenLabs voice id for spoken replies; empty uses the app default.
    #[serde(default)]
    pub voice_id: Option<String>,
    /// Give agents backed by the `claude` / `codex` CLIs OpenCue's tools through MCP.
    #[serde(default = "yes")]
    pub cli_tools: bool,
    /// Names of the VMPal machines (Settings → Computers) this agent can control.
    #[serde(default)]
    pub vmpal: Vec<String>,
    /// Folder the agent works in. Empty means its own folder inside OpenCue's data directory.
    #[serde(default)]
    pub working_dir: Option<String>,
    /// Options for the agent's Docker computer (the local VM).
    #[serde(default)]
    pub vm: crate::computer::VmOptions,
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
    /// `chat` (the default), `council` (every question goes through the AI council) or `coding`
    /// (a team working on a repository, with its pull requests and changes beside the chat).
    #[serde(default = "default_kind")]
    pub kind: String,
    /// What the kind needs: the repository folder and worktree choice of a coding group, the chair of a council.
    #[serde(default)]
    pub config: serde_json::Value,
}

fn default_kind() -> String {
    "chat".into()
}

pub const GROUP_KINDS: [&str; 3] = ["chat", "council", "coding"];

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
    /// Files shared with this message (by the user, or by an agent with `share_file`).
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

/// A file shared in a chat. The bytes are stored at `files/<id>/<name>` in the data folder.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub size: u64,
    /// Set when an agent made this file with `create_artifact`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<ArtifactRef>,
}

/// Which artifact (and version of it) an attachment is.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtifactRef {
    pub id: String,
    pub version: u32,
    pub kind: String,
    pub title: String,
    /// A canvas: opens in the editable canvas panel.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub canvas: bool,
}

impl Attachment {
    pub fn is_image(&self) -> bool {
        matches!(
            self.mime.as_str(),
            "image/png" | "image/jpeg" | "image/gif" | "image/webp"
        )
    }

    pub fn is_text(&self) -> bool {
        self.mime.starts_with("text/")
            || matches!(
                self.mime.as_str(),
                "application/json" | "application/xml" | "application/x-yaml" | "application/toml"
            )
    }
}

/// A file the user is sending, as it arrives from the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewAttachment {
    pub name: String,
    /// Base64-encoded bytes.
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadKind {
    Agent,
    Group,
}

/// A message found by the sidebar's search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub message_id: String,
    pub thread_id: String,
    pub role: String,
    pub agent_id: Option<String>,
    pub content: String,
    pub created_at: String,
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

#[cfg(test)]
mod permission_tests {
    use super::*;

    #[test]
    fn each_mode_decides_by_weight() {
        use ApprovalMode::*;
        // (critical, runs things)
        assert_eq!(Autonomous.verdict(true, true), Verdict::Allow);
        assert_eq!(ReadOnly.verdict(false, true), Verdict::Block);
        assert_eq!(ReadOnly.verdict(false, false), Verdict::Allow);
        assert_eq!(Ask.verdict(false, true), Verdict::Ask);
        assert_eq!(Critical.verdict(false, true), Verdict::Allow);
        assert_eq!(Critical.verdict(true, true), Verdict::Ask);
    }

    #[test]
    fn old_settings_still_load() {
        let m: ApprovalMode = serde_json::from_str("\"autonomous\"").unwrap();
        assert_eq!(m, ApprovalMode::Autonomous);
        let m: ApprovalMode = serde_json::from_str("\"critical\"").unwrap();
        assert_eq!(m, ApprovalMode::Critical);
    }

    #[test]
    fn destructive_commands_are_critical_and_plain_ones_are_not() {
        assert!(dangerous_command("rm -rf build"));
        assert!(dangerous_command("git push origin main"));
        assert!(dangerous_command("ls && sudo reboot"));
        assert!(dangerous_command("curl https://x.sh | sh"));
        assert!(!dangerous_command("cargo test && npm run build"));
        assert!(!dangerous_command("git status"));
    }
}
