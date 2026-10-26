//! System notifications: a finished reply or an approval request while the window is out of sight.
//! Everything is a setting (Settings → Notifications); the defaults are on, only when unfocused.

use opencue_core::events::CoreEvent;
use opencue_core::models::Role;
use opencue_core::Core;
use std::sync::atomic::{AtomicI32, Ordering};
use tauri::{AppHandle, Manager};

use tauri_plugin_notification::{NotificationExt, PermissionState};

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

static NEXT_ID: AtomicI32 = AtomicI32::new(1);

pub fn show(app: &AppHandle, title: &str, body: &str) -> Result<(), String> {
    let n = app.notification();
    // Ask again when it isn't granted: the system can drop the permission between runs.
    if !matches!(n.permission_state(), Ok(PermissionState::Granted)) {
        match n.request_permission() {
            Ok(PermissionState::Granted) => {}
            _ => {
                return Err(
                    "Notifications are blocked for OpenCue. Allow them in your system settings."
                        .into(),
                )
            }
        }
    }
    // Every notification gets its own id: the system replaces (and silently drops) a new one that
    // reuses the id of one still sitting in its notification center.
    n.builder()
        .id(NEXT_ID.fetch_add(1, Ordering::Relaxed))
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
        CoreEvent::QuestionRequested { question } => (
            "notify_approvals",
            question.agent_id.clone(),
            (false, question.body.clone()),
        ),
        _ => return,
    };
    if !flag(core, "notify_enabled", true).await || !flag(core, kind, true).await {
        return;
    }
    // With the notch chosen, it shows these itself (see notch.rs and the NotchApp page).
    if core
        .store
        .setting("notify_channel")
        .await
        .ok()
        .flatten()
        .as_deref()
        == Some("notch")
    {
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
