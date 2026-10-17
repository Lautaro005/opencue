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
    /// Something in the sidebar changed (agent/group created, renamed, deleted).
    ThreadsChanged,
    McpChanged,
    AutomationsChanged,
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
