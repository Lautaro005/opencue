//! Built-in agent tools and the dispatcher that also routes MCP calls.

use serde_json::{json, Value};

use crate::computer::ComputerBackend;
use crate::mcp::McpTool;
use crate::models::{Agent, Group, ToolCall};
use crate::providers::ToolSpec;

pub const REMEMBER: &str = "remember";
pub const RECALL: &str = "recall";
pub const FORGET: &str = "forget";
pub const WEB_SEARCH: &str = "web_search";
pub const WEB_FETCH: &str = "web_fetch";
pub const SEND_TO_AGENT: &str = "send_to_agent";
pub const USE_SKILL: &str = "use_skill";
pub const SAVE_SKILL: &str = "save_skill";
pub const RUN_COMMAND: &str = "run_command";
pub const READ_FILE: &str = "read_file";
pub const WRITE_FILE: &str = "write_file";
pub const LIST_FILES: &str = "list_files";
pub const READ_EMAILS: &str = "read_emails";
pub const SEARCH_INBOX: &str = "search_inbox";
pub const SEND_EMAIL: &str = "send_email";
pub const COMPOSIO_SEARCH: &str = "composio_search";
pub const COMPOSIO_EXECUTE: &str = "composio_execute";
pub const SHARE_FILE: &str = "share_file";
pub const CREATE_AUTOMATION: &str = "create_automation";

pub fn is_phase2(name: &str) -> bool {
    [
        USE_SKILL,
        SAVE_SKILL,
        RUN_COMMAND,
        READ_FILE,
        WRITE_FILE,
        LIST_FILES,
        READ_EMAILS,
        SEARCH_INBOX,
        SEND_EMAIL,
        COMPOSIO_SEARCH,
        COMPOSIO_EXECUTE,
        CREATE_AUTOMATION,
    ]
    .contains(&name)
}

/// Which optional capabilities this agent has right now.
#[derive(Default)]
pub struct Capabilities {
    pub skills: bool,
    pub computer: ComputerBackend,
    pub email: bool,
    pub composio: bool,
}

/// Everything an agent may call in one turn.
pub struct Toolbox {
    pub specs: Vec<ToolSpec>,
    pub mcp: Vec<McpTool>,
    pub computer: ComputerBackend,
}

fn spec(name: &str, description: &str, parameters: Value) -> ToolSpec {
    ToolSpec {
        name: name.into(),
        description: description.into(),
        parameters,
    }
}

