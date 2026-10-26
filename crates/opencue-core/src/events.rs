//! Events pushed from the core to the GUI / CLI.

use serde::Serialize;

use crate::models::{Message, ToolCall};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CoreEvent {
    /// A message row was created or updated.
    Message {
        message: Message,
    },
    /// Streaming text for an in-progress assistant message.
    Delta {
        thread_id: String,
        message_id: String,
        agent_id: String,
        text: String,
    },
    /// A tool call (an artifact, say) is still being written by the model: how far it has got.
    ToolProgress {
        thread_id: String,
        message_id: String,
        agent_id: String,
        tool: String,
        /// The title, once the model has written it.
        title: Option<String>,
        chars: usize,
    },
    /// A local model is downloading, unpacking, ready, cancelled or failed.
    ModelProgress {
        id: String,
        phase: String,
        done: u64,
        total: u64,
        error: Option<String>,
    },
    /// A terminal session printed something (base64 bytes).
    TermOutput {
        id: String,
        data: String,
    },
    /// A terminal session's program ended.
    TermExit {
        id: String,
        code: i32,
    },
    /// Messages were taken out of a thread (a queued message the user took back).
    MessagesRemoved {
        thread_id: String,
        ids: Vec<String>,
    },
    /// An agent started or stopped working in a thread.
    Busy {
        thread_id: String,
        agent_id: String,
        busy: bool,
    },
    ApprovalRequested {
        approval: ApprovalRequest,
    },
    ApprovalResolved {
        approval_id: String,
        approved: bool,
    },
    /// WhatsApp changed state (QR, connected, who answers…).
    MessengerChanged,
    /// An agent is using the built-in browser: the app shows it.
    BrowserActivity {
        agent_id: String,
    },
    /// A catalog app finished signing in (or failed to).
    CatalogChanged {
        slug: String,
        connected: bool,
        error: Option<String>,
    },
    /// An agent asks the user a question or for permission.
    QuestionRequested {
        question: crate::bridge::Question,
    },
    QuestionResolved {
        question_id: String,
        answer: String,
    },
    /// Something in the sidebar changed (agent/group created, renamed, deleted).
    ThreadsChanged,
    McpChanged,
    /// An agent rewrote its own instructions or soul.
    ProfileChanged {
        agent_id: String,
    },
    AutomationsChanged,
    /// The dashboard page was rewritten (or reset).
    DashboardChanged,
    /// Output from a command running on an agent's computer.
    ComputerOutput {
        agent_id: String,
        text: String,
        stderr: bool,
    },
    Error {
        thread_id: Option<String>,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct ApprovalRequest {
    pub id: String,
    pub thread_id: String,
    pub agent_id: String,
    pub message_id: String,
    pub tool_call: ToolCall,
    pub reason: String,
}
