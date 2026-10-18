//! System notifications: a finished reply or an approval request while the window is out of sight.
//! Everything is a setting (Settings → Notifications); the defaults are on, only when unfocused.

use opencue_core::events::CoreEvent;
use opencue_core::models::Role;
use opencue_core::Core;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

async fn flag(core: &Core, key: &str, default: bool) -> bool {
    core.store
        .setting(key)
        .await
        .ok()
        .flatten()
        .map(|v| v != "false")
        .unwrap_or(default)
}

fn window_in_view(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .map(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false))
        .unwrap_or(false)
}

fn snippet(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > 140 {
        format!("{}…", flat.chars().take(140).collect::<String>())
    } else {
        flat
    }
}

pub fn show(app: &AppHandle, title: &str, body: &str) -> Result<(), String> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|e| e.to_string())
}

/// Turn a core event into a notification, if the user asked for that kind and isn't looking.
pub async fn on_event(app: &AppHandle, core: &Core, ev: &CoreEvent) {
    let (kind, agent_id, body) = match ev {
        CoreEvent::Message { message } => {
            let final_reply = message.role == Role::Assistant
                && message.tool_calls.is_none()
                && !message.content.trim().is_empty();
            if !final_reply || !matches!(message.status.as_str(), "done" | "error") {
                return;
            }
            let Some(id) = message.agent_id.clone() else {
                return;
            };
            let failed = message.status == "error";
            ("notify_replies", id, (failed, snippet(&message.content)))
        }
        CoreEvent::ApprovalRequested { approval } => (
            "notify_approvals",
            approval.agent_id.clone(),
            (false, approval.reason.clone()),
        ),
        _ => return,
    };
    if !flag(core, "notify_enabled", true).await || !flag(core, kind, true).await {
        return;
    }
    if flag(core, "notify_background_only", true).await && window_in_view(app) {
        return;
    }
    let name = core
        .store
        .get_agent(&agent_id)
        .await
        .map(|a| a.name)
        .unwrap_or_else(|_| "OpenCue".into());
    let show_text = flag(core, "notify_preview", true).await;
    let (title, text) = match (kind, body) {
        ("notify_approvals", (_, reason)) => (format!("{name} needs your approval"), reason),
        (_, (true, text)) => (format!("{name} ran into a problem"), text),
        (_, (false, text)) => (name, text),
    };
    let text = if show_text || kind == "notify_approvals" {
        text
    } else {
        "New message".into()
    };
    if let Err(e) = show(app, &title, &text) {
        tracing::debug!("notification failed: {e}");
    }
}