impl Toolbox {
    pub fn build(
        agent: &Agent,
        group: Option<&Group>,
        global_memory: bool,
        caps: &Capabilities,
        mcp: Vec<McpTool>,
    ) -> Self {
        let mut specs = vec![
            ToolSpec {
                name: REMEMBER.into(),
                description: "Save a durable fact or preference about the user or your work so you remember it in future conversations. Use scope \"global\" only for facts every agent should know.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "One self-contained fact, e.g. 'Prefers aisle seats on flights'." },
                        "scope": { "type": "string", "enum": if global_memory { json!(["agent", "global"]) } else { json!(["agent"]) } }
                    },
                    "required": ["text"]
                }),
            },
            ToolSpec {
                name: RECALL.into(),
                description: "Search your saved memories. Returns matching entries with their ids.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": { "query": { "type": "string", "description": "Keywords; empty returns everything." } }
                }),
            },
            ToolSpec {
                name: FORGET.into(),
                description: "Delete a saved memory by id (see recall). The user may be asked to approve.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "scope": { "type": "string", "enum": ["agent", "global"] }
                    },
                    "required": ["id"]
                }),
            },
        ];
        if agent.settings.web_access {
            specs.push(ToolSpec {
                name: WEB_SEARCH.into(),
                description:
                    "Search the web (DuckDuckGo/Wikipedia). Returns titles, URLs and snippets."
                        .into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" },
                        "max_results": { "type": "integer", "minimum": 1, "maximum": 20 }
                    },
                    "required": ["query"]
                }),
            });
            specs.push(ToolSpec {
                name: WEB_FETCH.into(),
                description: "Download a web page and return its main content as Markdown. Use offset to read further in long pages.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "url": { "type": "string" },
                        "offset": { "type": "integer", "minimum": 0 }
                    },
                    "required": ["url"]
                }),
            });
        }
        if group.is_some() {
            specs.push(ToolSpec {
                name: SEND_TO_AGENT.into(),
                description: "Hand work to another agent in this group chat. They will reply in the group after you finish.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "agent": { "type": "string", "description": "The other agent's name." },
                        "message": { "type": "string", "description": "What you need from them." }
                    },
                    "required": ["agent", "message"]
                }),
            });
        }
        specs.push(spec(
            CREATE_AUTOMATION,
            "Create an automation for yourself: a prompt that runs by itself on a schedule or when an email arrives. Use it when the user asks for something recurring or event-driven (or runs /create-automation).",
            json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Short title, e.g. 'Morning briefing'." },
                    "prompt": { "type": "string", "description": "What you should do each time it runs, written as an instruction to yourself." },
                    "trigger": { "type": "string", "enum": ["schedule", "email"] },
                    "cron": { "type": "string", "description": "For schedule: 5-field cron in the user's local time, e.g. '0 9 * * 1-5' for weekdays at 9:00." },
                    "filter": { "type": "string", "description": "For email: only emails matching this text (e.g. 'from:boss@example.com'). Empty means every new email." },
                    "poll_minutes": { "type": "integer", "minimum": 1, "maximum": 1440, "description": "For email: how often to check. Default 5." }
                },
                "required": ["name", "prompt", "trigger"]
            }),
        ));
        if caps.skills {
            specs.push(spec(USE_SKILL, "Load a skill's full instructions by name (see the Skills list in your prompt) before doing a task it covers.", json!({
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"]
            })));
            specs.push(spec(SAVE_SKILL, "Create or update a reusable skill (Markdown instructions) so you or other agents can follow it next time. Use when the user teaches you a procedure worth keeping.", json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Short kebab-case name, e.g. 'weekly-report'." },
                    "description": { "type": "string", "description": "When to use this skill, in one or two sentences." },
                    "body": { "type": "string", "description": "Step-by-step Markdown instructions." }
                },
                "required": ["name", "description", "body"]
            })));
        }
        if caps.computer != ComputerBackend::None {
            let place = match caps.computer {
                ComputerBackend::Host => "on the user's computer, in your workspace folder",
                ComputerBackend::Docker => "in your Linux container (workspace at /workspace)",
                ComputerBackend::Wsl => "in Windows Subsystem for Linux",
                _ => "in your Linux sandbox VM",
            };
            specs.push(spec(
                RUN_COMMAND,
                &format!("Run a shell command {place}. Returns output and exit code."),
                json!({
                    "type": "object",
                    "properties": {
                        "command": { "type": "string" },
                        "timeout_seconds": { "type": "integer", "minimum": 1, "maximum": 1800 }
                    },
                    "required": ["command"]
                }),
            ));
            specs.push(spec(READ_FILE, "Read a text file from your workspace.", json!({
                "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"]
            })));
            specs.push(spec(
                WRITE_FILE,
                "Create or overwrite a text file in your workspace.",
                json!({
                    "type": "object",
                    "properties": { "path": { "type": "string" }, "content": { "type": "string" } },
                    "required": ["path", "content"]
                }),
            ));
            specs.push(spec(
                LIST_FILES,
                "List a folder in your workspace (empty path = workspace root).",
                json!({
                    "type": "object", "properties": { "path": { "type": "string" } }
                }),
            ));
            specs.push(spec(
                SHARE_FILE,
                "Send a file from your workspace to the chat so the user can open or save it (reports, images, spreadsheets…). Files the user shares appear in your workspace under files/.",
                json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "caption": { "type": "string", "description": "Optional short message shown with the file." }
                    },
                    "required": ["path"]
                }),
            ));
        }
        if caps.email {
            specs.push(spec(
                READ_EMAILS,
                "List the newest emails in your inbox.",
                json!({
                    "type": "object",
                    "properties": {
                        "limit": { "type": "integer", "minimum": 1, "maximum": 30 },
                        "unread_only": { "type": "boolean" }
                    }
                }),
            ));
            specs.push(spec(SEARCH_INBOX, "Search your inbox. Plain words search the full text; prefix with from: or subject: to narrow.", json!({
                "type": "object",
                "properties": { "query": { "type": "string" }, "limit": { "type": "integer", "minimum": 1, "maximum": 30 } },
                "required": ["query"]
            })));
            specs.push(spec(SEND_EMAIL, "Send an email from your address. The user may be asked to approve it first.", json!({
                "type": "object",
                "properties": {
                    "to": { "type": "string", "description": "Comma-separated addresses." },
                    "subject": { "type": "string" },
                    "body": { "type": "string", "description": "Plain text." },
                    "cc": { "type": "string" },
                    "in_reply_to": { "type": "string", "description": "Message-ID of the email you are answering, if any." }
                },
                "required": ["to", "subject", "body"]
            })));
        }
        if caps.composio {
            specs.push(spec(COMPOSIO_SEARCH, "Find Composio actions for connected apps (Gmail, Slack, Notion, GitHub, Calendar…). Returns action slugs and their parameters.", json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "What you want to do, e.g. 'create calendar event'." },
                    "toolkit": { "type": "string", "description": "Optional app slug, e.g. 'googlecalendar'." }
                },
                "required": ["query"]
            })));
            specs.push(spec(COMPOSIO_EXECUTE, "Run a Composio action by slug with its arguments (see composio_search). Actions that change data may need the user's approval.", json!({
                "type": "object",
                "properties": { "slug": { "type": "string" }, "arguments": { "type": "object" } },
                "required": ["slug", "arguments"]
            })));
        }
        for t in &mcp {
            specs.push(ToolSpec {
                name: t.qualified_name(),
                description: t
                    .tool
                    .description
                    .as_deref()
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        format!("{} tool from MCP server {}", t.tool.name, t.server_name)
                    }),
                parameters: Value::Object((*t.tool.input_schema).clone()),
            });
        }
        Self {
            specs,
            mcp,
            computer: caps.computer.clone(),
        }
    }

    pub fn empty() -> Self {
        Self {
            specs: vec![],
            mcp: vec![],
            computer: ComputerBackend::None,
        }
    }

    pub fn mcp_tool(&self, name: &str) -> Option<&McpTool> {
        self.mcp.iter().find(|t| t.qualified_name() == name)
    }

    /// Why this call needs the user's OK in "ask" mode, if it does.
    pub fn approval_reason(&self, call: &ToolCall) -> Option<String> {
        match call.name.as_str() {
            FORGET => return Some("Deletes a saved memory".into()),
            CREATE_AUTOMATION => {
                return Some(format!(
                    "Creates the automation “{}” that will run on its own",
                    arg_str(&call.arguments, "name").unwrap_or("untitled")
                ))
            }
            SEND_EMAIL => {
                return Some(format!(
                    "Sends an email to {}",
                    arg_str(&call.arguments, "to").unwrap_or("someone")
                ))
            }
            RUN_COMMAND if self.computer.touches_host() => {
                return Some("Runs a command on your computer".into())
            }
            WRITE_FILE if self.computer.touches_host() => {
                return Some("Writes a file on your computer".into())
            }
            COMPOSIO_EXECUTE => {
                let slug = arg_str(&call.arguments, "slug").unwrap_or_default();
                return (!crate::composio::looks_read_only(slug))
                    .then(|| format!("Runs {slug} on a connected app"));
            }
            _ => {}
        }
        let t = self.mcp_tool(&call.name)?;
        t.is_destructive().then(|| {
            format!(
                "{} on {} may change or delete data",
                t.tool.name, t.server_name
            )
        })
    }
}

pub fn arg_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

pub fn arg_u64(args: &Value, key: &str) -> Option<u64> {
    args.get(key).and_then(|v| {
        v.as_u64()
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
    })
}
