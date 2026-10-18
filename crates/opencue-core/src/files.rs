//! Files shared in chats and groups. The user attaches files to a message (they are stored in the
//! data folder and copied into each agent's workspace); agents send files back with `share_file`.

use std::path::Path;

use base64::Engine;

use crate::db::new_id;
use crate::models::*;
use crate::providers::ImageData;
use crate::Core;

/// Largest file the app accepts in a chat.
pub const MAX_FILE_BYTES: usize = 25 * 1024 * 1024;
/// Text files up to this size are shown to the model inline.
const INLINE_TEXT_BYTES: u64 = 40 * 1024;
/// Images larger than this are described instead of sent to the model.
const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;

/// Keep only a plain file name: no folders, no characters that break on Windows.
pub fn clean_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_control() || r#"<>:"|?*"#.contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() {
        "file".into()
    } else {
        cleaned.chars().take(120).collect()
    }
}

pub fn mime_for(name: &str) -> &'static str {
    let ext = name
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "txt" | "log" => "text/plain",
        "md" | "markdown" => "text/markdown",
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" | "ts" | "tsx" | "jsx" | "py" | "rs" | "go" | "java" | "c" | "h" | "cpp"
        | "sh" | "rb" | "php" | "sql" | "swift" | "kt" => "text/x-source",
        "json" => "application/json",
        "xml" => "application/xml",
        "yaml" | "yml" => "application/x-yaml",
        "toml" => "application/toml",
        "zip" => "application/zip",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}

pub fn human_size(bytes: u64) -> String {
    match bytes {
        b if b >= 1024 * 1024 => format!("{:.1} MB", b as f64 / 1024.0 / 1024.0),
        b if b >= 1024 => format!("{} KB", b / 1024),
        b => format!("{b} B"),
    }
}

