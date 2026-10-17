//! Automations: a trigger, an agent and a prompt. Triggers are schedules (cron), new email,
//! a local webhook, or a message in a group chat. They run while OpenCue is open or in the tray.

use std::str::FromStr;

use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};

fn default_poll() -> u32 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Trigger {
    /// Five-field cron in local time, e.g. `0 9 * * 1-5`.
    Schedule { cron: String },
    /// New mail in the agent's inbox; `filter` matches sender or subject (case-insensitive).
    Email {
        #[serde(default)]
        filter: String,
        #[serde(default = "default_poll")]
        poll_minutes: u32,
    },
    /// `POST http://127.0.0.1:<port>/hooks/<token>`; the body becomes trigger data.
    Webhook { token: String },
    /// A user message in a group chat, optionally containing `keyword`.
    GroupMessage {
        group_id: String,
        #[serde(default)]
        keyword: String,
    },
}

impl Trigger {
    pub fn kind(&self) -> &'static str {
        match self {
            Trigger::Schedule { .. } => "schedule",
            Trigger::Email { .. } => "email",
            Trigger::Webhook { .. } => "webhook",
            Trigger::GroupMessage { .. } => "group_message",
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        match self {
            Trigger::Schedule { cron } => {
                croner::Cron::from_str(cron.trim())
                    .map_err(|e| anyhow::anyhow!("invalid schedule `{cron}`: {e}"))?;
            }
            Trigger::Webhook { token } => {
                anyhow::ensure!(
                    token.len() >= 16 && token.chars().all(|c| c.is_ascii_alphanumeric()),
                    "webhook token must be at least 16 letters or digits"
                );
            }
            Trigger::GroupMessage { group_id, .. } => {
                anyhow::ensure!(!group_id.is_empty(), "pick a group")
            }
            Trigger::Email { .. } => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Automation {
    pub id: String,
    pub agent_id: String,
    pub name: String,
    pub trigger: Trigger,
    pub prompt: String,
    pub enabled: bool,
    pub last_run_at: Option<String>,
    pub last_status: Option<String>,
    pub created_at: String,
    /// Next scheduled run (schedules only), computed when listing.
    #[serde(default)]
    pub next_run_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationInput {
    #[serde(default)]
    pub id: Option<String>,
    pub agent_id: String,
    pub name: String,
    pub trigger: Trigger,
    pub prompt: String,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

/// The first run strictly after `after`.
pub fn next_run(cron: &str, after: DateTime<Local>) -> Option<DateTime<Local>> {
    let c = croner::Cron::from_str(cron.trim()).ok()?;
    c.find_next_occurrence(&after, false).ok()
}

#[derive(Debug, PartialEq, Eq)]
pub enum Due {
    NotYet,
    Fire,
    /// Missed while the app was closed: record it as handled without running.
    Skip,
}

/// Should a schedule fire now? `since` is the last run (or creation). Runs missed by more than
/// `grace` are skipped rather than replayed.
pub fn schedule_due(
    cron: &str,
    since: DateTime<Local>,
    now: DateTime<Local>,
    grace: chrono::Duration,
) -> Option<Due> {
    let next = next_run(cron, since)?;
    Some(if next > now {
        Due::NotYet
    } else if now - next <= grace {
        Due::Fire
    } else {
        Due::Skip
    })
}

pub fn parse_ts(s: &str) -> Option<DateTime<Local>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc).with_timezone(&Local))
}

pub fn new_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// A parsed HTTP/1.1 request, just enough for webhooks.
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub body: Vec<u8>,
}

/// Read one request from a socket (headers up to 16 KiB, body up to 256 KiB).
pub async fn read_http(stream: &mut tokio::net::TcpStream) -> anyhow::Result<HttpRequest> {
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let n = stream.read(&mut chunk).await?;
        anyhow::ensure!(n > 0, "connection closed");
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
        anyhow::ensure!(buf.len() < 16 * 1024, "headers too large");
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = head.lines();
    let first = lines.next().unwrap_or_default();
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    let length: usize = lines
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse().ok())
        .unwrap_or(0);
    anyhow::ensure!(length <= 256 * 1024, "body too large");
    let mut body = buf[header_end..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(length);
    Ok(HttpRequest { method, path, body })
}

pub async fn write_http(stream: &mut tokio::net::TcpStream, status: &str, body: &str) {
    use tokio::io::AsyncWriteExt;
    let res = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(res.as_bytes()).await;
    let _ = stream.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn schedules_fire_once_and_skip_stale_runs() {
        let since = Local.with_ymd_and_hms(2026, 10, 17, 8, 0, 0).unwrap();
        let grace = chrono::Duration::minutes(10);
        let at = |h, m| Local.with_ymd_and_hms(2026, 10, 17, h, m, 0).unwrap();
        assert_eq!(
            schedule_due("0 9 * * *", since, at(8, 59), grace),
            Some(Due::NotYet)
        );
        assert_eq!(
            schedule_due("0 9 * * *", since, at(9, 1), grace),
            Some(Due::Fire)
        );
        assert_eq!(
            schedule_due("0 9 * * *", since, at(13, 0), grace),
            Some(Due::Skip)
        );
        assert_eq!(schedule_due("nonsense", since, at(9, 1), grace), None);
    }

    #[test]
    fn triggers_round_trip_as_tagged_json() {
        let t: Trigger = serde_json::from_str(r#"{"kind":"email","filter":"juzgado"}"#).unwrap();
        assert_eq!(
            t,
            Trigger::Email {
                filter: "juzgado".into(),
                poll_minutes: 5
            }
        );
        assert!(Trigger::Schedule {
            cron: "*/5 * * * *".into()
        }
        .validate()
        .is_ok());
        assert!(Trigger::Webhook {
            token: "short".into()
        }
        .validate()
        .is_err());
    }
}
