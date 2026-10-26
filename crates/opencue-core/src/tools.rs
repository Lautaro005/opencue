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
pub const SHARE_FILE: &str = "share_file";
pub const CREATE_AUTOMATION: &str = "create_automation";
pub const UPDATE_INSTRUCTIONS: &str = "update_instructions";
pub const UPDATE_SOUL: &str = "update_soul";
pub const UPDATE_PLAN: &str = "update_plan";
pub const RESEARCH: &str = "research";
pub const ASK_USER: &str = "ask_user";
pub const CREATE_ARTIFACT: &str = "create_artifact";
pub const READ_ARTIFACT: &str = "read_artifact";
pub const READ_DASHBOARD: &str = "read_dashboard";
pub const UPDATE_DASHBOARD: &str = "update_dashboard";
pub const BROWSER_OPEN: &str = "browser_open";
pub const BROWSER_SNAPSHOT: &str = "browser_snapshot";
pub const BROWSER_CLICK: &str = "browser_click";
pub const BROWSER_TYPE: &str = "browser_type";
pub const BROWSER_CLOSE: &str = "browser_close";

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
        CREATE_AUTOMATION,
        UPDATE_INSTRUCTIONS,
        UPDATE_SOUL,
        UPDATE_PLAN,
    ]
    .contains(&name)
}

