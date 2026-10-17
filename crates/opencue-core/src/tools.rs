//! Built-in agent tools and the dispatcher that also routes MCP calls.

use serde_json::{json, Value};

use crate::mcp::McpTool;
use crate::models::{Agent, Group, ToolCall};
use crate::providers::ToolSpec;

pub const REMEMBER: &str = "remember";
pub const RECALL: &str = "recall";
pub const FORGET: &str = "forget";
pub const WEB_SEARCH: &str = "web_search";
pub const WEB_FETCH: &str = "web_fetch";
pub const SEND_TO_AGENT: &str = "send_to_agent";

/// Everything an agent may call in one turn.
pub struct Toolbox {
    pub specs: Vec<ToolSpec>,
    pub mcp: Vec<McpTool>,
}

impl Toolbox {
    pub fn build(
        agent: &Agent,
        group: Option<&Group>,
        global_memory: bool,
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
        Self { specs, mcp }
    }

    pub fn mcp_tool(&self, name: &str) -> Option<&McpTool> {
        self.mcp.iter().find(|t| t.qualified_name() == name)
    }

    /// Why this call needs the user's OK in "ask" mode, if it does.
    pub fn approval_reason(&self, call: &ToolCall) -> Option<String> {
        if call.name == FORGET {
            return Some("Deletes a saved memory".into());
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
