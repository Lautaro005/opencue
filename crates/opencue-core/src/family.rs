//! The rest of the family: brain (memory), Lumina (meetings) and AI WorkStudio (the workspace that
//! ties them together). OpenCue does not embed them: it finds them on this computer, starts them,
//! and hands their MCP servers to its agents with one switch, the same way WorkStudio does.
//!
//! * brain: a Python MCP server in `~/.brain`, dashboard on 127.0.0.1:8765.
//! * Lumina: `lumina mcp`, app on 127.0.0.1:47770.
//! * WorkStudio: `workstudio mcp`, one server with the tools of the others. When it is connected
//!   the direct brain/Lumina entries are paused so no tool shows up twice.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use crate::models::{McpServerConfig, McpTransport};
use crate::Core;

pub const IDS: [&str; 3] = ["brain", "lumina", "workstudio"];
const PAUSED: &str = "family_paused";

#[derive(Debug, Clone, Serialize, Default)]
pub struct FamilyApp {
    pub id: String,
    pub name: String,
    pub installed: bool,
    /// None when the app has no port to ask (WorkStudio).
    pub running: Option<bool>,
    pub version: Option<String>,
    pub home: Option<String>,
    /// Where its web app answers, if it has one.
    pub url: Option<String>,
    /// `off`, `on` (its own MCP server) or `via_workstudio` (the direct entry is paused).
    pub mcp: String,
    pub install_command: String,
    pub repo_url: String,
    pub can_start: bool,
}

fn home_dir() -> Option<PathBuf> {
    directories::UserDirs::new().map(|d| d.home_dir().to_path_buf())
}

fn app_home(id: &str) -> Option<PathBuf> {
    let (var, folder) = match id {
        "brain" => ("BRAIN_HOME", ".brain"),
        "lumina" => ("LUMINA_HOME", ".lumina"),
        _ => return None,
    };
    std::env::var_os(var)
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|h| h.join(folder)))
}

/// A command by name: on the PATH, or where the installers put it.
fn command(name: &str) -> Option<PathBuf> {
    which::which(name).ok().or_else(|| {
        let h = home_dir()?;
        [".local/bin", ".cargo/bin", "bin"]
            .iter()
            .map(|d| h.join(d).join(name))
            .chain(
                (cfg!(windows)).then(|| h.join(".local").join("bin").join(format!("{name}.cmd"))),
            )
            .find(|p| p.is_file())
    })
}

fn workstudio_installed() -> bool {
    if command("workstudio").is_some() {
        return true;
    }
    let mut spots = vec![PathBuf::from("/Applications/AI WorkStudio.app")];
    if let Some(h) = home_dir() {
        spots.push(h.join("Applications").join("AI WorkStudio.app"));
        spots.push(h.join(".local").join("opt").join("workstudio"));
    }
    spots.iter().any(|p| p.exists())
}

fn installed(id: &str) -> bool {
    match id {
        "brain" => app_home("brain").is_some_and(|h| h.join("server.py").is_file()),
        "lumina" => {
            command("lumina").is_some()
                || app_home("lumina").is_some_and(|h| h.join("lumina.sh").is_file())
        }
        _ => workstudio_installed(),
    }
}

fn version(id: &str) -> Option<String> {
    let home = app_home(id)?;
    let text = std::fs::read_to_string(home.join("VERSION")).ok()?;
    Some(text.trim().trim_start_matches('v').to_string()).filter(|v| !v.is_empty())
}

async fn answers(url: &str, needle: &str) -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_millis(900))
        .build()
    else {
        return false;
    };
    match client.get(url).send().await {
        Ok(r) => r.text().await.is_ok_and(|t| t.contains(needle)),
        Err(_) => false,
    }
}

fn url_of(id: &str) -> Option<&'static str> {
    match id {
        "brain" => Some("http://127.0.0.1:8765"),
        "lumina" => Some("http://127.0.0.1:47770"),
        _ => None,
    }
}

/// The MCP server OpenCue would start for this app, if it is installed.
pub fn mcp_config(id: &str) -> Option<McpServerConfig> {
    let (name, program, args): (&str, PathBuf, Vec<String>) = match id {
        "brain" => (
            "brain",
            command("uv")?,
            vec![
                "run".into(),
                "--directory".into(),
                app_home("brain")?.display().to_string(),
                "python".into(),
                "server.py".into(),
            ],
        ),
        "lumina" => ("Lumina", command("lumina")?, vec!["mcp".into()]),
        "workstudio" => {
            // Its own tools (the ones for OpenCue) would loop back here, so only the others.
            let only: Vec<&str> = ["brain", "lumina"]
                .into_iter()
                .filter(|a| installed(a))
                .chain(["web"])
                .collect();
            (
                "WorkStudio",
                command("workstudio")?,
                vec!["mcp".into(), "--only".into(), only.join(",")],
            )
        }
        _ => return None,
    };
    Some(McpServerConfig {
        id: format!("family-{id}"),
        name: name.into(),
        transport: McpTransport::Stdio,
        command: Some(program.display().to_string()),
        args,
        env: Default::default(),
        url: None,
        headers: Default::default(),
        enabled: true,
    })
}

fn install_command(id: &str) -> &'static str {
    match id {
        "brain" => "curl -fsSL https://raw.githubusercontent.com/Lautaro005/brain/main/install.sh | bash",
        "lumina" => "curl -fsSL https://raw.githubusercontent.com/Lautaro005/lumina/main/install.sh | sh",
        _ => "curl -fsSL https://raw.githubusercontent.com/Lautaro005/workstudio/main/install.sh | sh -s -- --all",
    }
}

