//! Skills the user's coding CLIs already have installed (Claude Code, Codex, Grok, Vibe…), found
//! on disk so they can be used from OpenCue's chat with `/name` without copying anything. They are
//! only read: OpenCue's own skills (and their names) always win.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::skills::{parse_frontmatter, Skill};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CliSkill {
    pub name: String,
    pub description: String,
    pub path: String,
    /// `claude`, `codex`, `agents`, `grok`, `vibe` or `gemini`.
    pub source: String,
}

fn home() -> Option<PathBuf> {
    directories::UserDirs::new().map(|d| d.home_dir().to_path_buf())
}

/// Where each CLI keeps its skills (folders with a `SKILL.md`) and its commands (single `.md` files).
fn roots() -> Vec<(&'static str, Vec<PathBuf>, Vec<PathBuf>)> {
    let Some(h) = home() else { return vec![] };
    let codex = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| h.join(".codex"));
    let claude = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| h.join(".claude"));
    vec![
        (
            "claude",
            vec![claude.join("skills")],
            vec![claude.join("commands")],
        ),
        (
            "codex",
            vec![codex.join("skills"), codex.join("skills").join(".system")],
            vec![codex.join("prompts")],
        ),
        ("agents", vec![h.join(".agents").join("skills")], vec![]),
        ("grok", vec![h.join(".grok").join("skills")], vec![]),
        ("vibe", vec![h.join(".vibe").join("skills")], vec![]),
        ("gemini", vec![h.join(".gemini").join("skills")], vec![]),
    ]
}

/// `skills/*/SKILL.md` folders found anywhere under a plugin folder (a few levels deep).
fn plugin_skill_dirs(root: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 7 || out.len() > 300 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        if e.file_name() == "skills" {
            out.push(p);
        } else if e.file_name() != "node_modules" && e.file_name() != ".git" {
            plugin_skill_dirs(&p, depth + 1, out);
        }
    }
}

fn read_one(file: &Path, fallback: &str, source: &str) -> Option<(CliSkill, String)> {
    let text = std::fs::read_to_string(file).ok()?;
    let (meta, body) = parse_frontmatter(&text);
    let name = meta
        .get("name")
        .map(|n| crate::skills::slug(n))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| crate::skills::slug(fallback));
    if name.is_empty() {
        return None;
    }
    let description = meta.get("description").cloned().unwrap_or_else(|| {
        body.lines()
            .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .unwrap_or_default()
            .chars()
            .take(140)
            .collect()
    });
    Some((
        CliSkill {
            name,
            description,
            path: file.display().to_string(),
            source: source.into(),
        },
        body,
    ))
}

/// Everything found, each name once (the first CLI that has it).
fn scan_all() -> Vec<(CliSkill, String)> {
    let mut out: Vec<(CliSkill, String)> = vec![];
    let mut seen = std::collections::HashSet::new();
    let mut add = |item: Option<(CliSkill, String)>| {
        if let Some(i) = item {
            if seen.insert(i.0.name.clone()) {
                out.push(i);
            }
        }
    };
    for (source, skill_dirs, command_dirs) in roots() {
        let mut dirs = skill_dirs;
        if source == "claude" {
            if let Some(h) = home() {
                let plugins = std::env::var_os("CLAUDE_CONFIG_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| h.join(".claude"))
                    .join("plugins");
                plugin_skill_dirs(&plugins, 0, &mut dirs);
            }
        }
        for dir in dirs {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut names: Vec<_> = entries.flatten().collect();
            names.sort_by_key(|e| e.file_name());
            for e in names {
                let file = e.path().join("SKILL.md");
                add(read_one(&file, &e.file_name().to_string_lossy(), source));
            }
        }
        for dir in command_dirs {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut files: Vec<_> = entries.flatten().collect();
            files.sort_by_key(|e| e.file_name());
            for e in files {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "md") {
                    let stem = p
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    add(read_one(&p, &stem, source));
                }
            }
        }
    }
    out
}

/// The skills the installed CLIs have, for the composer's menus.
pub fn list() -> Vec<CliSkill> {
    scan_all().into_iter().map(|(s, _)| s).collect()
}

/// One skill by name, with its instructions.
pub fn get(name: &str) -> Option<Skill> {
    let want = crate::skills::slug(name);
    scan_all()
        .into_iter()
        .find(|(s, _)| s.name == want)
        .map(|(s, body)| Skill {
            name: s.name,
            description: s.description,
            path: s.path,
            body,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_skill_and_a_command_file() {
        let dir = tempfile::tempdir().unwrap();
        let skill = dir.path().join("pdf-tools");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(
            skill.join("SKILL.md"),
            "---\nname: PDF Tools\ndescription: Work with PDFs\n---\nStep one.\n",
        )
        .unwrap();
        let (s, body) = read_one(&skill.join("SKILL.md"), "pdf-tools", "claude").unwrap();
        assert_eq!(
            (s.name.as_str(), s.description.as_str()),
            ("pdf-tools", "Work with PDFs")
        );
        assert_eq!(body, "Step one.");
        let cmd = dir.path().join("review.md");
        std::fs::write(&cmd, "Review the diff carefully.\nThen summarize.\n").unwrap();
        let (c, _) = read_one(&cmd, "review", "claude").unwrap();
        assert_eq!(c.name, "review");
        assert_eq!(c.description, "Review the diff carefully.");
    }
}
