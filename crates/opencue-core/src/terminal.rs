//! Terminal sessions: Claude Code, Codex, Grok Build, Gemini, Mistral Vibe, Muse (or a plain shell)
//! running in a real pseudo-terminal, so you chat with them inside OpenCue exactly as you would in a
//! terminal window. The app draws the terminal (xterm.js); this module starts the program, keeps
//! the last output so a session can be reopened, and passes keys, resizes and output along.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};

use crate::db::{new_id, now};
use crate::events::CoreEvent;
use crate::Core;

/// How much output a session remembers for when it is reopened.
const SCROLLBACK: usize = 400_000;
const SETTING: &str = "terminal_sessions";

/// A command-line tool that can run in a session.
pub struct Tool {
    pub key: &'static str,
    pub label: &'static str,
    pub command: &'static [&'static str],
    pub install: &'static str,
}

pub const TOOLS: &[Tool] = &[
    Tool {
        key: "claude",
        label: "Claude Code",
        command: &["claude"],
        install: "npm install -g @anthropic-ai/claude-code",
    },
    Tool {
        key: "codex",
        label: "Codex",
        command: &["codex"],
        install: "npm install -g @openai/codex",
    },
    Tool {
        key: "grok",
        label: "Grok Build",
        command: &["grok"],
        install: "see x.ai/cli",
    },
    Tool {
        key: "gemini",
        label: "Gemini CLI",
        command: &["gemini"],
        install: "npm install -g @google/gemini-cli",
    },
    Tool {
        key: "agy",
        label: "Antigravity",
        command: &["agy"],
        install: "see antigravity.google",
    },
    Tool {
        key: "vibe",
        label: "Mistral Vibe",
        command: &["vibe"],
        install: "uv tool install mistral-vibe",
    },
    Tool {
        key: "muse",
        label: "Muse Code",
        command: &["muse"],
        install: "see dev.meta.ai",
    },
    Tool {
        key: "shell",
        label: "Terminal",
        command: &[],
        install: "",
    },
];

#[derive(Debug, Clone, Serialize)]
pub struct ToolInfo {
    pub key: String,
    pub label: String,
    pub installed: bool,
    pub install: String,
}

/// A saved session (the program may have ended; it can be started again).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TermInfo {
    pub id: String,
    pub tool: String,
    pub label: String,
    pub cwd: String,
    pub created_at: String,
    #[serde(default)]
    pub alive: bool,
}

struct Live {
    writer: Mutex<Box<dyn Write + Send>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    killer: Mutex<Box<dyn portable_pty::ChildKiller + Send + Sync>>,
    scrollback: Arc<Mutex<Vec<u8>>>,
    alive: Arc<AtomicBool>,
}

#[derive(Default)]
pub(crate) struct Terminals {
    live: Mutex<HashMap<String, Arc<Live>>>,
}

fn tool(key: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|t| t.key == key)
}

/// The program and arguments for a tool, and where to look for it.
fn resolve(t: &Tool) -> anyhow::Result<(PathBuf, Vec<String>)> {
    if t.key == "shell" {
        #[cfg(windows)]
        let shell = which::which("pwsh")
            .or_else(|_| which::which("powershell"))
            .unwrap_or_else(|_| PathBuf::from("cmd.exe"));
        #[cfg(not(windows))]
        let shell = std::env::var_os("SHELL")
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .unwrap_or_else(|| PathBuf::from("/bin/sh"));
        return Ok((shell, vec![]));
    }
    let program = which::which(t.command[0]).map_err(|_| {
        anyhow::anyhow!(
            "{} is not installed on this computer ({}). Install it, then try again.",
            t.label,
            if t.install.is_empty() {
                "see its website".to_string()
            } else {
                format!("`{}`", t.install)
            }
        )
    })?;
    Ok((
        program,
        t.command[1..].iter().map(|s| s.to_string()).collect(),
    ))
}

impl Core {
    pub fn term_tools(&self) -> Vec<ToolInfo> {
        TOOLS
            .iter()
            .map(|t| ToolInfo {
                key: t.key.into(),
                label: t.label.into(),
                installed: t.key == "shell" || which::which(t.command[0]).is_ok(),
                install: t.install.into(),
            })
            .collect()
    }

