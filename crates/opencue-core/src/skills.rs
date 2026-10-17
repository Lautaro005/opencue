//! Agent Skills in the Claude Code format: `skills/<name>/SKILL.md` with `name` and `description`
//! in YAML frontmatter. Agents see the list in their prompt and load a skill's body on demand.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSummary {
    pub name: String,
    pub description: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub path: String,
    /// Markdown body without the frontmatter.
    pub body: String,
}

/// Split `---\nkey: value\n---\nbody`. Only the flat `key: value` subset of YAML is understood,
/// plus indented continuation lines and `>`/`|` block scalars, which is what SKILL.md files use.
pub fn parse_frontmatter(text: &str) -> (BTreeMap<String, String>, String) {
    let mut meta = BTreeMap::new();
    let trimmed = text.trim_start_matches('\u{feff}');
    let Some(rest) = trimmed
        .strip_prefix("---\n")
        .or_else(|| trimmed.strip_prefix("---\r\n"))
    else {
        return (meta, text.to_string());
    };
    let Some(end) = rest.find("\n---") else {
        return (meta, text.to_string());
    };
    let (head, tail) = rest.split_at(end);
    let body = tail
        .trim_start_matches("\n---")
        .trim_start_matches(['\r', '\n'])
        .trim_end()
        .to_string();
    let mut current: Option<String> = None;
    for line in head.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(key) = &current {
                let v = meta.entry(key.clone()).or_insert_with(String::new);
                if !v.is_empty() {
                    v.push(' ');
                }
                v.push_str(line.trim());
            }
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_string();
            let mut value = v.trim().to_string();
            if value == ">" || value == "|" || value == ">-" || value == "|-" {
                value.clear();
            }
            let value = value.trim_matches('"').trim_matches('\'').to_string();
            meta.insert(key.clone(), value);
            current = Some(key);
        }
    }
    (meta, body)
}

/// Lowercase, dashes, no path tricks.
pub fn slug(name: &str) -> String {
    let mut s = String::new();
    for c in name.trim().chars() {
        if c.is_alphanumeric() {
            s.extend(c.to_lowercase());
        } else if !s.ends_with('-') {
            s.push('-');
        }
    }
    s.trim_matches('-').chars().take(64).collect()
}

fn render(name: &str, description: &str, body: &str) -> String {
    let desc = description.replace('\n', " ");
    format!(
        "---\nname: {name}\ndescription: {desc}\n---\n\n{}\n",
        body.trim()
    )
}

pub struct Skills {
    dir: PathBuf,
}

impl Skills {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn file_for(&self, name: &str) -> anyhow::Result<PathBuf> {
        let s = slug(name);
        anyhow::ensure!(!s.is_empty(), "skill name is required");
        Ok(self.dir.join(s).join("SKILL.md"))
    }

    pub fn list(&self) -> Vec<SkillSummary> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return out;
        };
        for entry in entries.flatten() {
            let file = entry.path().join("SKILL.md");
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            let (meta, _) = parse_frontmatter(&text);
            let dir_name = entry.file_name().to_string_lossy().to_string();
            out.push(SkillSummary {
                name: meta.get("name").cloned().unwrap_or(dir_name),
                description: meta.get("description").cloned().unwrap_or_default(),
                path: file.display().to_string(),
            });
        }
        out.sort_by_key(|a| a.name.to_lowercase());
        out
    }

    pub fn get(&self, name: &str) -> Option<Skill> {
        let wanted = slug(name);
        let summary = self.list().into_iter().find(|s| slug(&s.name) == wanted)?;
        let text = std::fs::read_to_string(&summary.path).ok()?;
        let (_, body) = parse_frontmatter(&text);
        Some(Skill {
            name: summary.name,
            description: summary.description,
            path: summary.path,
            body,
        })
    }

    pub fn save(&self, name: &str, description: &str, body: &str) -> anyhow::Result<Skill> {
        let name = name.trim();
        anyhow::ensure!(
            !description.trim().is_empty(),
            "a skill needs a description saying when to use it"
        );
        let file = self.file_for(name)?;
        std::fs::create_dir_all(file.parent().expect("skill dir"))?;
        std::fs::write(&file, render(name, description.trim(), body))?;
        Ok(Skill {
            name: name.to_string(),
            description: description.trim().to_string(),
            path: file.display().to_string(),
            body: body.trim().to_string(),
        })
    }

    /// Save a full SKILL.md as downloaded or pasted.
    pub fn save_raw(&self, text: &str, fallback_name: &str) -> anyhow::Result<Skill> {
        let (meta, body) = parse_frontmatter(text);
        let name = meta
            .get("name")
            .filter(|n| !n.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| fallback_name.to_string());
        let description = meta.get("description").cloned().unwrap_or_default();
        anyhow::ensure!(
            !description.trim().is_empty(),
            "this file has no `description` in its frontmatter, so it is not a SKILL.md"
        );
        self.save(&name, &description, &body)
    }

    pub fn delete(&self, name: &str) -> anyhow::Result<()> {
        let file = self.file_for(name)?;
        let dir = file.parent().expect("skill dir").to_path_buf();
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }
}

/// GitHub page links point at HTML; fetch the raw file instead.
pub fn raw_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("https://github.com/") {
        if let Some((repo, path)) = rest.split_once("/blob/") {
            return format!("https://raw.githubusercontent.com/{repo}/{path}");
        }
        if let Some((repo, path)) = rest.split_once("/tree/") {
            return format!(
                "https://raw.githubusercontent.com/{repo}/{}/SKILL.md",
                path.trim_end_matches('/')
            );
        }
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_and_folded_description() {
        let (meta, body) = parse_frontmatter(
            "---\nname: pdf-tools\ndescription: >\n  Fill and merge PDFs.\n  Use for forms.\n---\n\n# Steps\nDo it.",
        );
        assert_eq!(meta["name"], "pdf-tools");
        assert_eq!(meta["description"], "Fill and merge PDFs. Use for forms.");
        assert_eq!(body, "# Steps\nDo it.");
    }

    #[test]
    fn saves_lists_and_deletes() {
        let tmp = tempfile::tempdir().unwrap();
        let skills = Skills::new(tmp.path().to_path_buf());
        skills
            .save("Trip Planner", "Plan trips step by step", "1. Ask dates")
            .unwrap();
        let list = skills.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Trip Planner");
        assert_eq!(skills.get("trip planner").unwrap().body, "1. Ask dates");
        skills.delete("Trip Planner").unwrap();
        assert!(skills.list().is_empty());
    }

    #[test]
    fn github_links_become_raw() {
        assert_eq!(
            raw_url("https://github.com/a/b/blob/main/skills/x/SKILL.md"),
            "https://raw.githubusercontent.com/a/b/main/skills/x/SKILL.md"
        );
    }
}
