//! brain, the family's second brain, as the agents' notebook. When it is installed and the user
//! turns it on (Settings → Apps → WorkStudio → brain), every agent, every group and the agents as a
//! whole get a folder of their own inside the brain vault, written the way brain's FORMAT.md asks:
//!
//! ```text
//! projects/opencue-agents.md            the project note, with a map of the folders below
//! projects/opencue-agents/<agent>/      one per agent (each group: group-<name>, everyone: shared)
//!     index.md    what is in the folder, one line per note (read first, then open what matters)
//!     profile.md  the agent's instructions and soul
//!     memory.md   what it remembers
//!     <topic>.md  insights it saved with `brain_save`
//! ```
//!
//! Files are only rewritten when their content changed, so syncing after every reply is cheap.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::models::{Agent, AgentPatch, Group};
use crate::Core;

/// `true` when the user turned the sync on.
pub const SYNC_SETTING: &str = "brain_sync";
/// The folder inside the vault that holds the agents' folders.
pub const ROOT_SETTING: &str = "brain_root";
pub const DEFAULT_ROOT: &str = "projects/opencue-agents";
/// The most of a note `brain_read` returns.
const READ_LIMIT: usize = 60_000;

#[derive(Debug, Clone, serde::Serialize)]
pub struct BrainInfo {
    pub installed: bool,
    pub vault: Option<String>,
    pub enabled: bool,
    pub root: String,
    pub shared: bool,
    /// Empty: `<root>/shared`.
    pub shared_folder: String,
}

/// Whose folder a note goes to.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Scope {
    Agent,
    Group,
    Shared,
}

impl Scope {
    pub fn parse(s: Option<&str>) -> Self {
        match s {
            Some("group") => Scope::Group,
            Some("shared") | Some("all") => Scope::Shared,
            _ => Scope::Agent,
        }
    }
}

/// The brain vault on this computer, if brain is installed.
pub fn vault_dir() -> Option<PathBuf> {
    let dir = crate::family::app_home("brain")?.join("vault");
    dir.is_dir().then_some(dir)
}

/// A file or folder name made of lowercase letters, digits and dashes.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.trim().to_lowercase().chars() {
        let c = match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out: String = out.trim_matches('-').chars().take(60).collect();
    if out.is_empty() {
        "note".into()
    } else {
        out.trim_end_matches('-').to_string()
    }
}

/// A folder the user typed, cleaned up: relative to the vault, no `..`, forward slashes.
pub fn clean_folder(raw: &str) -> Option<String> {
    let raw = raw.trim().trim_matches('/').replace('\\', "/");
    if raw.is_empty() {
        return None;
    }
    let path = Path::new(&raw);
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    Some(raw.trim_end_matches(".md").to_string())
}

/// A note's frontmatter and body as brain writes them. `created` survives rewrites.
fn note(path: &Path, fields: &[(&str, String)], body: &str) -> String {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let created = std::fs::read_to_string(path)
        .ok()
        .and_then(|old| {
            old.lines()
                .skip(1)
                .take_while(|l| *l != "---")
                .find_map(|l| l.strip_prefix("created: ").map(str::to_string))
        })
        .unwrap_or_else(|| today.clone());
    let mut s = String::from("---\n");
    for (k, v) in fields {
        if v.is_empty() {
            continue;
        }
        s.push_str(&format!("{k}: {v}\n"));
    }
    s.push_str(&format!("created: {created}\nupdated: {today}\n---\n"));
    s.push_str(body.trim_end());
    s.push('\n');
    s
}

/// Quote a frontmatter value when YAML would misread it.
fn yaml(v: &str) -> String {
    let v = v.replace('\n', " ");
    if v.contains(':')
        || v.contains('#')
        || v.starts_with(['[', '{', '"', '\'', '-', '*', '&', '!'])
    {
        format!("\"{}\"", v.replace('"', "'"))
    } else {
        v
    }
}