    async fn term_saved(&self) -> Vec<TermInfo> {
        self.store
            .setting(SETTING)
            .await
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    async fn term_save(&self, list: &[TermInfo]) -> anyhow::Result<()> {
        self.store
            .set_setting(SETTING, &serde_json::to_string(list)?)
            .await
    }

    pub async fn term_list(&self) -> Vec<TermInfo> {
        let saved = self.term_saved().await;
        let live = self.terms.live.lock().unwrap();
        saved
            .into_iter()
            .map(|mut t| {
                t.alive = live
                    .get(&t.id)
                    .is_some_and(|l| l.alive.load(Ordering::SeqCst));
                t
            })
            .collect()
    }

    /// A new session of `tool_key` in `cwd`.
    pub async fn term_create(
        self: &Arc<Self>,
        tool_key: &str,
        cwd: &str,
        cols: u16,
        rows: u16,
    ) -> anyhow::Result<TermInfo> {
        let t = tool(tool_key).ok_or_else(|| anyhow::anyhow!("unknown tool `{tool_key}`"))?;
        let dir = PathBuf::from(cwd.trim());
        anyhow::ensure!(dir.is_dir(), "{} is not a folder", dir.display());
        let info = TermInfo {
            id: new_id(),
            tool: t.key.into(),
            label: t.label.into(),
            cwd: dir.display().to_string(),
            created_at: now(),
            alive: false,
        };
        self.term_spawn(&info, cols, rows)?;
        let mut list = self.term_saved().await;
        list.push(info.clone());
        self.term_save(&list).await?;
        self.emit(CoreEvent::ThreadsChanged);
        Ok(TermInfo {
            alive: true,
            ..info
        })
    }

    /// Start the program of a saved session again (after it ended, or the app restarted).
    pub async fn term_restart(
        self: &Arc<Self>,
        id: &str,
        cols: u16,
        rows: u16,
    ) -> anyhow::Result<()> {
        let info = self
            .term_saved()
            .await
            .into_iter()
            .find(|t| t.id == id)
            .ok_or_else(|| anyhow::anyhow!("no such session"))?;
        self.term_kill(id);
        self.term_spawn(&info, cols, rows)?;
        self.emit(CoreEvent::ThreadsChanged);
        Ok(())
    }

    fn term_spawn(self: &Arc<Self>, info: &TermInfo, cols: u16, rows: u16) -> anyhow::Result<()> {
        let t = tool(&info.tool).ok_or_else(|| anyhow::anyhow!("unknown tool"))?;
        let (program, args) = resolve(t)?;
        let pair = native_pty_system().openpty(PtySize {
            rows: rows.max(2),
            cols: cols.max(10),
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let mut cmd = CommandBuilder::new(&program);
        cmd.args(&args);
        cmd.cwd(&info.cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| anyhow::anyhow!("could not start {}: {e}", program.display()))?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let live = Arc::new(Live {
            writer: Mutex::new(writer),
            master: Mutex::new(pair.master),
            killer: Mutex::new(child.clone_killer()),
            scrollback: Arc::new(Mutex::new(Vec::new())),
            alive: Arc::new(AtomicBool::new(true)),
        });
        self.terms
            .live
            .lock()
            .unwrap()
            .insert(info.id.clone(), live.clone());

        let core = self.clone();
        let id = info.id.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        {
                            let mut sb = live.scrollback.lock().unwrap();
                            sb.extend_from_slice(&buf[..n]);
                            let extra = sb.len().saturating_sub(SCROLLBACK);
                            if extra > 0 {
                                sb.drain(..extra);
                            }
                        }
                        core.emit(CoreEvent::TermOutput {
                            id: id.clone(),
                            data: base64::engine::general_purpose::STANDARD.encode(&buf[..n]),
                        });
                    }
                }
            }
            let code = child.wait().map(|s| s.exit_code() as i32).unwrap_or(-1);
            live.alive.store(false, Ordering::SeqCst);
            core.emit(CoreEvent::TermExit { id, code });
        });
        Ok(())
    }

    fn term_live(&self, id: &str) -> Option<Arc<Live>> {
        self.terms.live.lock().unwrap().get(id).cloned()
    }

    pub fn term_write(&self, id: &str, data: &str) -> anyhow::Result<()> {
        let live = self
            .term_live(id)
            .filter(|l| l.alive.load(Ordering::SeqCst))
            .ok_or_else(|| anyhow::anyhow!("the session has ended"))?;
        let mut w = live.writer.lock().unwrap();
        w.write_all(data.as_bytes())?;
        w.flush()?;
        Ok(())
    }

    pub fn term_resize(&self, id: &str, cols: u16, rows: u16) {
        if let Some(live) = self.term_live(id) {
            let _ = live.master.lock().unwrap().resize(PtySize {
                rows: rows.max(2),
                cols: cols.max(10),
                pixel_width: 0,
                pixel_height: 0,
            });
        }
    }

    /// What the session has printed so far (base64), to draw it again when it is reopened.
    pub fn term_snapshot(&self, id: &str) -> String {
        self.term_live(id)
            .map(|l| {
                base64::engine::general_purpose::STANDARD.encode(&*l.scrollback.lock().unwrap())
            })
            .unwrap_or_default()
    }

    /// Stop the program (the session stays in the list).
    pub fn term_kill(&self, id: &str) {
        if let Some(live) = self.terms.live.lock().unwrap().remove(id) {
            let _ = live.killer.lock().unwrap().kill();
        }
    }

    /// Stop the program and forget the session.
    pub async fn term_close(&self, id: &str) -> anyhow::Result<()> {
        self.term_kill(id);
        let list: Vec<TermInfo> = self
            .term_saved()
            .await
            .into_iter()
            .filter(|t| t.id != id)
            .collect();
        self.term_save(&list).await?;
        self.emit(CoreEvent::ThreadsChanged);
        Ok(())
    }

    pub async fn term_rename(&self, id: &str, label: &str) -> anyhow::Result<()> {
        let mut list = self.term_saved().await;
        if let Some(t) = list.iter_mut().find(|t| t.id == id) {
            t.label = label.trim().chars().take(60).collect();
        }
        self.term_save(&list).await?;
        self.emit(CoreEvent::ThreadsChanged);
        Ok(())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_shell_session_runs_and_remembers_its_output() {
        let dir = tempfile::tempdir().unwrap();
        let core = crate::Core::open(crate::paths::Paths::at(dir.path()).unwrap())
            .await
            .unwrap();
        let mut events = core.subscribe();
        let info = core
            .term_create("shell", dir.path().to_str().unwrap(), 80, 24)
            .await
            .unwrap();
        assert!(info.alive);
        core.term_write(&info.id, "echo opencue-$((20+22))\nexit\n")
            .unwrap();
        let mut seen = String::new();
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while let Ok(ev) = events.recv().await {
                match ev {
                    CoreEvent::TermOutput { data, .. } => seen.push_str(&String::from_utf8_lossy(
                        &base64::engine::general_purpose::STANDARD
                            .decode(data)
                            .unwrap(),
                    )),
                    CoreEvent::TermExit { .. } => break,
                    _ => {}
                }
            }
        })
        .await
        .expect("the session ended");
        assert!(seen.contains("opencue-42"), "output was: {seen}");
        let again = String::from_utf8_lossy(
            &base64::engine::general_purpose::STANDARD
                .decode(core.term_snapshot(&info.id))
                .unwrap(),
        )
        .to_string();
        assert!(again.contains("opencue-42"));
        assert_eq!(core.term_list().await.len(), 1);
        core.term_close(&info.id).await.unwrap();
        assert!(core.term_list().await.is_empty());
        assert!(core.term_create("nonsense", "/", 80, 24).await.is_err());
    }

    #[test]
    fn tools_have_unique_keys() {
        let mut keys: Vec<&str> = TOOLS.iter().map(|t| t.key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), TOOLS.len());
    }
}
