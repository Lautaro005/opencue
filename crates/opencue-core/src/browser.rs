//! The built-in browser: a webview pane inside the app's Computer panel that an agent can drive
//! (open a page, read it, click, type) while the user watches. The desktop app owns the webview
//! and implements [`BrowserHost`]; this module holds the rules around it: which pages may be
//! opened, the approval card for a new site, and the hop from a CLI agent's tool server (a
//! separate process) to the running app through the `bridge` table.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::db::{new_id, now};
use crate::events::CoreEvent;
use crate::models::{Agent, ApprovalMode};
use crate::tools::{
    arg_str, BROWSER_CLICK, BROWSER_CLOSE, BROWSER_OPEN, BROWSER_SNAPSHOT, BROWSER_TYPE,
};
use crate::Core;

const POLL: Duration = Duration::from_millis(400);

/// What the desktop app does for the browser tools. Every agent has its own page. `action` is `open`, `snapshot`, `click`,
/// `type` or `close`; the answer is the text the agent reads.
#[async_trait]
pub trait BrowserHost: Send + Sync {
    async fn run(&self, agent_id: &str, action: &str, args: Value) -> anyhow::Result<String>;
}

/// The page address an agent may open: http(s) only, and not this computer or its network.
pub fn checked_url(raw: &str) -> anyhow::Result<url::Url> {
    let raw = raw.trim();
    let with_scheme = if raw.contains("://") {
        raw.to_string()
    } else {
        format!("https://{raw}")
    };
    let url =
        url::Url::parse(&with_scheme).map_err(|e| anyhow::anyhow!("not a web address: {e}"))?;
    anyhow::ensure!(
        matches!(url.scheme(), "http" | "https"),
        "only http and https pages can be opened"
    );
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("the address has no host"))?
        .to_lowercase();
    anyhow::ensure!(
        !is_local_host(&host),
        "pages on this computer or its network can't be opened"
    );
    Ok(url)
}

fn is_local_host(host: &str) -> bool {
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return true;
    }
    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_loopback()
                    || v4.is_private()
                    || v4.is_link_local()
                    || v4.is_unspecified()
                    || v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1])
            }
            std::net::IpAddr::V6(v6) => {
                v6.is_loopback() || v6.is_unspecified() || (v6.segments()[0] & 0xfe00) == 0xfc00
            }
        };
    }
    false
}

impl Core {
    /// Give the app's webview to the core (the desktop app does this at startup).
    pub fn set_browser_host(&self, host: Arc<dyn BrowserHost>) {
        *self.browser_host.lock().unwrap() = Some(host);
    }

    /// Run one browser tool for an agent.
    pub(crate) async fn browser_tool(
        self: &Arc<Self>,
        agent: &Agent,
        name: &str,
        args: &Value,
        token: Option<&CancellationToken>,
    ) -> anyhow::Result<String> {
        anyhow::ensure!(
            agent.settings.browser,
            "the browser is off for this agent: turn it on in its profile → Computer"
        );
        let action = name.trim_start_matches("browser_");
        let mut args = args.clone();
        if name == BROWSER_OPEN {
            let url = checked_url(arg_str(&args, "url").unwrap_or_default())?;
            let host = url.host_str().unwrap_or_default().to_string();
            // A site the agent hasn't used yet asks first (unless it runs fully autonomous).
            let key = format!("{}|{host}", agent.id);
            let known = self.browser_allowed.lock().unwrap().contains(&key);
            if !known && agent.settings.approval_mode == ApprovalMode::Ask {
                let ok = self
                    .ask_permission(
                        "",
                        agent,
                        "",
                        &format!(
                            "Open {host} in the browser and use it (read, click, type).\n{url}"
                        ),
                        token,
                    )
                    .await?;
                anyhow::ensure!(
                    ok,
                    "The user rejected opening {host}. Don't open it; adapt or tell them what you can do instead."
                );
            }
            self.browser_allowed.lock().unwrap().insert(key);
            args["url"] = json!(url.as_str());
        }
        // The Computer panel opens by itself on the Browser tab.
        self.emit(CoreEvent::BrowserActivity {
            agent_id: agent.id.clone(),
        });
        if self.primary {
            let host = self.browser_host.lock().unwrap().clone();
            let host = host.ok_or_else(|| {
                anyhow::anyhow!("the browser is only available in the desktop app")
            })?;
            return host.run(&agent.id, action, args).await;
        }
        self.browser_remote(agent, action, args).await
    }

    /// From a tool server process: ask the running app to do it.
    async fn browser_remote(
        &self,
        agent: &Agent,
        action: &str,
        args: Value,
    ) -> anyhow::Result<String> {
        let id = new_id();
        sqlx::query("INSERT INTO bridge (id, kind, agent_id, thread_id, payload, status, created_at) VALUES (?, 'browser', ?, NULL, ?, 'pending', ?)")
            .bind(&id)
            .bind(&agent.id)
            .bind(json!({ "action": action, "args": args }).to_string())
            .bind(now())
            .execute(&self.store.db)
            .await?;
        let started = std::time::Instant::now();
        loop {
            tokio::time::sleep(POLL).await;
            let row: Option<(String, Option<String>)> =
                sqlx::query_as("SELECT status, result FROM bridge WHERE id = ?")
                    .bind(&id)
                    .fetch_optional(&self.store.db)
                    .await?;
            match row {
                Some((s, result)) if s == "done" => {
                    let text = result.unwrap_or_default();
                    return match text.strip_prefix("Error: ") {
                        Some(e) => Err(anyhow::anyhow!("{e}")),
                        None => Ok(text),
                    };
                }
                Some((s, _)) if s == "cancelled" => {
                    anyhow::bail!("The browser request was cancelled.")
                }
                None => anyhow::bail!("The browser request disappeared."),
                _ => {}
            }
            anyhow::ensure!(
                started.elapsed() < Duration::from_secs(120),
                "The app did not answer. Is OpenCue open?"
            );
        }
    }

    /// The app side of [`Core::browser_remote`].
    pub(crate) async fn browser_serve(&self, agent_id: &str, payload: &str) -> String {
        let v: Value = serde_json::from_str(payload).unwrap_or_default();
        let action = v.get("action").and_then(Value::as_str).unwrap_or_default();
        let args = v.get("args").cloned().unwrap_or_default();
        let host = self.browser_host.lock().unwrap().clone();
        match host {
            Some(h) => h
                .run(agent_id, action, args)
                .await
                .unwrap_or_else(|e| format!("Error: {e:#}")),
            None => "Error: the browser is only available in the desktop app".into(),
        }
    }
}

/// The browser tools an agent with the switch on gets.
pub fn is_browser_tool(name: &str) -> bool {
    matches!(
        name,
        BROWSER_OPEN | BROWSER_SNAPSHOT | BROWSER_CLICK | BROWSER_TYPE | BROWSER_CLOSE
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_web_pages_open() {
        assert_eq!(
            checked_url("example.com/a?b=1").unwrap().as_str(),
            "https://example.com/a?b=1"
        );
        assert!(checked_url("http://example.com").is_ok());
        for bad in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "http://localhost:7777",
            "http://127.0.0.1",
            "http://192.168.1.1/admin",
            "http://10.0.0.5",
            "http://[::1]/",
            "http://printer.local",
            "http://169.254.169.254/latest",
        ] {
            assert!(checked_url(bad).is_err(), "{bad} should be refused");
        }
    }
}
