//! Platform data locations. `OPENCUE_HOME` overrides everything (handy for tests and portable installs).

use std::path::{Path, PathBuf};

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

    pub fn secrets_fallback_file(&self) -> PathBuf {
        self.root.join("secrets.json")
    }
}
