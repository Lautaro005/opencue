//! The dashboard: the home view when no chat is open. It is an HTML page (the built-in one, or
//! one the user's chosen agent wrote with `update_dashboard`) that the app feeds with a live
//! snapshot of what is going on: running agents, automations, plans, recent chats and usage.

use serde_json::{json, Value};

use crate::models::*;
use crate::Core;

/// The page shown until an agent writes another one.
pub const DEFAULT_HTML: &str = include_str!("dashboard_default.html");
const MAX_HTML_BYTES: usize = 512 * 1024;
/// The setting holding the id of the agent that may edit the dashboard.
pub const AGENT_SETTING: &str = "dashboard_agent";

/// What the agent is told about the page it edits.
pub const CONTRACT: &str = "The dashboard is one self-contained HTML page (inline CSS and JS, no network). OpenCue sends it live data as a window message, now and every few seconds: \
window.addEventListener('message', e => { if (e.data?.type === 'opencue:data') render(e.data.data) }); and the page can ask for it at once with parent.postMessage({type:'opencue:ready'}, '*'). \
parent.postMessage({type:'opencue:open', agent_id}, '*') opens an agent's chat. The data: now, theme ('light'|'dark'), user_name; \
agents {total, running, list[{id,name,model,busy,avatar{color,shape,face}}]}; running[{thread_id,name,kind}]; \
automations {total, enabled, upcoming[], list[{id,name,agent_id,agent_name,enabled,trigger,cron,next_run_at,last_run_at,last_status}]}; \
tasks[{agent_id,agent_name,done,total,items[{text,status}]}]; usage {messages,input_tokens,output_tokens,by_agent[{agent_id,name,messages,tokens}]}; \
recent[{thread_id,name,kind,last_message,last_at}]. Set text with textContent, never innerHTML, since names and messages are data.";

fn plan_items(plan: &str) -> Vec<(String, &'static str)> {
    plan.lines()
        .filter_map(|l| {
            let l = l.trim();
            let (mark, text) = l.strip_prefix("- [")?.split_once("] ")?;
            let status = match mark {
                "x" => "done",
                "~" => "doing",
                _ => "todo",
            };
            Some((text.trim().to_string(), status))
        })
        .collect()
}

fn short(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        flat
    } else {
        format!("{}…", flat.chars().take(max).collect::<String>())
    }
}

impl Core {
    fn dashboard_file(&self) -> std::path::PathBuf {
        self.paths().root().join("dashboard").join("index.html")
    }

    /// The dashboard page: the user's own, or the built-in one.
    pub fn dashboard_html(&self) -> String {
        std::fs::read_to_string(self.dashboard_file()).unwrap_or_else(|_| DEFAULT_HTML.to_string())
    }

    /// Whether the page is the built-in one.
    pub fn dashboard_is_default(&self) -> bool {
        !self.dashboard_file().exists()
    }

    pub fn set_dashboard_html(&self, html: &str) -> anyhow::Result<()> {
        anyhow::ensure!(!html.trim().is_empty(), "the dashboard can't be empty");
        anyhow::ensure!(
            html.len() <= MAX_HTML_BYTES,
            "the dashboard is too large (limit 512 KB)"
        );
        let file = self.dashboard_file();
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(file, html)?;
        self.emit(crate::CoreEvent::DashboardChanged);
        Ok(())
    }

    /// Back to the built-in page.
    pub fn reset_dashboard(&self) -> anyhow::Result<()> {
        let _ = std::fs::remove_file(self.dashboard_file());
        self.emit(crate::CoreEvent::DashboardChanged);
        Ok(())
    }

    /// The agent allowed to edit the dashboard, if one was chosen and still exists.
    pub async fn dashboard_agent(&self) -> Option<String> {
        let id = self.store.setting(AGENT_SETTING).await.ok().flatten()?;
        if id.is_empty() {
            return None;
        }
        self.store.get_agent(&id).await.ok().map(|a| a.id)
    }