/// Write a note only when what it says changed (the `updated` date alone is not a change).
fn write_if_changed(path: &Path, text: &str) -> anyhow::Result<bool> {
    let strip = |s: &str| {
        s.lines()
            .filter(|l| !l.starts_with("updated: "))
            .collect::<Vec<_>>()
            .join("\n")
    };
    if let Ok(old) = std::fs::read_to_string(path) {
        if strip(&old) == strip(text) {
            return Ok(false);
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, text)?;
    Ok(true)
}

/// The `description:` of a note, for the folder index.
fn description_of(path: &Path) -> String {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| {
            t.lines().take(20).find_map(|l| {
                l.strip_prefix("description: ")
                    .map(|d| d.trim_matches('"').to_string())
            })
        })
        .unwrap_or_default()
}

/// What a folder holds, for brain_read and the index.
fn listing(dir: &Path) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            (name.ends_with(".md") && name != "index.md")
                .then(|| (name.clone(), description_of(&e.path())))
        })
        .collect();
    files.sort();
    files
}

impl Core {
    /// The vault, when the user turned the sync on and brain is installed.
    pub async fn brain_vault(&self) -> Option<PathBuf> {
        let on = self.store.setting(SYNC_SETTING).await.ok().flatten();
        if on.as_deref() != Some("true") {
            return None;
        }
        vault_dir()
    }

    async fn brain_root(&self) -> String {
        self.store
            .setting(ROOT_SETTING)
            .await
            .ok()
            .flatten()
            .and_then(|r| clean_folder(&r))
            .unwrap_or_else(|| DEFAULT_ROOT.into())
    }

    /// The agent's folder (relative to the vault), when the sync is on for it.
    pub async fn brain_agent_folder(&self, agent: &Agent) -> Option<String> {
        self.brain_vault().await?;
        if !agent.settings.brain_sync {
            return None;
        }
        if let Some(f) = agent
            .settings
            .brain_folder
            .as_deref()
            .and_then(clean_folder)
        {
            return Some(f);
        }
        // Chosen once and kept, so renaming the agent does not leave its notes behind.
        let folder = format!("{}/{}", self.brain_root().await, slug(&agent.name));
        let mut settings = agent.settings.clone();
        settings.brain_folder = Some(folder.clone());
        let _ = self
            .store
            .update_agent(
                &agent.id,
                AgentPatch {
                    settings: Some(settings),
                    ..Default::default()
                },
            )
            .await;
        Some(folder)
    }

    pub async fn brain_group_folder(&self, group: &Group) -> Option<String> {
        self.brain_vault().await?;
        if group.config.get("brain_sync").and_then(Value::as_bool) == Some(false) {
            return None;
        }
        if let Some(f) = group
            .config
            .get("brain_folder")
            .and_then(Value::as_str)
            .and_then(clean_folder)
        {
            return Some(f);
        }
        Some(format!(
            "{}/group-{}",
            self.brain_root().await,
            slug(&group.name)
        ))
    }

    pub async fn brain_shared_folder(&self) -> Option<String> {
        self.brain_vault().await?;
        if self
            .store
            .setting("brain_shared")
            .await
            .ok()
            .flatten()
            .as_deref()
            == Some("false")
        {
            return None;
        }
        let custom = self
            .store
            .setting("brain_shared_folder")
            .await
            .ok()
            .flatten()
            .and_then(|f| clean_folder(&f));
        Some(match custom {
            Some(f) => f,
            None => format!("{}/shared", self.brain_root().await),
        })
    }

