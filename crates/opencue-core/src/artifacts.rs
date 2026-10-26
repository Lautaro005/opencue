//! Artifacts: standalone things an agent makes for the user (a page, a diagram, a document, a
//! spreadsheet, a deck…). Each one is a file that shows up in the chat as a card, opens in a side
//! panel and is kept in the agent's Files, one file per version. The text the agent sent (the
//! "source") is kept next to the versions so the panel can preview even Word, Excel, PowerPoint
//! and PDF files without opening them.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::db::{new_id, now};
use crate::files::{clean_name, mime_for};
use crate::models::*;
use crate::tools::arg_str;
use crate::{office, Core};

/// What an artifact can be.
pub const KINDS: [&str; 12] = [
    "html", "svg", "markdown", "mermaid", "code", "text", "json", "csv", "xlsx", "docx", "pptx",
    "pdf",
];

/// Largest source an agent may send.
const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactVersion {
    pub version: u32,
    pub attachment: Attachment,
    pub created_at: String,
    /// The text the agent sent for this version.
    pub source: String,
    /// Who wrote this version: `"user"` when it was edited by hand in the canvas, otherwise the agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub thread_id: String,
    pub agent_id: String,
    pub title: String,
    pub kind: String,
    pub language: Option<String>,
    /// A canvas: the user can edit it in the side panel, and the agent reads their edits back.
    #[serde(default)]
    pub canvas: bool,
    pub versions: Vec<ArtifactVersion>,
}

/// File extension for an artifact.
pub fn extension(kind: &str, language: Option<&str>) -> &'static str {
    match kind {
        "html" => "html",
        "svg" => "svg",
        "markdown" => "md",
        "mermaid" => "mmd",
        "json" => "json",
        "csv" => "csv",
        "xlsx" => "xlsx",
        "docx" => "docx",
        "pptx" => "pptx",
        "pdf" => "pdf",
        "code" => match language
            .unwrap_or_default()
            .to_ascii_lowercase()
            .trim_start_matches('.')
        {
            "python" | "py" => "py",
            "javascript" | "js" | "node" => "js",
            "typescript" | "ts" => "ts",
            "tsx" => "tsx",
            "jsx" | "react" => "jsx",
            "rust" | "rs" => "rs",
            "go" | "golang" => "go",
            "java" => "java",
            "c" => "c",
            "cpp" | "c++" => "cpp",
            "csharp" | "c#" | "cs" => "cs",
            "ruby" | "rb" => "rb",
            "php" => "php",
            "swift" => "swift",
            "kotlin" | "kt" => "kt",
            "bash" | "sh" | "shell" | "zsh" => "sh",
            "sql" => "sql",
            "yaml" | "yml" => "yaml",
            "toml" => "toml",
            "css" => "css",
            "xml" => "xml",
            "lua" => "lua",
            "r" => "r",
            _ => "txt",
        },
        _ => "txt",
    }
}

fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.trim().chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out: String = out.trim_matches('-').chars().take(60).collect();
    if out.is_empty() {
        "artifact".into()
    } else {
        out
    }
}

/// Turn what the agent sent into the bytes of the file.
pub fn render(kind: &str, title: &str, content: &str) -> anyhow::Result<Vec<u8>> {
    Ok(match kind {
        "docx" => office::docx(content, title)?,
        "pdf" => office::pdf(content, title)?,
        "xlsx" => office::xlsx(&office::parse_sheets(content)?)?,
        "pptx" => {
            let slides = office::parse_slides(content);
            anyhow::ensure!(
                !slides.is_empty(),
                "a presentation needs slides: separate them with a line of ---"
            );
            office::pptx(&slides, title)?
        }
        "json" => {
            serde_json::from_str::<Value>(content)
                .map_err(|e| anyhow::anyhow!("the content is not valid JSON: {e}"))?;
            content.as_bytes().to_vec()
        }
        _ => content.as_bytes().to_vec(),
    })
}

impl Core {
    fn artifacts_dir(&self) -> PathBuf {
        self.paths().root().join("artifacts")
    }

    fn artifact_file(&self, id: &str) -> anyhow::Result<PathBuf> {
        anyhow::ensure!(
            !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "invalid artifact id"
        );
        Ok(self.artifacts_dir().join(format!("{id}.json")))
    }

