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
recent[{thread_id,name,kind,last_message,last_at}]; activity[{day,sent,replies,tokens}] (the last 28 days, oldest first, days without messages included); apps[{id,name,kind:'mcp'|'catalog'}] (the apps and tools connected to the agents); \
custom {key: {title, source, value, updated_at}}: data the agent saved with update_dashboard_data from connected apps, the web or files (value is any JSON). \
Set text with textContent, never innerHTML, since names and messages are data.";
/// How to build a good dashboard: the agent gets it with `read_dashboard`.
pub const DESIGN_KIT: &str = "## Dashboard design kit\n\
Purpose: a dashboard helps the user decide what to do next. Lead with what needs them (failed automations, tasks waiting, mail to answer, today's meetings), \
then what is moving, then history. Every block answers a question; drop decoration that answers none.\n\
Layout: a greeting line with the one-sentence state of things; a row of 3-5 stat cards (big number, short label, a delta or sparkline when there is history); \
then a responsive grid of cards (CSS grid, repeat(auto-fit, minmax(300px, 1fr))). It must work from 700 px wide up, with no horizontal scroll.\n\
Style profiles (pick the one the user asked for, or keep the current one): \
brand (OpenCue blue #4f6bed accent, soft cards, radius 14, hairline borders); calm (neutral greys, one accent, generous spacing, thin type); \
bold (saturated chart colors, larger numbers, colored card headers); glass (translucent cards with backdrop-filter blur over a soft gradient); \
editorial (serif headings, rules between sections, no card shadows); terminal (monospace, dark, green/amber accents). \
Always define colors as CSS variables for light and dark and switch on data.theme; text contrast at least 4.5:1.\n\
Components you can draw with plain HTML/CSS/SVG (no libraries needed): stat card; sparkline (an inline SVG polyline over activity); \
bar list (rank by tokens or messages with a proportional bar); activity heatmap (28 squares from activity, opacity by count); \
progress list (plan steps with done/total and a bar); timeline (upcoming automations by next_run_at); status pills (live/on/off/failed); \
table (sortable on click) for custom rows; donut (SVG circles with stroke-dasharray) for shares. Charts from data.custom go through the same components.\n\
States: every block needs an empty state that says what fills it (\"No automations yet: ask an agent with /create-automation\"), \
and custom data shows its source and how old it is (updated_at). Times are relative (\"5 min ago\") and in the user's locale.\n\
Interaction: rows that belong to an agent open its chat with parent.postMessage({type:'opencue:open', agent_id}, '*'). \
Keep animations subtle (fade-in, a pulse on live items) and respect prefers-reduced-motion.\n\
Checklist before update_dashboard: the page listens for opencue:data and posts opencue:ready; text set with textContent; \
no fetch, XMLHttpRequest, WebSocket or external images (there is no network; scripts only from cdnjs, jsDelivr or unpkg if you really need a library); \
works in light and dark; under 512 KB. OpenCue checks the page and keeps the previous version, so the user can undo.";

/// The most previous versions of the page kept for undo.
const KEEP_VERSIONS: usize = 12;

/// Problems that make a page unusable (`Err`) and ones worth fixing (the warnings returned).
pub fn check_page(html: &str) -> Result<Vec<String>, String> {
    let lower = html.to_lowercase();
    if !lower.contains("<html") && !lower.contains("<body") && !lower.contains("<div") {
        return Err("this is not an HTML page: pass the complete document".into());
    }
    if !html.contains("opencue:data") {
        return Err("the page never reads its data: add window.addEventListener('message', e => { if (e.data?.type === 'opencue:data') render(e.data.data) })".into());
    }
    let mut warnings = vec![];
    if !html.contains("opencue:ready") {
        warnings.push("it does not post {type:'opencue:ready'} on load, so the first data arrives a few seconds late".to_string());
    }
    for (needle, what) in [
        ("fetch(", "fetch"),
        ("xmlhttprequest", "XMLHttpRequest"),
        ("websocket", "WebSocket"),
        ("eventsource", "EventSource"),
    ] {
        if lower.contains(needle) {
            warnings.push(format!("it uses {what}, but the page has no network: bring that data with update_dashboard_data instead"));
        }
    }
    if lower.contains(".innerhtml") {
        warnings.push("it sets innerHTML: use textContent for names, messages and custom data (they are data, not markup)".into());
    }
    if !lower.contains("theme") {
        warnings.push(
            "it ignores data.theme, so it will not follow the app's light or dark mode".into(),
        );
    }
    Ok(warnings)
}