    /// The project note above the folders, with a map of them (created once, refreshed when a
    /// folder is added).
    async fn brain_project_note(&self, vault: &Path) -> anyhow::Result<()> {
        let root = self.brain_root().await;
        let dir = vault.join(&root);
        let mut folders: Vec<(String, String)> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                (name.clone(), description_of(&e.path().join("index.md")))
            })
            .collect();
        folders.sort();
        let name = slug(root.rsplit('/').next().unwrap_or("opencue-agents"));
        let mut body = String::from(
            "# OpenCue agents\n\nWhat the OpenCue agents know and keep: one folder per agent, per group and one shared by all of them. \
             OpenCue writes it as they work; each folder starts with its `index.md`.\n\n## Mapa de notas\n\n",
        );
        for (f, d) in &folders {
            body.push_str(&format!(
                "- [{f}]({name}/{f}/index.md){}\n",
                if d.is_empty() {
                    String::new()
                } else {
                    format!(" — {d}")
                }
            ));
        }
        let path = vault.join(format!("{root}.md"));
        let text = note(
            &path,
            &[
                ("type", "Project".into()),
                ("name", name.clone()),
                ("title", "OpenCue agents".into()),
                ("description", yaml("Notebook of the OpenCue agents: one folder per agent, per group and one shared by all")),
                ("tags", "[opencue, agents]".into()),
            ],
            &body,
        );
        write_if_changed(&path, &text)?;
        Ok(())
    }

    /// Rewrite a folder's index: what it is and one line per note in it.
    fn brain_index(
        &self,
        vault: &Path,
        folder: &str,
        title: &str,
        what: &str,
    ) -> anyhow::Result<()> {
        let dir = vault.join(folder);
        std::fs::create_dir_all(&dir)?;
        let mut body = format!(
            "# {title}\n\n{what}\n\nRead this first, then open only the notes that matter for the task.\n\n## Notes\n\n"
        );
        let files = listing(&dir);
        if files.is_empty() {
            body.push_str("(none yet)\n");
        }
        for (f, d) in files {
            body.push_str(&format!(
                "- [{f}]({f}){}\n",
                if d.is_empty() {
                    String::new()
                } else {
                    format!(" — {d}")
                }
            ));
        }
        let parent = folder.rsplit('/').nth(1).map(slug).unwrap_or_default();
        let path = dir.join("index.md");
        let text = note(
            &path,
            &[
                ("type", "SubProject".into()),
                ("name", slug(&folder.replace('/', "-"))),
                ("title", yaml(title)),
                ("description", yaml(what)),
                ("tags", "[opencue, agents]".into()),
                (
                    "related",
                    if parent.is_empty() {
                        String::new()
                    } else {
                        format!("[{parent}]")
                    },
                ),
            ],
            &body,
        );
        write_if_changed(&path, &text)?;
        Ok(())
    }

    fn brain_file(
        &self,
        vault: &Path,
        folder: &str,
        file: &str,
        title: &str,
        description: &str,
        body: &str,
    ) -> anyhow::Result<bool> {
        let path = vault.join(folder).join(file);
        let index = slug(&folder.replace('/', "-"));
        let text = note(
            &path,
            &[
                ("type", "File".into()),
                (
                    "name",
                    format!("{index}-{}", slug(file.trim_end_matches(".md"))),
                ),
                ("title", yaml(title)),
                ("description", yaml(description)),
                ("tags", "[opencue]".into()),
                ("related", format!("[{index}]")),
            ],
            body,
        );
        write_if_changed(&path, &text)
    }

    /// Copy an agent's profile and memory to its folder. Cheap when nothing changed.
    pub async fn brain_sync_agent(&self, agent_id: &str) -> anyhow::Result<()> {
        let Some(vault) = self.brain_vault().await else {
            return Ok(());
        };
        let agent = self.store.get_agent(agent_id).await?;
        let Some(folder) = self.brain_agent_folder(&agent).await else {
            return Ok(());
        };
        let instructions = self.store.instructions(&agent.id);
        let soul = self.store.soul(&agent.id);
        let mut profile = format!(
            "# {} — profile\n\n## Instructions (its job)\n\n",
            agent.name
        );
        profile.push_str(if instructions.trim().is_empty() {
            "(not written yet)"
        } else {
            instructions.trim()
        });
        profile.push_str("\n\n## Soul (who it is)\n\n");
        profile.push_str(if soul.trim().is_empty() {
            "(not written yet)"
        } else {
            soul.trim()
        });
        let mut changed = self.brain_file(
            &vault,
            &folder,
            "profile.md",
            &format!("{} — profile", agent.name),
            &format!(
                "{}'s job and personality, as OpenCue keeps them",
                agent.name
            ),
            &profile,
        )?;
        let mem = self.store.agent_memory(&agent.id).entries();
        let mut memory = format!("# {} — memory\n\n", agent.name);
        if mem.is_empty() {
            memory.push_str("(nothing yet)\n");
        }
        for e in mem {
            memory.push_str(&format!("- {}\n", e.text));
        }
        changed |= self.brain_file(
            &vault,
            &folder,
            "memory.md",
            &format!("{} — memory", agent.name),
            &format!(
                "What {} remembers about the user and their work",
                agent.name
            ),
            &memory,
        )?;
        if changed || !vault.join(&folder).join("index.md").exists() {
            self.brain_index(
                &vault,
                &folder,
                &format!("{} (OpenCue agent)", agent.name),
                &format!(
                    "{}'s folder: its profile, memory and the insights it saved",
                    agent.name
                ),
            )?;
            self.brain_project_note(&vault).await?;
        }
        Ok(())
    }

    /// Copy the shared memory to the shared folder.
    pub async fn brain_sync_shared(&self) -> anyhow::Result<()> {
        let (Some(vault), Some(folder)) =
            (self.brain_vault().await, self.brain_shared_folder().await)
        else {
            return Ok(());
        };
        let mut memory = String::from("# Shared memory\n\nWhat every OpenCue agent knows.\n\n");
        let mem = self.store.global_memory().entries();
        if mem.is_empty() {
            memory.push_str("(nothing yet)\n");
        }
        for e in mem {
            memory.push_str(&format!("- {}\n", e.text));
        }
        let changed = self.brain_file(
            &vault,
            &folder,
            "memory.md",
            "Shared memory",
            "What every OpenCue agent knows about the user",
            &memory,
        )?;
        if changed || !vault.join(&folder).join("index.md").exists() {
            self.brain_index(
                &vault,
                &folder,
                "Shared by all agents",
                "Notes and memory shared by every OpenCue agent",
            )?;
            self.brain_project_note(&vault).await?;
        }
        Ok(())
    }

    /// A memory changed (an agent's, or the shared one when `agent_id` is None): tell the app and
    /// copy it to brain.
    pub async fn memory_changed(&self, agent_id: Option<String>) {
        self.emit(crate::CoreEvent::MemoryChanged {
            agent_id: agent_id.clone(),
        });
        let r = match agent_id {
            Some(id) => self.brain_sync_agent(&id).await,
            None => self.brain_sync_shared().await,
        };
        if let Err(e) = r {
            tracing::debug!("brain sync failed: {e:#}");
        }
    }

    /// An agent's instructions or soul changed: tell the app and copy them to brain.
    pub async fn profile_changed(&self, agent_id: &str) {
        self.emit(crate::CoreEvent::ProfileChanged {
            agent_id: agent_id.to_string(),
        });
        if let Err(e) = self.brain_sync_agent(agent_id).await {
            tracing::debug!("brain sync failed: {e:#}");
        }
    }

    /// The folders an agent may save to, for its prompt.
    pub async fn brain_folders(
        &self,
        agent: &Agent,
        group: Option<&Group>,
    ) -> Vec<(Scope, String)> {
        let mut out = vec![];
        if let Some(f) = self.brain_agent_folder(agent).await {
            out.push((Scope::Agent, f));
        }
        if let Some(g) = group {
            if let Some(f) = self.brain_group_folder(g).await {
                out.push((Scope::Group, f));
            }
        }
        if let Some(f) = self.brain_shared_folder().await {
            out.push((Scope::Shared, f));
        }
        out
    }

    /// Bring every folder up to date (when the sync is turned on, or on demand).
    pub async fn brain_sync_all(&self) -> anyhow::Result<usize> {
        let Some(vault) = self.brain_vault().await else {
            anyhow::bail!("brain is not installed on this computer, or the sync is off");
        };
        let mut n = 0;
        for a in self.store.list_agents().await? {
            if a.settings.brain_sync {
                self.brain_sync_agent(&a.id).await?;
                n += 1;
            }
        }
        for g in self.store.list_groups().await? {
            if let Some(f) = self.brain_group_folder(&g).await {
                self.brain_index(
                    &vault,
                    &f,
                    &format!("{} (OpenCue group)", g.name),
                    "What the group decided and learned",
                )?;
                n += 1;
            }
        }
        self.brain_sync_shared().await?;
        self.brain_project_note(&vault).await?;
        Ok(n)
    }

    /// For Settings: whether brain is here, whether the sync is on and where things go.
    pub async fn brain_info(&self) -> BrainInfo {
        let vault = vault_dir();
        BrainInfo {
            installed: vault.is_some(),
            vault: vault.map(|v| v.display().to_string()),
            enabled: self
                .store
                .setting(SYNC_SETTING)
                .await
                .ok()
                .flatten()
                .as_deref()
                == Some("true"),
            root: self.brain_root().await,
            shared: self
                .store
                .setting("brain_shared")
                .await
                .ok()
                .flatten()
                .as_deref()
                != Some("false"),
            shared_folder: self
                .store
                .setting("brain_shared_folder")
                .await
                .ok()
                .flatten()
                .and_then(|f| clean_folder(&f))
                .unwrap_or_default(),
        }
    }

    /// What the agent is told about its folders.
    pub(crate) async fn brain_prompt(&self, agent: &Agent, group: Option<&Group>) -> String {
        let folders = self.brain_folders(agent, group).await;
        if folders.is_empty() {
            return String::new();
        }
        let mut s = String::from(
            "\n\n## brain (the user's second brain)\nbrain is the user's notebook, shared by their AI tools. Your folders in it:\n",
        );
        for (scope, f) in &folders {
            let who = match scope {
                Scope::Agent => "yours",
                Scope::Group => "this group's",
                Scope::Shared => "shared by every agent",
            };
            s.push_str(&format!("- {f} ({who})\n"));
        }
        s.push_str(
            "Each folder starts with an index.md listing its notes. When a task needs context about the user, their work, projects or past decisions, \
             call `brain_read` (the index first, then only the notes that matter) instead of guessing. When you reach a conclusion, a decision or something \
             worth knowing next time, save it with `brain_save`: insights, not raw data, one note per topic (save the same title again to update it). \
             Meeting notes and decisions are the most valuable things to keep. Your profile and memory are copied there by OpenCue.",
        );
        s
    }

    /// `brain_save`: one note per topic in the agent's, its group's or the shared folder.
    pub(crate) async fn brain_save_tool(
        &self,
        agent: &Agent,
        group: Option<&Group>,
        args: &Value,
    ) -> anyhow::Result<String> {
        let vault = self
            .brain_vault()
            .await
            .ok_or_else(|| anyhow::anyhow!("brain is not connected"))?;
        let title = crate::tools::arg_str(args, "title")
            .ok_or_else(|| anyhow::anyhow!("`title` is required"))?;
        let content = crate::tools::arg_str(args, "content")
            .ok_or_else(|| anyhow::anyhow!("`content` is required"))?;
        anyhow::ensure!(
            content.chars().count() <= 40_000,
            "too long: save the insight, not the raw data"
        );
        let scope = Scope::parse(crate::tools::arg_str(args, "scope"));
        let folders = self.brain_folders(agent, group).await;
        let folder = folders
            .iter()
            .find(|(s, _)| *s == scope)
            .map(|(_, f)| f.clone())
            .ok_or_else(|| {
                anyhow::anyhow!("there is no {scope:?} folder in brain for this chat")
            })?;
        let file = format!("{}.md", slug(title));
        anyhow::ensure!(
            !["index.md", "profile.md", "memory.md"].contains(&file.as_str()),
            "that name is kept by OpenCue; pick another title"
        );
        let description = crate::tools::arg_str(args, "description")
            .map(str::to_string)
            .unwrap_or_else(|| {
                content
                    .lines()
                    .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
                    .unwrap_or(title)
                    .chars()
                    .take(140)
                    .collect()
            });
        let body = if content.trim_start().starts_with('#') {
            content.to_string()
        } else {
            format!("# {title}\n\n{content}")
        };
        self.brain_file(&vault, &folder, &file, title, &description, &body)?;
        let (index_title, what) = match scope {
            Scope::Agent => (
                format!("{} (OpenCue agent)", agent.name),
                format!(
                    "{}'s folder: its profile, memory and the insights it saved",
                    agent.name
                ),
            ),
            Scope::Group => (
                format!(
                    "{} (OpenCue group)",
                    group.map(|g| g.name.as_str()).unwrap_or("Group")
                ),
                "What the group decided and learned".to_string(),
            ),
            Scope::Shared => (
                "Shared by all agents".to_string(),
                "Notes and memory shared by every OpenCue agent".to_string(),
            ),
        };
        self.brain_index(&vault, &folder, &index_title, &what)?;
        self.brain_project_note(&vault).await?;
        Ok(format!("Saved to brain: {folder}/{file}"))
    }

    /// `brain_read`: a folder's index (by default the agent's), or one note of the vault.
    pub(crate) async fn brain_read_tool(
        &self,
        agent: &Agent,
        group: Option<&Group>,
        args: &Value,
    ) -> anyhow::Result<String> {
        let vault = self
            .brain_vault()
            .await
            .ok_or_else(|| anyhow::anyhow!("brain is not connected"))?;
        let folders = self.brain_folders(agent, group).await;
        let Some(raw) = crate::tools::arg_str(args, "path").filter(|p| !p.trim().is_empty()) else {
            let mut out =
                String::from("Your folders in brain (read a note with brain_read and its path):\n");
            for (scope, f) in &folders {
                out.push_str(&format!("\n## {scope:?}: {f}\n"));
                let files = listing(&vault.join(f));
                if files.is_empty() {
                    out.push_str("(empty)\n");
                }
                for (name, d) in files {
                    out.push_str(&format!(
                        "- {f}/{name}{}\n",
                        if d.is_empty() {
                            String::new()
                        } else {
                            format!(" — {d}")
                        }
                    ));
                }
            }
            return Ok(out);
        };
        let rel = clean_folder(raw).ok_or_else(|| {
            anyhow::anyhow!("give a path inside the vault, like projects/x/notes.md")
        })?;
        let mut path = vault.join(&rel);
        if path.is_dir() {
            path = path.join("index.md");
        } else if path.extension().is_none() {
            path.set_extension("md");
        }
        let text =
            std::fs::read_to_string(&path).map_err(|_| anyhow::anyhow!("no note at {rel}"))?;
        Ok(if text.chars().count() > READ_LIMIT {
            format!(
                "{}\n…[cut]",
                text.chars().take(READ_LIMIT).collect::<String>()
            )
        } else {
            text
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_and_folders() {
        assert_eq!(slug("María José — Ventas"), "maria-jose-ventas");
        assert_eq!(slug("  !!  "), "note");
        assert_eq!(clean_folder("/projects/x/"), Some("projects/x".into()));
        assert_eq!(clean_folder("../etc"), None);
        assert_eq!(clean_folder("projects/../../x"), None);
        assert_eq!(clean_folder(""), None);
    }

    #[tokio::test]
    async fn folders_get_an_index_and_the_project_a_map() {
        let data = tempfile::tempdir().unwrap();
        let vault = tempfile::tempdir().unwrap();
        let core = Core::open(crate::paths::Paths::at(data.path()).unwrap())
            .await
            .unwrap();
        let folder = format!("{DEFAULT_ROOT}/ana");
        assert!(core
            .brain_file(
                vault.path(),
                &folder,
                "memory.md",
                "Ana — memory",
                "What Ana remembers",
                "# Ana\n\n- likes tea"
            )
            .unwrap());
        assert!(!core
            .brain_file(
                vault.path(),
                &folder,
                "memory.md",
                "Ana — memory",
                "What Ana remembers",
                "# Ana\n\n- likes tea"
            )
            .unwrap());
        core.brain_index(vault.path(), &folder, "Ana (OpenCue agent)", "Ana's folder")
            .unwrap();
        core.brain_project_note(vault.path()).await.unwrap();
        let index = std::fs::read_to_string(vault.path().join(&folder).join("index.md")).unwrap();
        assert!(
            index.contains("type: SubProject")
                && index.contains("[memory.md](memory.md) — What Ana remembers")
        );
        let project =
            std::fs::read_to_string(vault.path().join(format!("{DEFAULT_ROOT}.md"))).unwrap();
        assert!(
            project.contains("type: Project") && project.contains("(opencue-agents/ana/index.md)")
        );
        let mem = std::fs::read_to_string(vault.path().join(&folder).join("memory.md")).unwrap();
        assert!(mem.contains("related: [projects-opencue-agents-ana]"));
    }

    #[test]
    fn notes_are_rewritten_only_when_they_change() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a/n.md");
        let t = note(
            &p,
            &[("type", "File".into()), ("title", yaml("A: b"))],
            "# Hi",
        );
        assert!(t.contains("title: \"A: b\""));
        assert!(write_if_changed(&p, &t).unwrap());
        assert!(!write_if_changed(&p, &t.replace("updated: ", "updated: 1999-")).unwrap());
        let t2 = note(&p, &[("type", "File".into())], "# Changed");
        assert!(write_if_changed(&p, &t2).unwrap());
        assert_eq!(listing(&dir.path().join("a")).len(), 1);
    }
}