    pub fn get_artifact(&self, id: &str) -> anyhow::Result<Artifact> {
        let text = std::fs::read_to_string(self.artifact_file(id)?)
            .map_err(|_| anyhow::anyhow!("artifact not found"))?;
        Ok(serde_json::from_str(&text)?)
    }

    fn save_artifact(&self, a: &Artifact) -> anyhow::Result<()> {
        std::fs::create_dir_all(self.artifacts_dir())?;
        std::fs::write(self.artifact_file(&a.id)?, serde_json::to_string(a)?)?;
        Ok(())
    }

    /// Every artifact one agent made, newest first.
    pub fn list_artifacts(&self, agent_id: &str) -> Vec<Artifact> {
        let mut out: Vec<Artifact> = std::fs::read_dir(self.artifacts_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| std::fs::read_to_string(e.path()).ok())
            .filter_map(|t| serde_json::from_str::<Artifact>(&t).ok())
            .filter(|a| a.agent_id == agent_id)
            .collect();
        out.sort_by(|a, b| {
            let last = |x: &Artifact| x.versions.last().map(|v| v.created_at.clone());
            last(b).cmp(&last(a))
        });
        out
    }

    /// `create_artifact`: make a new artifact, or a new version of one the agent made before.
    pub(crate) async fn artifact_tool(
        &self,
        thread_id: &str,
        agent: &Agent,
        args: &Value,
    ) -> anyhow::Result<String> {
        let content = arg_str(args, "content")
            .ok_or_else(|| anyhow::anyhow!("`content` is required"))?
            .to_string();
        anyhow::ensure!(
            content.len() <= MAX_SOURCE_BYTES,
            "the content is too large (limit 2 MB)"
        );
        let asked_kind = arg_str(args, "type").map(|t| t.trim().to_ascii_lowercase());
        let language = arg_str(args, "language")
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string);

