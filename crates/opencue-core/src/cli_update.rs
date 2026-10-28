//! Is the coding CLI the user has installed behind the latest release, and update it from the app.
//! The installed version comes from `<cli> --version`; the latest from the npm registry or PyPI
//! (the only places Claude Code, Codex and Vibe publish to). CLIs with no public registry are
//! left out rather than guessed at.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CliVersion {
    /// The provider kind (`claude_cli`, `codex_cli`, `vibe_cli`).
    pub kind: String,
    pub installed: Option<String>,
    pub latest: Option<String>,
    pub outdated: bool,
}

struct Tool {
    kind: &'static str,
    binary: &'static str,
    npm: Option<&'static str>,
    pypi: Option<&'static str>,
}

const TOOLS: &[Tool] = &[
    Tool {
        kind: "claude_cli",
        binary: "claude",
        npm: Some("@anthropic-ai/claude-code"),
        pypi: None,
    },
    Tool {
        kind: "codex_cli",
        binary: "codex",
        npm: Some("@openai/codex"),
        pypi: None,
    },
    Tool {
        kind: "vibe_cli",
        binary: "vibe",
        npm: None,
        pypi: Some("mistral-vibe"),
    },
];

static LATEST: Mutex<Vec<(String, Instant, Option<String>)>> = Mutex::new(Vec::new());
const FRESH: Duration = Duration::from_secs(3600);

/// The first `1.2.3`-looking token in a CLI's `--version` output.
pub fn parse_version(text: &str) -> Option<String> {
    text.split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .map(|t| t.trim_matches('.'))
        .find(|t| t.contains('.') && t.split('.').all(|p| !p.is_empty() && p.len() < 9))
        .map(str::to_string)
}

fn parts(v: &str) -> Vec<u64> {
    v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// Whether `latest` is a newer version than `installed`.
pub fn is_newer(latest: &str, installed: &str) -> bool {
    let (a, b) = (parts(latest), parts(installed));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x > y;
        }
    }
    false
}

async fn installed(binary: &str) -> Option<String> {
    let path = which::which(binary).ok()?;
    let mut cmd = Command::new(path);
    cmd.arg("--version")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    let out = tokio::time::timeout(Duration::from_secs(10), cmd.output())
        .await
        .ok()?
        .ok()?;
    parse_version(&String::from_utf8_lossy(&out.stdout))
        .or_else(|| parse_version(&String::from_utf8_lossy(&out.stderr)))
}

async fn latest(tool: &Tool) -> Option<String> {
    let key = tool.kind.to_string();
    if let Some((_, at, v)) = LATEST.lock().unwrap().iter().find(|(k, _, _)| *k == key) {
        if at.elapsed() < FRESH {
            return v.clone();
        }
    }
    let url = match (tool.npm, tool.pypi) {
        (Some(p), _) => format!(
            "https://registry.npmjs.org/{}/latest",
            p.replace('/', "%2F")
        ),
        (_, Some(p)) => format!("https://pypi.org/pypi/{p}/json"),
        _ => return None,
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("OpenCue")
        .build()
        .ok()?;
    let v: serde_json::Value = client.get(url).send().await.ok()?.json().await.ok()?;
    let found = v["version"]
        .as_str()
        .or_else(|| v["info"]["version"].as_str())
        .map(str::to_string);
    let mut cache = LATEST.lock().unwrap();
    cache.retain(|(k, _, _)| *k != key);
    cache.push((key, Instant::now(), found.clone()));
    found
}

/// The installed and latest version of every CLI that has one installed.
pub async fn versions() -> Vec<CliVersion> {
    let mut out = vec![];
    for tool in TOOLS {
        let Some(have) = installed(tool.binary).await else {
            continue;
        };
        let newest = latest(tool).await;
        let outdated = newest.as_deref().is_some_and(|n| is_newer(n, &have));
        out.push(CliVersion {
            kind: tool.kind.into(),
            installed: Some(have),
            latest: newest,
            outdated,
        });
    }
    out
}

/// What to run to update a CLI.
fn update_command(tool: &Tool) -> Option<(String, Vec<String>)> {
    let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    match tool.kind {
        // Claude Code updates itself, whichever way it was installed.
        "claude_cli" => Some(("claude".into(), args(&["update"]))),
        "codex_cli" => {
            let path = which::which("codex").ok()?.display().to_string();
            if path.contains("Cellar") || path.contains("homebrew") {
                let brew = which::which("brew").ok()?;
                Some((brew.display().to_string(), args(&["upgrade", "codex"])))
            } else {
                let npm = which::which("npm").ok()?;
                Some((
                    npm.display().to_string(),
                    args(&["install", "-g", "@openai/codex@latest"]),
                ))
            }
        }
        "vibe_cli" => {
            if let Ok(uv) = which::which("uv") {
                Some((
                    uv.display().to_string(),
                    args(&["tool", "upgrade", "mistral-vibe"]),
                ))
            } else {
                let pipx = which::which("pipx").ok()?;
                Some((
                    pipx.display().to_string(),
                    args(&["upgrade", "mistral-vibe"]),
                ))
            }
        }
        _ => None,
    }
}

/// Update one CLI and say what it printed (or why it could not).
pub async fn update(kind: &str) -> anyhow::Result<String> {
    let tool = TOOLS
        .iter()
        .find(|t| t.kind == kind)
        .ok_or_else(|| anyhow::anyhow!("OpenCue cannot update this CLI"))?;
    let (program, args) = update_command(tool).ok_or_else(|| {
        anyhow::anyhow!(
            "Could not tell how {} was installed. Update it from a terminal.",
            tool.binary
        )
    })?;
    let mut cmd = Command::new(&program);
    cmd.args(&args)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    let out = tokio::time::timeout(Duration::from_secs(300), cmd.output())
        .await
        .map_err(|_| anyhow::anyhow!("The update took too long"))??;
    LATEST.lock().unwrap().retain(|(k, _, _)| k != kind);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let tail: String = text
        .lines()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    anyhow::ensure!(
        out.status.success(),
        "{}",
        if tail.trim().is_empty() {
            format!("`{program}` failed")
        } else {
            tail
        }
    );
    Ok(tail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_versions_and_compares_them() {
        assert_eq!(parse_version("2.1.4 (Claude Code)"), Some("2.1.4".into()));
        assert_eq!(parse_version("codex-cli 0.46.0"), Some("0.46.0".into()));
        assert_eq!(parse_version("vibe v1.2"), Some("1.2".into()));
        assert_eq!(parse_version("no numbers"), None);
        assert!(is_newer("2.1.10", "2.1.9"));
        assert!(!is_newer("2.1", "2.1.0"));
        assert!(!is_newer("1.0.0", "1.0.1"));
    }
}