    /// The snapshot the dashboard page draws.
    pub async fn dashboard_data(&self) -> anyhow::Result<Value> {
        let agents = self.store.list_agents().await?;
        let threads = self.threads().await?;
        let busy = |id: &str| threads.iter().any(|t| t.id == id && t.busy);
        let name_of = |id: &str| {
            agents
                .iter()
                .find(|a| a.id == id)
                .map(|a| a.name.clone())
                .unwrap_or_default()
        };

        let running: Vec<Value> = threads
            .iter()
            .filter(|t| t.busy)
            .map(|t| {
                json!({
                    "thread_id": t.id,
                    "name": t.name,
                    "kind": match t.kind { ThreadKind::Group => "group", _ => "agent" },
                })
            })
            .collect();

        let automations = self.automations(None).await?;
        let mut upcoming: Vec<&crate::automations::Automation> = automations
            .iter()
            .filter(|a| a.enabled && a.next_run_at.is_some())
            .collect();
        upcoming.sort_by(|a, b| a.next_run_at.cmp(&b.next_run_at));
        let auto_json = |a: &crate::automations::Automation| {
            let (trigger, cron) = match &a.trigger {
                crate::automations::Trigger::Schedule { cron } => ("schedule", cron.clone()),
                t => (t.kind(), String::new()),
            };
            json!({
                "id": a.id,
                "name": a.name,
                "agent_id": a.agent_id,
                "agent_name": name_of(&a.agent_id),
                "enabled": a.enabled,
                "trigger": trigger,
                "cron": cron,
                "next_run_at": a.next_run_at,
                "last_run_at": a.last_run_at,
                "last_status": a.last_status,
            })
        };
        let mut list: Vec<&crate::automations::Automation> = automations.iter().collect();
        list.sort_by(|a, b| {
            b.enabled
                .cmp(&a.enabled)
                .then_with(|| a.next_run_at.cmp(&b.next_run_at))
        });

        let mut tasks = Vec::new();
        for a in &agents {
            let items = plan_items(&self.store.plan(&a.id));
            let done = items.iter().filter(|(_, s)| *s == "done").count();
            if !items.is_empty() && done < items.len() {
                tasks.push(json!({
                    "agent_id": a.id,
                    "agent_name": a.name,
                    "done": done,
                    "total": items.len(),
                    "items": items.iter().map(|(t, s)| json!({ "text": t, "status": s })).collect::<Vec<_>>(),
                }));
            }
        }

        let usage = self.store.usage().await?;
        let mut by_agent: Vec<Value> = usage
            .iter()
            .filter_map(|u| {
                let id = u.agent_id.clone()?;
                Some(json!({
                    "agent_id": id,
                    "name": u.agent_name.clone().unwrap_or_default(),
                    "messages": u.messages,
                    "tokens": u.input_tokens + u.output_tokens,
                }))
            })
            .collect();
        by_agent.sort_by_key(|v| std::cmp::Reverse(v["tokens"].as_i64().unwrap_or(0)));

        let mut recent: Vec<&ThreadSummary> =
            threads.iter().filter(|t| t.last_at.is_some()).collect();
        recent.sort_by(|a, b| b.last_at.cmp(&a.last_at));

        Ok(json!({
            "now": chrono::Utc::now().to_rfc3339(),
            "user_name": self.store.setting("user_name").await.ok().flatten().unwrap_or_default(),
            "agents": {
                "total": agents.len(),
                "running": agents.iter().filter(|a| busy(&a.id)).count(),
                "list": agents.iter().map(|a| json!({
                    "id": a.id, "name": a.name, "model": a.model, "busy": busy(&a.id), "avatar": a.avatar,
                })).collect::<Vec<_>>(),
            },
            "running": running,
            "automations": {
                "total": automations.len(),
                "enabled": automations.iter().filter(|a| a.enabled).count(),
                "upcoming": upcoming.iter().take(5).map(|a| auto_json(a)).collect::<Vec<_>>(),
                "list": list.iter().map(|a| auto_json(a)).collect::<Vec<_>>(),
            },
            "tasks": tasks,
            "usage": {
                "messages": usage.iter().map(|u| u.messages).sum::<i64>(),
                "input_tokens": usage.iter().map(|u| u.input_tokens).sum::<i64>(),
                "output_tokens": usage.iter().map(|u| u.output_tokens).sum::<i64>(),
                "by_agent": by_agent,
            },
            "recent": recent.iter().take(8).map(|t| json!({
                "thread_id": t.id,
                "name": t.name,
                "kind": match t.kind { ThreadKind::Group => "group", _ => "agent" },
                "last_message": t.last_message.as_deref().map(|m| short(m, 90)),
                "last_at": t.last_at,
            })).collect::<Vec<_>>(),
        }))
    }

    /// `read_dashboard` / `update_dashboard`, for the agent that was given the dashboard.
    pub(crate) async fn dashboard_tool(
        &self,
        agent: &Agent,
        name: &str,
        args: &Value,
    ) -> anyhow::Result<String> {
        anyhow::ensure!(
            self.dashboard_agent().await.as_deref() == Some(agent.id.as_str()),
            "you are not the agent in charge of the dashboard (Settings → General)"
        );
        match name {
            crate::tools::READ_DASHBOARD => Ok(format!(
                "{CONTRACT}\n\nCurrent page ({}):\n\n{}",
                if self.dashboard_is_default() {
                    "the built-in one"
                } else {
                    "written by you or another agent"
                },
                self.dashboard_html()
            )),
            _ => {
                let html = crate::tools::arg_str(args, "html")
                    .ok_or_else(|| anyhow::anyhow!("`html` is required"))?;
                self.set_dashboard_html(html)?;
                Ok("Dashboard updated: it is shown on the user's home screen now.".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_are_read_back() {
        let items = plan_items("- [x] one\n- [~] two\n- [ ] three\nnoise");
        assert_eq!(
            items,
            vec![
                ("one".to_string(), "done"),
                ("two".to_string(), "doing"),
                ("three".to_string(), "todo")
            ]
        );
    }

    #[test]
    fn the_built_in_page_listens_for_data() {
        assert!(DEFAULT_HTML.contains("opencue:data"));
        assert!(DEFAULT_HTML.contains("opencue:ready"));
        assert_eq!(short("a  b\nc", 10), "a b c");
        assert_eq!(short("abcdef", 3), "abc…");
    }
}