/// Which optional capabilities this agent has right now.
#[derive(Default)]
pub struct Capabilities {
    pub skills: bool,
    pub computer: ComputerBackend,
    pub email: bool,
    pub browser: bool,
    /// This agent was given the dashboard page to edit.
    pub dashboard: bool,
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
        specs.push(spec(
            UPDATE_INSTRUCTIONS,
            "Rewrite your own instructions: your job, what you do, rules, workflows and how to treat the user. Call it when the user tells you what you are for or how to work, and send the COMPLETE updated text (merge with the current one, keep it short, Markdown).",
            json!({
                "type": "object",
                "properties": { "text": { "type": "string", "description": "The full new instructions." } },
                "required": ["text"]
            }),
        ));
        specs.push(spec(
            UPDATE_SOUL,
            "Rewrite your own soul (SOUL.md): who you are, your personality, tone, values and style. Call it when the user shapes how you should sound or behave, and send the COMPLETE updated text (merge with the current one, keep it short, Markdown).",
            json!({
                "type": "object",
                "properties": { "text": { "type": "string", "description": "The full new soul." } },
                "required": ["text"]
            }),
        ));
        specs.push(spec(
            UPDATE_PLAN,
            "Keep a short plan for work that takes several steps. Send the COMPLETE list each time, with every step's status; it is shown back to you at the end of every step so you stay on track. Send an empty list when the work is done.",
            json!({
                "type": "object",
                "properties": {
                    "steps": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "step": { "type": "string" },
                                "status": { "type": "string", "enum": ["pending", "doing", "done"] }
                            },
                            "required": ["step", "status"]
                        }
                    }
                },
                "required": ["steps"]
            }),
        ));
        specs.push(spec(
            RESEARCH,
            "Hand a reading or research job to a helper that starts with a clean context and can only read (web search and pages, your memory, your workspace files). It returns a short report, so your own context stays small. Use it for anything that needs many searches or long pages.",
            json!({
                "type": "object",
                "properties": { "task": { "type": "string", "description": "What to find out, with every detail the helper needs: it can't see this chat." } },
                "required": ["task"]
            }),
        ));
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
        specs.push(spec(
            ASK_USER,
            "Ask the user something and wait for the answer: a question you need answered before going on, a choice between options, or permission for something sensitive. The user sees a highlighted card in the chat and answers there. Use it instead of guessing; don't use it for things you can find out yourself.",
            json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string", "description": "What you need to know, or what you want permission to do and why." },
                    "title": { "type": "string", "description": "Optional short heading for the card." },
                    "kind": { "type": "string", "enum": ["question", "permission"], "description": "\"permission\" shows Accept / Reject buttons; \"question\" shows a text field (or your options)." },
                    "options": { "type": "array", "items": { "type": "string" }, "description": "Optional answers to offer as buttons." }
                },
                "required": ["question"]
            }),
        ));
        if group.is_none() {
            specs.push(spec(
                SEND_TO_AGENT,
                "Talk to another agent and get their reply as the result, without a group chat. Ask them for what you need with every detail: they can't see your conversation with the user. The user can open the exchange from the tool card.",
                json!({
                    "type": "object",
                    "properties": {
                        "agent": { "type": "string", "description": "The other agent's name." },
                        "message": { "type": "string", "description": "What you need from them." }
                    },
                    "required": ["agent", "message"]
                }),
            ));
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
        specs.push(spec(
            CREATE_ARTIFACT,
            "Show the user something you made as an artifact: a side panel opens next to the chat and the file is kept in your Files. Use it for substantial, standalone content they will keep, edit or reuse (a page, a diagram, a document, a spreadsheet, a deck), not for short answers. \
             Types and what `content` must be: html (a complete page with inline CSS and JS; no network except cdnjs, jsDelivr and unpkg scripts), svg (an <svg> image), markdown (a formatted text), mermaid (diagram source: flowchart, sequence, gantt…), code (source code; set `language`), text, json, csv, \
             xlsx (JSON {\"sheets\":[{\"name\":\"Sales\",\"rows\":[[\"Item\",\"Qty\"],[\"Pen\",3],[\"Total\",\"=SUM(B2:B2)\"]]}]}; strings starting with = are formulas; the first row is the header), \
             docx and pdf (Markdown: # headings, **bold**, lists, | tables |, ``` code), pptx (Markdown, slides separated by a line of ---: # title, - bullets, Notes: speaker notes). \
             To change an artifact you made, call it again with its `artifact_id` and the COMPLETE new content: it becomes a new version.",
            json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Short name shown on the card and used for the file name." },
                    "type": { "type": "string", "enum": crate::artifacts::KINDS, "description": "Required for a new artifact." },
                    "content": { "type": "string", "description": "The complete content, in the format of the type." },
                    "language": { "type": "string", "description": "For type code: python, javascript, rust…" },
                    "artifact_id": { "type": "string", "description": "Update this artifact (new version) instead of creating one." },
                    "canvas": { "type": "boolean", "description": "Make it a canvas the user can edit in the side panel (use it when they ran /canvas or want to write or build something together with you)." }
                },
                "required": ["content"]
            }),
        ));
        specs.push(spec(
            READ_ARTIFACT,
            "Read the latest text of an artifact or canvas you made, including anything the user edited by hand. Call it before changing a canvas.",
            json!({
                "type": "object",
                "properties": { "artifact_id": { "type": "string" } },
                "required": ["artifact_id"]
            }),
        ));
        if caps.dashboard {
            specs.push(spec(
                READ_DASHBOARD,
                "Read the user's dashboard (the home screen page you were given) and how it receives live data. Call it before changing it.",
                json!({ "type": "object", "properties": {} }),
            ));
            specs.push(spec(
                UPDATE_DASHBOARD,
                "Replace the user's dashboard with a new complete HTML page. Read it first with read_dashboard and keep what the user liked. Pass the full page.",
                json!({
                    "type": "object",
                    "properties": { "html": { "type": "string", "description": "The complete HTML document." } },
                    "required": ["html"]
                }),
            ));
        }
        if caps.browser {
            specs.push(spec(
                BROWSER_OPEN,
                "Open a web page in your browser pane (the user watches it). Use it for sites that need real browsing: logins, forms, dashboards, pages built with JavaScript. For plain reading prefer web_fetch. The user may be asked to approve a new site.",
                json!({
                    "type": "object",
                    "properties": {
                        "url": { "type": "string" },
                        "new_tab": { "type": "boolean", "description": "Open it in a new tab instead of the current one" }
                    },
                    "required": ["url"]
                }),
            ));
            specs.push(spec(
                BROWSER_SNAPSHOT,
                "Read the page in your browser: address, title, the visible text and a numbered list of the things you can click or type into. Call it after every open, click or type.",
                json!({ "type": "object", "properties": {} }),
            ));
            specs.push(spec(
                BROWSER_CLICK,
                "Click an element from the last snapshot by its number.",
                json!({
                    "type": "object",
                    "properties": { "index": { "type": "integer", "minimum": 0 } },
                    "required": ["index"]
                }),
            ));
            specs.push(spec(
                BROWSER_TYPE,
                "Type text into an input from the last snapshot by its number; set submit to press Enter afterwards. Never type passwords or payment details: ask the user to do that in the pane.",
                json!({
                    "type": "object",
                    "properties": {
                        "index": { "type": "integer", "minimum": 0 },
                        "text": { "type": "string" },
                        "submit": { "type": "boolean" }
                    },
                    "required": ["index", "text"]
                }),
            ));
            specs.push(spec(
                BROWSER_CLOSE,
                "Close the page in your browser pane when you are done.",
                json!({ "type": "object", "properties": {} }),
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
