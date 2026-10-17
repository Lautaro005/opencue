//! Memory is a Markdown bullet list per agent (`agents/<id>/MEMORY.md`) plus a global one.
//! Plain files keep it human-editable; each bullet is one fact.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MemoryEntry {
    /// 1-based position, stable until the file is edited.
    pub id: usize,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct MemoryFile {
    path: PathBuf,
}

const HEADER: &str = "# Memory\n\n";

impl MemoryFile {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn raw(&self) -> String {
        std::fs::read_to_string(&self.path).unwrap_or_default()
    }

    pub fn set_raw(&self, text: &str) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, text)?;
        Ok(())
    }

    pub fn entries(&self) -> Vec<MemoryEntry> {
        self.raw()
            .lines()
            .filter_map(|l| {
                l.trim_start()
                    .strip_prefix("- ")
                    .or_else(|| l.trim_start().strip_prefix("* "))
            })
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .enumerate()
            .map(|(i, t)| MemoryEntry {
                id: i + 1,
                text: t.to_string(),
            })
            .collect()
    }

    pub fn add(&self, text: &str) -> anyhow::Result<MemoryEntry> {
        let text = text.replace('\n', " ").trim().to_string();
        anyhow::ensure!(!text.is_empty(), "memory text is empty");
        let existing = self.entries();
        if let Some(found) = existing.iter().find(|e| e.text.eq_ignore_ascii_case(&text)) {
            return Ok(found.clone());
        }
        let mut raw = self.raw();
        if raw.trim().is_empty() {
            raw = HEADER.to_string();
        }
        if !raw.ends_with('\n') {
            raw.push('\n');
        }
        raw.push_str(&format!("- {text}\n"));
        self.set_raw(&raw)?;
        Ok(MemoryEntry {
            id: existing.len() + 1,
            text,
        })
    }

    /// Remove by 1-based id. Returns the removed text.
    pub fn remove(&self, id: usize) -> anyhow::Result<String> {
        let raw = self.raw();
        let mut seen = 0;
        let mut removed = None;
        let kept: Vec<&str> = raw
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                let is_entry = (t.starts_with("- ") || t.starts_with("* "))
                    && t.len() > 2
                    && !t[2..].trim().is_empty();
                if is_entry {
                    seen += 1;
                    if seen == id {
                        removed = Some(t[2..].trim().to_string());
                        return false;
                    }
                }
                true
            })
            .collect();
        let removed = removed.ok_or_else(|| anyhow::anyhow!("no memory with id {id}"))?;
        self.set_raw(&(kept.join("\n") + "\n"))?;
        Ok(removed)
    }

    pub fn search(&self, query: &str) -> Vec<MemoryEntry> {
        let words: Vec<String> = query
            .to_lowercase()
            .split_whitespace()
            .map(str::to_string)
            .collect();
        if words.is_empty() {
            return self.entries();
        }
        self.entries()
            .into_iter()
            .filter(|e| {
                let t = e.text.to_lowercase();
                words.iter().any(|w| t.contains(w))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_list_remove() {
        let dir = tempfile::tempdir().unwrap();
        let m = MemoryFile::new(dir.path().join("MEMORY.md"));
        m.add("Likes window seats").unwrap();
        m.add("Lives in Buenos Aires").unwrap();
        m.add("likes window seats").unwrap(); // dedupe
        assert_eq!(m.entries().len(), 2);
        assert_eq!(m.search("buenos").len(), 1);
        assert_eq!(m.remove(1).unwrap(), "Likes window seats");
        assert_eq!(
            m.entries(),
            vec![MemoryEntry {
                id: 1,
                text: "Lives in Buenos Aires".into()
            }]
        );
    }
}
