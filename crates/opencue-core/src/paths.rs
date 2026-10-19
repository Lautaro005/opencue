//! Platform data locations. `OPENCUE_HOME` overrides everything (handy for tests and portable installs).

use std::path::{Path, PathBuf};

/// Apps opened from the Dock or Finder inherit a bare PATH (no Homebrew, npm or `~/.local`), so
/// `claude`, `codex`, `grok`, `npx` and `docker` would not be found. Ask the user's login shell for
/// its PATH and add the usual install folders. Call once at start-up, before other threads run.
pub fn fix_path_env() {
    #[cfg(unix)]
    {
        let mut dirs: Vec<std::path::PathBuf> = Vec::new();
        let mut add = |p: std::path::PathBuf| {
            if !p.as_os_str().is_empty() && !dirs.contains(&p) {
                dirs.push(p);
            }
        };
        if let Some(shell) = std::env::var_os("SHELL").or_else(|| Some("/bin/zsh".into())) {
            // A timeout keeps a slow shell profile from blocking start-up.
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let out = std::process::Command::new(shell)
                    .args(["-ilc", "printf '__OC_PATH__%s__OC_PATH__' \"$PATH\""])
                    .stdin(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .output();
                let _ = tx.send(out);
            });
            if let Ok(Ok(out)) = rx.recv_timeout(std::time::Duration::from_secs(4)) {
                let text = String::from_utf8_lossy(&out.stdout);
                if let Some(path) = text.split("__OC_PATH__").nth(1).filter(|p| !p.is_empty()) {
                    std::env::split_paths(path).for_each(&mut add);
                }
            }
        }
        if let Some(current) = std::env::var_os("PATH") {
            std::env::split_paths(&current).for_each(&mut add);
        }
        if let Some(home) = directories::UserDirs::new().map(|d| d.home_dir().to_path_buf()) {
            for rel in [
                ".local/bin",
                ".cargo/bin",
                ".bun/bin",
                ".npm-global/bin",
                ".grok/bin",
                ".claude/local",
            ] {
                add(home.join(rel));
            }
        }
        for abs in ["/opt/homebrew/bin", "/opt/homebrew/sbin", "/usr/local/bin"] {
            add(abs.into());
        }
        if let Ok(joined) = std::env::join_paths(dirs) {
            std::env::set_var("PATH", joined);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    pub fn resolve() -> anyhow::Result<Self> {
        if let Ok(home) = std::env::var("OPENCUE_HOME") {
            return Self::at(home);
        }
        let dirs = directories::ProjectDirs::from("", "", "opencue").ok_or_else(|| {
            anyhow::anyhow!("could not determine a data directory for this platform")
        })?;
        Self::at(dirs.data_dir())
    }

    pub fn at(root: impl AsRef<Path>) -> anyhow::Result<Self> {
        let paths = Self {
            root: root.as_ref().to_path_buf(),
        };
        for dir in [
            paths.root(),
            &paths.agents_dir(),
            &paths.skills_dir(),
            &paths.memory_dir(),
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(paths)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn database(&self) -> PathBuf {
        self.root.join("opencue.db")
    }

    pub fn agents_dir(&self) -> PathBuf {
        self.root.join("agents")
    }

    pub fn agent_dir(&self, agent_id: &str) -> PathBuf {
        self.agents_dir().join(agent_id)
    }

    pub fn instructions_file(&self, agent_id: &str) -> PathBuf {
        self.agent_dir(agent_id).join("INSTRUCTIONS.md")
    }

    /// Who the agent is (personality, tone, values), written by the agent as it learns.
    pub fn soul_file(&self, agent_id: &str) -> PathBuf {
        self.agent_dir(agent_id).join("SOUL.md")
    }

    /// The agent's running plan, restated at the end of its prompt on every step.
    pub fn plan_file(&self, agent_id: &str) -> PathBuf {
        self.agent_dir(agent_id).join("PLAN.md")
    }

    pub fn agent_memory_file(&self, agent_id: &str) -> PathBuf {
        self.agent_dir(agent_id).join("MEMORY.md")
    }

    pub fn memory_dir(&self) -> PathBuf {
        self.root.join("memory")
    }

    pub fn global_memory_file(&self) -> PathBuf {
        self.memory_dir().join("GLOBAL.md")
    }

    pub fn skills_dir(&self) -> PathBuf {
        self.root.join("skills")
    }

    /// Files shared in chats: `files/<attachment id>/<name>`.
    pub fn files_dir(&self) -> PathBuf {
        self.root.join("files")
    }

    pub fn attachment_file(&self, id: &str, name: &str) -> PathBuf {
        self.files_dir().join(id).join(name)
    }

    pub fn agent_workspace(&self, agent_id: &str) -> PathBuf {
        self.agent_dir(agent_id).join("workspace")
    }

    /// Workspace and VM home of the Docker computer that all agents share.
    pub fn shared_dir(&self) -> PathBuf {
        self.root.join("shared")
    }

    pub fn secrets_fallback_file(&self) -> PathBuf {
        self.root.join("secrets.json")
    }
}