/// The most custom data the dashboard keeps.
const MAX_DATA_BYTES: usize = 256 * 1024;

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

    fn dashboard_data_file(&self) -> std::path::PathBuf {
        self.paths().root().join("dashboard").join("data.json")
    }

    /// What agents saved for the page with `update_dashboard_data`.
    fn dashboard_custom(&self) -> serde_json::Map<String, Value> {
        std::fs::read_to_string(self.dashboard_data_file())
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }

    /// Save (or with a null value, remove) one block of custom data.
    pub fn set_dashboard_custom(
        &self,
        key: &str,
        title: Option<&str>,
        source: Option<&str>,
        value: Value,
    ) -> anyhow::Result<()> {
        let key = key.trim();
        anyhow::ensure!(
            !key.is_empty()
                && key.len() <= 60
                && key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
            "`key` must be a short id made of letters, digits, _ or -"
        );
        let mut all = self.dashboard_custom();
        if value.is_null() {
            all.remove(key);
        } else {
            all.insert(
                key.to_string(),
                json!({
                    "title": title.unwrap_or(key),
                    "source": source.unwrap_or_default(),
                    "value": value,
                    "updated_at": chrono::Utc::now().to_rfc3339(),
                }),
            );
        }
        let text = Value::Object(all).to_string();
        anyhow::ensure!(
            text.len() <= MAX_DATA_BYTES,
            "too much data for the dashboard (limit 256 KB): save summaries, not raw dumps"
        );
        let file = self.dashboard_data_file();
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(file, text)?;
        Ok(())
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
        self.keep_dashboard_version()?;
        std::fs::write(file, html)?;
        self.emit(crate::CoreEvent::DashboardChanged);
        Ok(())
    }

    fn dashboard_history_dir(&self) -> std::path::PathBuf {
        self.paths().root().join("dashboard").join("history")
    }

    /// Put the current page (if it is the user's own) among the previous versions.
    fn keep_dashboard_version(&self) -> anyhow::Result<()> {
        let Ok(current) = std::fs::read_to_string(self.dashboard_file()) else {
            return Ok(());
        };
        let dir = self.dashboard_history_dir();
        std::fs::create_dir_all(&dir)?;
        let id = chrono::Utc::now().format("%Y%m%dT%H%M%S%3f").to_string();
        std::fs::write(dir.join(format!("{id}.html")), current)?;
        let mut all = self.dashboard_versions();
        while all.len() > KEEP_VERSIONS {
            if let Some(old) = all.pop() {
                let _ = std::fs::remove_file(dir.join(format!("{}.html", old.0)));
            }
        }
        Ok(())
    }

    /// Previous versions of the page, newest first: (id, saved at as RFC 3339).
    pub fn dashboard_versions(&self) -> Vec<(String, String)> {
        let mut ids: Vec<String> = std::fs::read_dir(self.dashboard_history_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                e.file_name()
                    .to_string_lossy()
                    .strip_suffix(".html")
                    .map(str::to_string)
            })
            .collect();
        ids.sort_unstable_by(|a, b| b.cmp(a));
        ids.into_iter()
            .map(|id| {
                let at = chrono::NaiveDateTime::parse_from_str(&id, "%Y%m%dT%H%M%S%3f")
                    .map(|t| t.and_utc().to_rfc3339())
                    .unwrap_or_default();
                (id, at)
            })
            .collect()
    }

    /// Go back to a previous version (the newest one when `id` is None). The page being replaced
    /// is kept too, so undo can be undone.
    pub fn restore_dashboard(&self, id: Option<&str>) -> anyhow::Result<()> {
        let id = match id {
            Some(i) => i.to_string(),
            None => self
                .dashboard_versions()
                .into_iter()
                .next()
                .map(|v| v.0)
                .ok_or_else(|| anyhow::anyhow!("there is no earlier version of the dashboard"))?,
        };
        anyhow::ensure!(
            id.chars().all(|c| c.is_ascii_alphanumeric()),
            "unknown version"
        );
        let path = self.dashboard_history_dir().join(format!("{id}.html"));
        let html =
            std::fs::read_to_string(&path).map_err(|_| anyhow::anyhow!("that version is gone"))?;
        let _ = std::fs::remove_file(&path);
        self.set_dashboard_html(&html)
    }

    /// Back to the built-in page.
    pub fn reset_dashboard(&self) -> anyhow::Result<()> {
        self.keep_dashboard_version()?;
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

        let mut apps: Vec<Value> = self
            .store
            .list_mcp_servers()
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|m| m.enabled)
            .map(|m| json!({ "id": m.id, "name": m.name, "kind": "mcp" }))
            .collect();
        apps.extend(
            self.catalog_status()
                .await
                .into_iter()
                .filter(|c| c.state == "connected")
                .map(|c| json!({ "id": c.slug, "name": c.name, "kind": "catalog" })),
        );

        // The last 28 days, each day present even when nothing happened.
        let today = chrono::Utc::now().date_naive();
        let start = today - chrono::Duration::days(27);
        let rows = self
            .store
            .activity(&start.format("%Y-%m-%d").to_string())
            .await
            .unwrap_or_default();
        let activity: Vec<Value> = (0..28)
            .map(|i| {
                let day = (start + chrono::Duration::days(i))
                    .format("%Y-%m-%d")
                    .to_string();
                let r = rows.iter().find(|r| r.0 == day);
                json!({
                    "day": day,
                    "sent": r.map_or(0, |r| r.1),
                    "replies": r.map_or(0, |r| r.2),
                    "tokens": r.map_or(0, |r| r.3),
                })
            })
            .collect();

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
            "activity": activity,
            "apps": apps,
            "custom": Value::Object(self.dashboard_custom()),
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
            crate::tools::READ_DASHBOARD => {
                let custom = self.dashboard_custom();
                let saved = if custom.is_empty() {
                    "none yet".to_string()
                } else {
                    custom
                        .iter()
                        .map(|(k, v)| {
                            format!(
                                "{k} ({}, from {}, saved {})",
                                v["title"].as_str().unwrap_or(k),
                                v["source"]
                                    .as_str()
                                    .filter(|s| !s.is_empty())
                                    .unwrap_or("?"),
                                v["updated_at"].as_str().unwrap_or("?")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                Ok(format!(
                    "{CONTRACT}\n\n{DESIGN_KIT}\n\nCustom data saved so far: {saved}.\n\nCurrent page ({}):\n\n{}",
                    if self.dashboard_is_default() {
                        "the built-in one"
                    } else {
                        "written by you or another agent"
                    },
                    self.dashboard_html()
                ))
            }
            crate::tools::UPDATE_DASHBOARD_DATA => {
                let key = crate::tools::arg_str(args, "key")
                    .ok_or_else(|| anyhow::anyhow!("`key` is required"))?;
                let value = args.get("value").cloned().unwrap_or(Value::Null);
                self.set_dashboard_custom(
                    key,
                    crate::tools::arg_str(args, "title"),
                    crate::tools::arg_str(args, "source"),
                    value.clone(),
                )?;
                Ok(if value.is_null() {
                    format!("Removed `{key}` from the dashboard data.")
                } else {
                    format!("Saved `{key}`: the page gets it as data.custom.{key} on its next refresh (a few seconds).")
                })
            }
            _ => {
                let html = crate::tools::arg_str(args, "html")
                    .ok_or_else(|| anyhow::anyhow!("`html` is required"))?;
                let warnings = check_page(html).map_err(|e| {
                    anyhow::anyhow!("Not saved: {e}. Fix it and call update_dashboard again.")
                })?;
                self.set_dashboard_html(html)?;
                Ok(if warnings.is_empty() {
                    "Dashboard updated: it is shown on the user's home screen now (the previous version is kept, so the user can undo).".into()
                } else {
                    format!(
                        "Dashboard updated and shown now, but fix these and save again:\n- {}",
                        warnings.join("\n- ")
                    )
                })
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

    #[tokio::test]
    async fn custom_data_is_saved_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        let core = crate::Core::open(crate::paths::Paths::at(dir.path()).unwrap())
            .await
            .unwrap();
        core.set_dashboard_custom("inbox", Some("Unread"), Some("Gmail"), json!(12))
            .unwrap();
        assert_eq!(core.dashboard_custom()["inbox"]["value"], 12);
        assert!(core
            .set_dashboard_custom("bad key!", None, None, json!(1))
            .is_err());
        core.set_dashboard_custom("inbox", None, None, Value::Null)
            .unwrap();
        assert!(core.dashboard_custom().is_empty());
    }

    #[test]
    fn pages_are_checked_before_saving() {
        assert!(check_page("hello").is_err());
        assert!(check_page("<div>no data</div>").is_err());
        assert!(
            check_page(DEFAULT_HTML).unwrap().is_empty(),
            "{:?}",
            check_page(DEFAULT_HTML)
        );
        let w = check_page("<body><script>addEventListener('message',e=>{if(e.data.type==='opencue:data'){x.innerHTML=1;fetch('/a')}})</script></body>").unwrap();
        assert_eq!(w.len(), 4, "{w:?}");
    }

    #[tokio::test]
    async fn earlier_versions_can_be_restored() {
        let dir = tempfile::tempdir().unwrap();
        let core = crate::Core::open(crate::paths::Paths::at(dir.path()).unwrap())
            .await
            .unwrap();
        assert!(core.restore_dashboard(None).is_err());
        core.set_dashboard_html("<div>one opencue:data</div>")
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        core.set_dashboard_html("<div>two opencue:data</div>")
            .unwrap();
        assert_eq!(core.dashboard_versions().len(), 1);
        core.restore_dashboard(None).unwrap();
        assert!(core.dashboard_html().contains("one"));
        // The page it replaced is kept, so undo can be undone.
        assert!(core.dashboard_versions().len() == 1);
        let data = core.dashboard_data().await.unwrap();
        assert_eq!(data["activity"].as_array().unwrap().len(), 28);
    }

    #[test]
    fn the_built_in_page_listens_for_data() {
        assert!(DEFAULT_HTML.contains("opencue:data"));
        assert!(DEFAULT_HTML.contains("opencue:ready"));
        assert_eq!(short("a  b\nc", 10), "a b c");
        assert_eq!(short("abcdef", 3), "abc…");
    }
}