impl Core {
    /// Store one file from the UI and copy it into the workspaces of the agents in the thread.
    pub(crate) async fn store_attachment(
        &self,
        thread_id: &str,
        name: &str,
        bytes: &[u8],
    ) -> anyhow::Result<Attachment> {
        anyhow::ensure!(
            bytes.len() <= MAX_FILE_BYTES,
            "{name} is larger than {}",
            human_size(MAX_FILE_BYTES as u64)
        );
        let name = clean_name(name);
        let att = Attachment {
            id: new_id(),
            mime: mime_for(&name).to_string(),
            size: bytes.len() as u64,
            name,
        };
        let path = self.paths().attachment_file(&att.id, &att.name);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, bytes)?;
        for agent_id in self.thread_agents(thread_id).await? {
            let Ok(agent) = self.store.get_agent(&agent_id).await else {
                continue;
            };
            let dir = self.workspace_for(&agent).join("files");
            if std::fs::create_dir_all(&dir).is_ok() {
                let _ = std::fs::copy(&path, dir.join(&att.name));
            }
        }
        Ok(att)
    }

    pub(crate) async fn decode_attachments(
        &self,
        thread_id: &str,
        files: &[NewAttachment],
    ) -> anyhow::Result<Vec<Attachment>> {
        let mut out = Vec::with_capacity(files.len());
        for f in files {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(f.data.trim())
                .map_err(|_| anyhow::anyhow!("{} could not be read", f.name))?;
            out.push(self.store_attachment(thread_id, &f.name, &bytes).await?);
        }
        Ok(out)
    }

    async fn thread_agents(&self, thread_id: &str) -> anyhow::Result<Vec<String>> {
        Ok(match self.store.get_group(thread_id).await? {
            Some(g) => g.members.into_iter().map(|m| m.agent_id).collect(),
            None => vec![thread_id.to_string()],
        })
    }

    /// Every file shared in a chat, newest first.
    pub async fn thread_files(&self, thread_id: &str) -> anyhow::Result<Vec<SharedFile>> {
        Ok(self
            .store
            .thread_files(thread_id)
            .await?
            .into_iter()
            .map(|(attachment, m)| SharedFile {
                attachment,
                message_id: m.id,
                agent_id: m.agent_id,
                created_at: m.created_at,
            })
            .collect())
    }

    pub async fn attachment(&self, id: &str) -> anyhow::Result<(Attachment, std::path::PathBuf)> {
        let a = self.store.find_attachment(id).await?;
        let path = self.paths().attachment_file(&a.id, &a.name);
        anyhow::ensure!(path.exists(), "{} is no longer on disk", a.name);
        Ok((a, path))
    }

    pub async fn read_attachment(&self, id: &str) -> anyhow::Result<(Attachment, Vec<u8>)> {
        let (a, path) = self.attachment(id).await?;
        Ok((a, std::fs::read(path)?))
    }

    /// How a message's files reach the model: a note per file, small text files inline, and
    /// pictures as images.
    pub(crate) fn describe_attachments(
        &self,
        atts: &[Attachment],
        with_images: bool,
    ) -> (String, Vec<ImageData>) {
        let mut text = String::new();
        let mut images = Vec::new();
        for a in atts {
            let path = self.paths().attachment_file(&a.id, &a.name);
            text.push_str(&format!(
                "\n\n[File: {} ({}, {}) — also saved in your workspace as files/{}]",
                a.name,
                a.mime,
                human_size(a.size),
                a.name
            ));
            if a.is_image() && with_images && a.size <= MAX_IMAGE_BYTES {
                if let Ok(bytes) = std::fs::read(&path) {
                    images.push(ImageData {
                        mime: a.mime.clone(),
                        data: base64::engine::general_purpose::STANDARD.encode(bytes),
                    });
                }
            } else if (a.is_text() || a.mime == "text/x-source") && a.size <= INLINE_TEXT_BYTES {
                if let Ok(body) = std::fs::read_to_string(&path) {
                    text.push_str(&format!("\n```\n{}\n```", body.trim_end()));
                }
            }
        }
        (text, images)
    }

    /// `share_file`: attach a file from the agent's workspace to a new message in the chat.
    pub(crate) async fn share_workspace_file(
        &self,
        thread_id: &str,
        agent: &Agent,
        rel: &str,
        caption: &str,
    ) -> anyhow::Result<Attachment> {
        let cfg = self.computer_config().await;
        let computer = self.computer_for(agent, &cfg);
        let bytes = if agent.settings.computer == crate::computer::ComputerBackend::Space {
            let cancel = tokio_util::sync::CancellationToken::new();
            computer.read(rel, &cancel).await?.into_bytes()
        } else {
            let path = computer.resolve(rel)?;
            anyhow::ensure!(path.is_file(), "{rel} is not a file in your workspace");
            std::fs::read(&path)?
        };
        let name = Path::new(rel.trim())
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".into());
        anyhow::ensure!(
            bytes.len() <= MAX_FILE_BYTES,
            "{name} is larger than {}",
            human_size(MAX_FILE_BYTES as u64)
        );
        let name = clean_name(&name);
        let att = Attachment {
            id: new_id(),
            mime: mime_for(&name).to_string(),
            size: bytes.len() as u64,
            name,
        };
        let path = self.paths().attachment_file(&att.id, &att.name);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, &bytes)?;
        let mut m = crate::runtime::new_message(
            thread_id,
            Role::Assistant,
            Some(&agent.id),
            caption.trim(),
        );
        m.attachments = vec![att.clone()];
        self.store.insert_message(&m).await?;
        self.emit(crate::CoreEvent::Message { message: m });
        Ok(att)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SharedFile {
    pub attachment: Attachment,
    pub message_id: String,
    pub agent_id: Option<String>,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_plain_and_safe() {
        assert_eq!(clean_name("../../etc/passwd"), "passwd");
        assert_eq!(clean_name("C:\\Users\\me\\a:b.txt"), "a_b.txt");
        assert_eq!(clean_name(".."), "file");
        assert_eq!(mime_for("Photo.JPG"), "image/jpeg");
        assert_eq!(human_size(2048), "2 KB");
    }
}