fn repo_url(id: &str) -> &'static str {
    match id {
        "brain" => "https://github.com/Lautaro005/brain",
        "lumina" => "https://github.com/Lautaro005/lumina",
        _ => "https://github.com/Lautaro005/workstudio",
    }
}

fn display_name(id: &str) -> &'static str {
    match id {
        "brain" => "brain",
        "lumina" => "Lumina",
        _ => "AI WorkStudio",
    }
}

impl Core {
    /// What is installed, what is running and what is wired to the agents.
    pub async fn family_status(&self) -> Vec<FamilyApp> {
        let servers = self.store.list_mcp_servers().await.unwrap_or_default();
        let on = |id: &str| {
            servers
                .iter()
                .any(|s| s.id == format!("family-{id}") && s.enabled)
        };
        let paused = self
            .store
            .setting(PAUSED)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        let (brain_up, lumina_up) = tokio::join!(
            answers("http://127.0.0.1:8765/api/version", "current"),
            answers("http://127.0.0.1:47770/api/status", "stt_engine")
        );
        IDS.iter()
            .map(|id| {
                let is_installed = installed(id);
                let mcp = if on(id) {
                    "on"
                } else if paused.split(',').any(|p| p == *id) && on("workstudio") {
                    "via_workstudio"
                } else {
                    "off"
                };
                FamilyApp {
                    id: id.to_string(),
                    name: display_name(id).into(),
                    installed: is_installed,
                    running: match *id {
                        "brain" => Some(brain_up),
                        "lumina" => Some(lumina_up),
                        _ => None,
                    },
                    version: version(id),
                    home: app_home(id).map(|h| h.display().to_string()),
                    url: url_of(id).map(str::to_string),
                    mcp: mcp.into(),
                    install_command: install_command(id).into(),
                    repo_url: repo_url(id).into(),
                    can_start: is_installed,
                }
            })
            .collect()
    }

    /// Give the agents the app's tools, or take them away.
    pub async fn family_set_mcp(
        self: &Arc<Self>,
        id: &str,
        enabled: bool,
    ) -> anyhow::Result<Vec<FamilyApp>> {
        anyhow::ensure!(IDS.contains(&id), "unknown app");
        let servers = self.store.list_mcp_servers().await?;
        let key = format!("family-{id}");
        if enabled {
            anyhow::ensure!(
                installed(id),
                "{} is not installed on this computer",
                display_name(id)
            );
            let cfg = mcp_config(id).ok_or_else(|| {
                anyhow::anyhow!(
                    "{} is installed, but its command was not found{}",
                    display_name(id),
                    if id == "brain" {
                        " (brain needs `uv`)"
                    } else {
                        ""
                    }
                )
            })?;
            self.save_mcp_server(cfg).await?;
            if id == "workstudio" {
                // One server with everything: pause the direct ones so no tool shows up twice.
                let mut paused = vec![];
                for other in ["brain", "lumina"] {
                    if let Some(s) = servers
                        .iter()
                        .find(|s| s.id == format!("family-{other}") && s.enabled)
                    {
                        let mut s = s.clone();
                        s.enabled = false;
                        self.save_mcp_server(s).await?;
                        paused.push(other);
                    }
                }
                self.store.set_setting(PAUSED, &paused.join(",")).await?;
            }
        } else if let Some(s) = servers.iter().find(|s| s.id == key) {
            let mut s = s.clone();
            s.enabled = false;
            self.save_mcp_server(s).await?;
            if id == "workstudio" {
                let paused = self.store.setting(PAUSED).await?.unwrap_or_default();
                for other in paused.split(',').filter(|p| !p.is_empty()) {
                    if let Some(s) = servers.iter().find(|s| s.id == format!("family-{other}")) {
                        let mut s = s.clone();
                        s.enabled = true;
                        self.save_mcp_server(s).await?;
                    }
                }
                self.store.set_setting(PAUSED, "").await?;
            }
        }
        Ok(self.family_status().await)
    }

    /// Start an app (its server, or the app itself) without waiting for it.
    pub async fn family_start(&self, id: &str) -> anyhow::Result<()> {
        anyhow::ensure!(installed(id), "{} is not installed", display_name(id));
        let (program, args): (PathBuf, Vec<String>) = match id {
            "brain" => (
                command("brain")
                    .unwrap_or_else(|| app_home("brain").unwrap_or_default().join("brain.sh")),
                vec!["--no-browser".into()],
            ),
            "lumina" => (
                command("lumina")
                    .ok_or_else(|| anyhow::anyhow!("the `lumina` command was not found"))?,
                vec!["serve".into(), "--port".into(), "47770".into()],
            ),
            _ => {
                if cfg!(target_os = "macos") && command("workstudio").is_none() {
                    (
                        PathBuf::from("open"),
                        vec!["-a".into(), "AI WorkStudio".into()],
                    )
                } else {
                    (
                        command("workstudio").ok_or_else(|| {
                            anyhow::anyhow!("the `workstudio` command was not found")
                        })?,
                        vec![],
                    )
                }
            }
        };
        let mut cmd = std::process::Command::new(program);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0008);
        }
        cmd.spawn()
            .map_err(|e| anyhow::anyhow!("could not start {}: {e}", display_name(id)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_app_has_names_and_install_commands() {
        for id in IDS {
            assert!(!display_name(id).is_empty());
            assert!(install_command(id).starts_with("curl"));
            assert!(repo_url(id).contains("Lautaro005"));
        }
        assert!(mcp_config("nope").is_none());
    }
}
