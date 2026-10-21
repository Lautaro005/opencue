//! Usage limits of the subscription CLIs the user connected (Settings → Usage): the 5-hour and
//! weekly windows, how much is used and when they reset. Codex writes them next to every
//! session it runs; Claude Code reports them in its stream while an agent works, so OpenCue
//! keeps the last ones it saw. Nothing here reads the CLIs' sign-in tokens.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Core;

const CLAUDE_KEY: &str = "claude_rate_limits";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LimitWindow {
    /// `5h`, `weekly` or the window's length.
    pub label: String,
    /// 0–100; `None` when the CLI only said whether the window is open.
    pub used_percent: Option<f64>,
    /// When the window resets (epoch seconds).
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CliLimits {
    pub cli: String,
    pub name: String,
    pub plan: Option<String>,
    pub windows: Vec<LimitWindow>,
    /// When these numbers were read (epoch seconds).
    pub updated_at: Option<i64>,
    pub note: Option<String>,
}

fn label_for(minutes: i64) -> String {
    match minutes {
        300 => "5h".into(),
        10080 => "weekly".into(),
        m if m % 1440 == 0 => format!("{}d", m / 1440),
        m if m % 60 == 0 => format!("{}h", m / 60),
        m => format!("{m}m"),
    }
}

fn label_for_claude(kind: &str) -> String {
    match kind {
        "five_hour" => "5h".into(),
        "seven_day" => "weekly".into(),
        k if k.starts_with("seven_day") => {
            format!("weekly ({})", k.trim_start_matches("seven_day_"))
        }
        k => k.replace('_', " "),
    }
}

/// The windows in a Codex `rate_limits` object, as of `at` (epoch seconds, the line's time).
pub fn codex_windows(limits: &Value, at: i64, now: i64) -> Vec<LimitWindow> {
    ["primary", "secondary"]
        .iter()
        .filter_map(|k| limits.get(*k).filter(|w| w.is_object()))
        .map(|w| {
            let minutes = w.get("window_minutes").and_then(Value::as_i64).unwrap_or(0);
            let resets_at = w.get("resets_at").and_then(Value::as_i64).or_else(|| {
                w.get("resets_in_seconds")
                    .and_then(Value::as_i64)
                    .map(|s| at + s)
            });
            let used = w.get("used_percent").and_then(Value::as_f64).unwrap_or(0.0);
            // A window that has reset since the last reading starts over.
            let over = resets_at.is_some_and(|r| r <= now);
            LimitWindow {
                label: label_for(minutes),
                used_percent: Some(if over { 0.0 } else { used.clamp(0.0, 100.0) }),
                resets_at: if over { None } else { resets_at },
            }
        })
        .collect()
}

fn newest_entry(dir: &Path, want_dir: bool) -> Option<PathBuf> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir() == want_dir)
        .collect();
    entries.sort_by_key(|e| e.file_name());
    entries.pop().map(|e| e.path())
}

/// The newest Codex session file: sessions/YYYY/MM/DD/rollout-….jsonl, newest day first.
fn newest_codex_session(root: &Path) -> Option<PathBuf> {
    let mut dir = root.to_path_buf();
    for _ in 0..3 {
        dir = newest_entry(&dir, true)?;
    }
    // The newest day may be empty: fall back to older days.
    let files = |d: &Path| {
        let mut f: Vec<_> = std::fs::read_dir(d)
            .ok()?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
            .collect();
        f.sort();
        f.pop()
    };
    files(&dir)
}

fn last_codex_limits(file: &Path, now: i64) -> Option<(Vec<LimitWindow>, Option<String>, i64)> {
    let mut f = std::fs::File::open(file).ok()?;
    let len = f.metadata().ok()?.len();
    let take = len.min(400_000);
    f.seek(SeekFrom::Start(len - take)).ok()?;
    let mut buf = String::new();
    f.take(take).read_to_string(&mut buf).ok().or_else(|| {
        buf.clear();
        None
    })?;
    for line in buf.lines().rev() {
        if !line.contains("\"rate_limits\"") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let limits = v
            .pointer("/payload/rate_limits")
            .or_else(|| v.get("rate_limits"));
        let Some(limits) = limits.filter(|l| l.is_object()) else {
            continue;
        };
        let at = v
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
            .map(|t| t.timestamp())
            .unwrap_or(now);
        let windows = codex_windows(limits, at, now);
        if windows.is_empty() {
            continue;
        }
        let plan = limits
            .get("plan_type")
            .and_then(Value::as_str)
            .map(str::to_string);
        return Some((windows, plan, at));
    }
    None
}

impl Core {
    /// Remember a Claude Code `rate_limit_event`, keeping the latest of each window.
    pub(crate) async fn note_claude_limits(&self, info: &Value) {
        let Some(kind) = info.get("rateLimitType").and_then(Value::as_str) else {
            return;
        };
        let mut all: serde_json::Map<String, Value> = self
            .store
            .setting(CLAUDE_KEY)
            .await
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let mut entry = info.clone();
        entry["seenAt"] = Value::from(chrono::Utc::now().timestamp());
        all.insert(kind.to_string(), entry);
        let _ = self
            .store
            .set_setting(CLAUDE_KEY, &Value::Object(all).to_string())
            .await;
    }