        let mut artifact = match arg_str(args, "artifact_id").filter(|i| !i.trim().is_empty()) {
            Some(id) => {
                let a = self.get_artifact(id.trim())?;
                anyhow::ensure!(
                    a.agent_id == agent.id,
                    "that artifact belongs to another agent"
                );
                if let Some(k) = &asked_kind {
                    anyhow::ensure!(
                        *k == a.kind,
                        "an artifact keeps its type ({}): create a new one for a different type",
                        a.kind
                    );
                }
                a
            }
            None => {
                let kind = asked_kind.ok_or_else(|| anyhow::anyhow!("`type` is required"))?;
                anyhow::ensure!(
                    KINDS.contains(&kind.as_str()),
                    "unknown type `{kind}`: use one of {}",
                    KINDS.join(", ")
                );
                Artifact {
                    id: new_id(),
                    thread_id: thread_id.to_string(),
                    agent_id: agent.id.clone(),
                    title: String::new(),
                    kind,
                    language: None,
                    canvas: false,
                    versions: vec![],
                }
            }
        };
        if let Some(t) = arg_str(args, "title")
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            artifact.title = t.chars().take(120).collect();
        }
        anyhow::ensure!(!artifact.title.is_empty(), "`title` is required");
        if language.is_some() {
            artifact.language = language;
        }
        if args.get("canvas").and_then(Value::as_bool) == Some(true) {
            artifact.canvas = true;
        }

        let bytes = render(&artifact.kind, &artifact.title, &content)?;
        let version = artifact.versions.len() as u32 + 1;
        let ext = extension(&artifact.kind, artifact.language.as_deref());
        let name = clean_name(&format!(
            "{}{}.{ext}",
            slug(&artifact.title),
            if version > 1 {
                format!("-v{version}")
            } else {
                String::new()
            }
        ));
        let att = Attachment {
            id: new_id(),
            mime: mime_for(&name).to_string(),
            size: bytes.len() as u64,
            name,
            artifact: Some(ArtifactRef {
                id: artifact.id.clone(),
                version,
                kind: artifact.kind.clone(),
                title: artifact.title.clone(),
                canvas: artifact.canvas,
            }),
        };
        let path = self.paths().attachment_file(&att.id, &att.name);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, &bytes)?;

        artifact.versions.push(ArtifactVersion {
            version,
            attachment: att.clone(),
            created_at: now(),
            source: content,
            by: None,
        });
        self.save_artifact(&artifact)?;

        let mut m = crate::runtime::new_message(thread_id, Role::Assistant, Some(&agent.id), "");
        m.attachments = vec![att];
        self.store.insert_message(&m).await?;
        self.emit(crate::CoreEvent::Message { message: m });

        Ok(format!(
            "Shown to the user as the {} {} “{}” (version {version}) and saved in your Files. To change it later, call create_artifact again with artifact_id \"{}\" and the complete new content{}.",
            artifact.kind,
            if artifact.canvas { "canvas" } else { "artifact" },
            artifact.title,
            artifact.id,
            if artifact.canvas {
                " (call read_artifact first: the user may have edited it)"
            } else {
                ""
            }
        ))
    }

    /// `read_artifact`: the latest text of an artifact, including what the user edited by hand.
    pub(crate) fn read_artifact_tool(&self, agent: &Agent, args: &Value) -> anyhow::Result<String> {
        let id = arg_str(args, "artifact_id")
            .ok_or_else(|| anyhow::anyhow!("`artifact_id` is required"))?;
        let a = self.get_artifact(id.trim())?;
        anyhow::ensure!(
            a.agent_id == agent.id,
            "that artifact belongs to another agent"
        );
        let v = a
            .versions
            .last()
            .ok_or_else(|| anyhow::anyhow!("that artifact has no versions"))?;
        Ok(format!(
            "{} “{}” — version {}{}:\n\n{}",
            a.kind,
            a.title,
            v.version,
            if v.by.as_deref() == Some("user") {
                " (edited by the user)"
            } else {
                ""
            },
            v.source
        ))
    }

    /// The user edited a canvas by hand: keep it as a new version (no new card in the chat).
    pub fn save_canvas(&self, id: &str, content: &str) -> anyhow::Result<Artifact> {
        anyhow::ensure!(
            content.len() <= MAX_SOURCE_BYTES,
            "the content is too large (limit 2 MB)"
        );
        let mut artifact = self.get_artifact(id)?;
        anyhow::ensure!(artifact.canvas, "only a canvas can be edited by hand");
        if artifact
            .versions
            .last()
            .is_some_and(|v| v.source == content)
        {
            return Ok(artifact);
        }
        let bytes = render(&artifact.kind, &artifact.title, content)?;
        let version = artifact.versions.len() as u32 + 1;
        let ext = extension(&artifact.kind, artifact.language.as_deref());
        let name = clean_name(&format!("{}-v{version}.{ext}", slug(&artifact.title)));
        let att = Attachment {
            id: new_id(),
            mime: mime_for(&name).to_string(),
            size: bytes.len() as u64,
            name,
            artifact: Some(ArtifactRef {
                id: artifact.id.clone(),
                version,
                kind: artifact.kind.clone(),
                title: artifact.title.clone(),
                canvas: artifact.canvas,
            }),
        };
        let path = self.paths().attachment_file(&att.id, &att.name);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, &bytes)?;
        artifact.versions.push(ArtifactVersion {
            version,
            attachment: att,
            created_at: now(),
            source: content.to_string(),
            by: Some("user".into()),
        });
        self.save_artifact(&artifact)?;
        Ok(artifact)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_follow_the_type_and_language() {
        assert_eq!(extension("html", None), "html");
        assert_eq!(extension("code", Some("Python")), "py");
        assert_eq!(extension("code", Some("c++")), "cpp");
        assert_eq!(extension("code", Some("weird")), "txt");
        assert_eq!(extension("xlsx", None), "xlsx");
        assert!(KINDS.iter().all(|k| !extension(k, None).is_empty()));
    }

    #[test]
    fn slugs_are_file_safe() {
        assert_eq!(slug("Q3 Report: Sales / EU!"), "q3-report-sales-eu");
        assert_eq!(slug("  "), "artifact");
        assert_eq!(slug("Año nuevo"), "año-nuevo");
    }

    #[test]
    fn rendering_checks_the_content() {
        assert!(render("json", "t", "{oops").is_err());
        assert!(render("json", "t", "{\"a\":1}").is_ok());
        assert!(render("pptx", "t", "").is_err());
        assert_eq!(render("html", "t", "<p>x</p>").unwrap(), b"<p>x</p>");
        assert!(render("docx", "t", "# Hi").unwrap().starts_with(b"PK"));
        assert!(render("pdf", "t", "# Hi").unwrap().starts_with(b"%PDF"));
    }
}