    fn claude_limits(&self, saved: Option<String>, now: i64) -> CliLimits {
        let all: serde_json::Map<String, Value> = saved
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let mut windows = vec![];
        let mut updated = None;
        for (kind, e) in &all {
            let resets_at = e.get("resetsAt").and_then(Value::as_i64);
            let over = resets_at.is_some_and(|r| r <= now);
            let used = e.get("utilization").and_then(Value::as_f64).map(|u| {
                if u <= 1.0 {
                    u * 100.0
                } else {
                    u
                }
            });
            windows.push(LimitWindow {
                label: label_for_claude(kind),
                used_percent: if over { Some(0.0) } else { used },
                resets_at: if over { None } else { resets_at },
            });
            updated = updated.max(e.get("seenAt").and_then(Value::as_i64));
        }
        windows.sort_by_key(|w| (w.label != "5h", w.label.clone()));
        CliLimits {
            cli: "claude".into(),
            name: "Claude Code".into(),
            plan: None,
            note: windows
                .is_empty()
                .then(|| "Appears after a Claude Code agent has replied.".into()),
            windows,
            updated_at: updated,
        }
    }

    /// The limits of every subscription CLI that is installed.
    pub async fn cli_limits(&self) -> Vec<CliLimits> {
        let now = chrono::Utc::now().timestamp();
        let mut out = vec![];
        if which::which("claude").is_ok() {
            let saved = self.store.setting(CLAUDE_KEY).await.ok().flatten();
            out.push(self.claude_limits(saved, now));
        }
        if which::which("codex").is_ok() {
            let home = std::env::var_os("CODEX_HOME")
                .map(PathBuf::from)
                .or_else(|| directories::UserDirs::new().map(|d| d.home_dir().join(".codex")));
            let found = home
                .map(|h| h.join("sessions"))
                .and_then(|s| newest_codex_session(&s))
                .and_then(|f| last_codex_limits(&f, now));
            out.push(match found {
                Some((windows, plan, at)) => CliLimits {
                    cli: "codex".into(),
                    name: "Codex".into(),
                    plan,
                    windows,
                    updated_at: Some(at),
                    note: None,
                },
                None => CliLimits {
                    cli: "codex".into(),
                    name: "Codex".into(),
                    plan: None,
                    windows: vec![],
                    updated_at: None,
                    note: Some("Appears after Codex has run a session.".into()),
                },
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn codex_windows_get_labels_percent_and_reset() {
        let l = json!({
            "primary": { "used_percent": 42.5, "window_minutes": 300, "resets_in_seconds": 3600 },
            "secondary": { "used_percent": 8.0, "window_minutes": 10080, "resets_at": 2_000_000_000 },
            "plan_type": "plus"
        });
        let w = codex_windows(&l, 1_000_000_000, 1_000_000_100);
        assert_eq!(w[0].label, "5h");
        assert_eq!(w[0].used_percent, Some(42.5));
        assert_eq!(w[0].resets_at, Some(1_000_003_600));
        assert_eq!(w[1].label, "weekly");
        assert_eq!(w[1].resets_at, Some(2_000_000_000));
    }

    #[test]
    fn a_window_that_reset_starts_over() {
        let l = json!({ "primary": { "used_percent": 90.0, "window_minutes": 300, "resets_in_seconds": 10 } });
        let w = codex_windows(&l, 1_000, 5_000);
        assert_eq!(w[0].used_percent, Some(0.0));
        assert_eq!(w[0].resets_at, None);
    }

    #[test]
    fn labels_read_naturally() {
        assert_eq!(label_for(300), "5h");
        assert_eq!(label_for(10080), "weekly");
        assert_eq!(label_for(1440), "1d");
        assert_eq!(label_for_claude("five_hour"), "5h");
        assert_eq!(label_for_claude("seven_day"), "weekly");
        assert_eq!(label_for_claude("seven_day_opus"), "weekly (opus)");
    }

    #[test]
    fn the_last_rate_limits_line_in_a_session_wins() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("rollout.jsonl");
        std::fs::write(
            &f,
            [
                r#"{"timestamp":"2026-10-20T10:00:00Z","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":10.0,"window_minutes":300,"resets_in_seconds":7200}}}}"#,
                r#"{"timestamp":"2026-10-20T11:00:00Z","type":"other"}"#,
                r#"{"timestamp":"2026-10-20T12:00:00Z","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":30.0,"window_minutes":300,"resets_in_seconds":7200},"plan_type":"pro"}}}"#,
            ]
            .join("\n"),
        )
        .unwrap();
        let (w, plan, _) = last_codex_limits(&f, 0).unwrap();
        assert_eq!(w[0].used_percent, Some(30.0));
        assert_eq!(plan.as_deref(), Some("pro"));
    }
}
