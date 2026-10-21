//! SQLite + Markdown persistence for agents, groups, messages, providers, MCP servers and settings.

use sqlx::{Row, SqlitePool};

use crate::db::{new_id, now};
use crate::memory::MemoryFile;
use crate::models::*;
use crate::paths::Paths;
use crate::providers::{ProviderConfig, ProviderKind};

#[derive(Clone)]
pub struct Store {
    pub db: SqlitePool,
    pub paths: Paths,
}

fn agent_from_row(r: &sqlx::sqlite::SqliteRow) -> Agent {
    Agent {
        id: r.get("id"),
        name: r.get("name"),
        avatar: Avatar {
            face: r.get("avatar_face"),
            color: r.get("avatar_color"),
            shape: r.get("avatar_shape"),
        },
        provider_id: r.get("provider_id"),
        model: r.get("model"),
        settings: serde_json::from_str(r.get::<&str, _>("settings")).unwrap_or_default(),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }
}

fn message_from_row(r: &sqlx::sqlite::SqliteRow) -> Message {
    Message {
        id: r.get("id"),
        thread_id: r.get("thread_id"),
        role: Role::parse(r.get("role")),
        agent_id: r.get("agent_id"),
        content: r.get("content"),
        tool_calls: r
            .get::<Option<String>, _>("tool_calls")
            .and_then(|s| serde_json::from_str(&s).ok()),
        tool_call_id: r.get("tool_call_id"),
        tool_name: r.get("tool_name"),
        status: r.get("status"),
        input_tokens: r.get("input_tokens"),
        output_tokens: r.get("output_tokens"),
        created_at: r.get("created_at"),
        attachments: r
            .get::<Option<String>, _>("attachments")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
    }
}

fn provider_from_row(r: &sqlx::sqlite::SqliteRow) -> anyhow::Result<ProviderConfig> {
    Ok(ProviderConfig {
        id: r.get("id"),
        kind: serde_json::from_value(serde_json::Value::String(r.get("kind")))?,
        name: r
            .get::<String, _>("name")
            .replace("CheapestInference", "CheaperInference"),
        base_url: r.get("base_url"),
        default_model: r.get("default_model"),
        enabled: r.get::<i64, _>("enabled") != 0,
        has_key: false,
    })
}

fn mcp_from_row(r: &sqlx::sqlite::SqliteRow) -> McpServerConfig {
    McpServerConfig {
        id: r.get("id"),
        name: r.get("name"),
        transport: if r.get::<&str, _>("transport") == "http" {
            McpTransport::Http
        } else {
            McpTransport::Stdio
        },
        command: r.get("command"),
        args: serde_json::from_str(r.get::<&str, _>("args")).unwrap_or_default(),
        env: serde_json::from_str(r.get::<&str, _>("env")).unwrap_or_default(),
        url: r.get("url"),
        headers: serde_json::from_str(r.get::<&str, _>("headers")).unwrap_or_default(),
        enabled: r.get::<i64, _>("enabled") != 0,
    }
}

/// Default avatars for new agents: (shape, face, color), matching the desktop picker.
const AVATARS: &[(&str, &str, &str)] = &[
    ("square", "googly", "#8e8e93"),
    ("drop", "wink", "#f6ad2b"),
    ("star", "googly", "#f7c32f"),
    ("bubble", "curious", "#2f86eb"),
    ("heart", "dots", "#f04848"),
    ("bot", "smile", "#34b27b"),
    ("cloud", "happy", "#5ec2f2"),
    ("hexagon", "stern", "#ff7a45"),
    ("flower", "sleepy", "#a78bfa"),
    ("ghost", "surprised", "#b7713f"),
];

impl Store {
    // ---------- agents ----------

    pub async fn list_agents(&self) -> anyhow::Result<Vec<Agent>> {
        let rows = sqlx::query("SELECT * FROM agents ORDER BY created_at")
            .fetch_all(&self.db)
            .await?;
        Ok(rows.iter().map(agent_from_row).collect())
    }

    pub async fn get_agent(&self, id: &str) -> anyhow::Result<Agent> {
        let row = sqlx::query("SELECT * FROM agents WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("agent not found"))?;
        Ok(agent_from_row(&row))
    }

    pub async fn find_agent_by_name(&self, name: &str) -> anyhow::Result<Option<Agent>> {
        let row = sqlx::query("SELECT * FROM agents WHERE lower(name) = lower(?) LIMIT 1")
            .bind(name.trim().trim_start_matches('@'))
            .fetch_optional(&self.db)
            .await?;
        Ok(row.as_ref().map(agent_from_row))
    }

    pub async fn create_agent(&self, new: NewAgent) -> anyhow::Result<Agent> {
        let name = new.name.trim();
        anyhow::ensure!(!name.is_empty(), "agent name is required");
        let count = sqlx::query("SELECT COUNT(*) AS n FROM agents")
            .fetch_one(&self.db)
            .await?
            .get::<i64, _>("n") as usize;
        let avatar = new.avatar.unwrap_or_else(|| {
            let (shape, face, color) = AVATARS[count % AVATARS.len()];
            Avatar {
                face: face.into(),
                color: color.into(),
                shape: shape.into(),
            }
        });
        let provider_id = match new.provider_id {
            Some(p) => Some(p),
            None => self.default_provider_id().await?,
        };
        let id = new_id();
        let ts = now();
        sqlx::query(
            "INSERT INTO agents (id, name, avatar_face, avatar_color, avatar_shape, provider_id, model, settings, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(&avatar.face)
        .bind(&avatar.color)
        .bind(&avatar.shape)
        .bind(&provider_id)
        .bind(&new.model)
        .bind(serde_json::to_string(&new.settings.unwrap_or_default())?)
        .bind(&ts)
        .bind(&ts)
        .execute(&self.db)
        .await?;
        std::fs::create_dir_all(self.paths.agent_dir(&id))?;
        self.set_instructions(&id, new.instructions.as_deref().unwrap_or(""))?;
        self.get_agent(&id).await
    }

    pub async fn update_agent(&self, id: &str, patch: AgentPatch) -> anyhow::Result<Agent> {
        let mut agent = self.get_agent(id).await?;
        if let Some(name) = patch
            .name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
        {
            agent.name = name;
        }
        if let Some(a) = patch.avatar {
            agent.avatar = a;
        }
        if let Some(p) = patch.provider_id {
            agent.provider_id = p;
        }
        if let Some(m) = patch.model {
            agent.model = m.filter(|m| !m.is_empty());
        }
        if let Some(s) = patch.settings {
            agent.settings = s;
        }
        sqlx::query(
            "UPDATE agents SET name = ?, avatar_face = ?, avatar_color = ?, avatar_shape = ?, provider_id = ?, model = ?, settings = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&agent.name)
        .bind(&agent.avatar.face)
        .bind(&agent.avatar.color)
        .bind(&agent.avatar.shape)
        .bind(&agent.provider_id)
        .bind(&agent.model)
        .bind(serde_json::to_string(&agent.settings)?)
        .bind(now())
        .bind(id)
        .execute(&self.db)
        .await?;
        self.get_agent(id).await
    }

    pub async fn delete_agent(&self, id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM messages WHERE thread_id = ?")
            .bind(id)
            .execute(&self.db)
            .await?;
        sqlx::query("DELETE FROM agents WHERE id = ?")
            .bind(id)
            .execute(&self.db)
            .await?;
        let dir = self.paths.agent_dir(id);
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn instructions(&self, agent_id: &str) -> String {
        std::fs::read_to_string(self.paths.instructions_file(agent_id)).unwrap_or_default()
    }

    pub fn set_instructions(&self, agent_id: &str, text: &str) -> anyhow::Result<()> {
        std::fs::create_dir_all(self.paths.agent_dir(agent_id))?;
        std::fs::write(self.paths.instructions_file(agent_id), text)?;
        Ok(())
    }

    pub fn soul(&self, agent_id: &str) -> String {
        std::fs::read_to_string(self.paths.soul_file(agent_id)).unwrap_or_default()
    }

    pub fn set_soul(&self, agent_id: &str, text: &str) -> anyhow::Result<()> {
        std::fs::create_dir_all(self.paths.agent_dir(agent_id))?;
        std::fs::write(self.paths.soul_file(agent_id), text)?;
        Ok(())
    }

    pub fn plan(&self, agent_id: &str) -> String {
        std::fs::read_to_string(self.paths.plan_file(agent_id)).unwrap_or_default()
    }

    pub fn set_plan(&self, agent_id: &str, text: &str) -> anyhow::Result<()> {
        std::fs::create_dir_all(self.paths.agent_dir(agent_id))?;
        if text.trim().is_empty() {
            let _ = std::fs::remove_file(self.paths.plan_file(agent_id));
        } else {
            std::fs::write(self.paths.plan_file(agent_id), text)?;
        }
        Ok(())
    }

    pub fn agent_memory(&self, agent_id: &str) -> MemoryFile {
        MemoryFile::new(self.paths.agent_memory_file(agent_id))
    }

    pub fn global_memory(&self) -> MemoryFile {
        MemoryFile::new(self.paths.global_memory_file())
    }

    // ---------- groups ----------

    pub async fn list_groups(&self) -> anyhow::Result<Vec<Group>> {
        let rows = sqlx::query("SELECT * FROM groups ORDER BY created_at")
            .fetch_all(&self.db)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let id: String = r.get("id");
            out.push(Group {
                members: self.group_members(&id).await?,
                id,
                name: r.get("name"),
                max_auto_turns: r.get("max_auto_turns"),
                created_at: r.get("created_at"),
            });
        }
        Ok(out)
    }

    pub async fn get_group(&self, id: &str) -> anyhow::Result<Option<Group>> {
        let Some(r) = sqlx::query("SELECT * FROM groups WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.db)
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(Group {
            id: id.to_string(),
            name: r.get("name"),
            max_auto_turns: r.get("max_auto_turns"),
            members: self.group_members(id).await?,
            created_at: r.get("created_at"),
        }))
    }

    async fn group_members(&self, group_id: &str) -> anyhow::Result<Vec<GroupMember>> {
        let rows = sqlx::query(
            "SELECT agent_id, is_owner FROM group_members WHERE group_id = ? ORDER BY position",
        )
        .bind(group_id)
        .fetch_all(&self.db)
        .await?;
        Ok(rows
            .iter()
            .map(|r| GroupMember {
                agent_id: r.get("agent_id"),
                is_owner: r.get::<i64, _>("is_owner") != 0,
            })
            .collect())
    }

    pub async fn create_group(&self, name: &str, agent_ids: &[String]) -> anyhow::Result<Group> {
        anyhow::ensure!(!name.trim().is_empty(), "group name is required");
        anyhow::ensure!(!agent_ids.is_empty(), "a group needs at least one agent");
        let id = new_id();
        let ts = now();
        sqlx::query("INSERT INTO groups (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)")
            .bind(&id)
            .bind(name.trim())
            .bind(&ts)
            .bind(&ts)
            .execute(&self.db)
            .await?;
        for (i, agent_id) in agent_ids.iter().enumerate() {
            self.add_group_member_at(&id, agent_id, i == 0, i as i64)
                .await?;
        }
        Ok(self.get_group(&id).await?.expect("just created"))
    }

    async fn add_group_member_at(
        &self,
        group_id: &str,
        agent_id: &str,
        owner: bool,
        position: i64,
    ) -> anyhow::Result<()> {
        sqlx::query("INSERT OR IGNORE INTO group_members (group_id, agent_id, is_owner, position) VALUES (?, ?, ?, ?)")
            .bind(group_id)
            .bind(agent_id)
            .bind(owner as i64)
            .bind(position)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn add_group_member(&self, group_id: &str, agent_id: &str) -> anyhow::Result<()> {
        let pos = sqlx::query(
            "SELECT COALESCE(MAX(position), -1) + 1 AS p FROM group_members WHERE group_id = ?",
        )
        .bind(group_id)
        .fetch_one(&self.db)
        .await?
        .get::<i64, _>("p");
        self.add_group_member_at(group_id, agent_id, false, pos)
            .await
    }

    pub async fn remove_group_member(&self, group_id: &str, agent_id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM group_members WHERE group_id = ? AND agent_id = ?")
            .bind(group_id)
            .bind(agent_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn update_group(
        &self,
        id: &str,
        name: Option<String>,
        max_auto_turns: Option<i64>,
    ) -> anyhow::Result<()> {
        if let Some(n) = name.filter(|n| !n.trim().is_empty()) {
            sqlx::query("UPDATE groups SET name = ?, updated_at = ? WHERE id = ?")
                .bind(n.trim())
                .bind(now())
                .bind(id)
                .execute(&self.db)
                .await?;
        }
        if let Some(t) = max_auto_turns {
            sqlx::query("UPDATE groups SET max_auto_turns = ? WHERE id = ?")
                .bind(t.clamp(0, 50))
                .bind(id)
                .execute(&self.db)
                .await?;
        }
        Ok(())
    }

    pub async fn delete_group(&self, id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM messages WHERE thread_id = ?")
            .bind(id)
            .execute(&self.db)
            .await?;
        sqlx::query("DELETE FROM groups WHERE id = ?")
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    // ---------- messages ----------

    pub async fn insert_message(&self, m: &Message) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO messages (id, thread_id, role, agent_id, content, tool_calls, tool_call_id, tool_name, status, input_tokens, output_tokens, created_at, attachments)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&m.id)
        .bind(&m.thread_id)
        .bind(m.role.as_str())
        .bind(&m.agent_id)
        .bind(&m.content)
        .bind(m.tool_calls.as_ref().map(serde_json::to_string).transpose()?)
        .bind(&m.tool_call_id)
        .bind(&m.tool_name)
        .bind(&m.status)
        .bind(m.input_tokens)
        .bind(m.output_tokens)
        .bind(&m.created_at)
        .bind(
            (!m.attachments.is_empty())
                .then(|| serde_json::to_string(&m.attachments))
                .transpose()?,
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn update_message(&self, m: &Message) -> anyhow::Result<()> {
        sqlx::query("UPDATE messages SET content = ?, tool_calls = ?, status = ?, input_tokens = ?, output_tokens = ? WHERE id = ?")
            .bind(&m.content)
            .bind(m.tool_calls.as_ref().map(serde_json::to_string).transpose()?)
            .bind(&m.status)
            .bind(m.input_tokens)
            .bind(m.output_tokens)
            .bind(&m.id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    /// Most recent `limit` messages of a thread, oldest first.
    pub async fn messages(&self, thread_id: &str, limit: i64) -> anyhow::Result<Vec<Message>> {
        let rows = sqlx::query(
            "SELECT * FROM (SELECT *, rowid AS seq FROM messages WHERE thread_id = ? ORDER BY created_at DESC, rowid DESC LIMIT ?) ORDER BY created_at, seq",
        )
        .bind(thread_id)
        .bind(limit)
        .fetch_all(&self.db)
        .await?;
        Ok(rows.iter().map(message_from_row).collect())
    }

    /// Messages (yours and the agents' replies) that contain the words, newest first. Chats between
    /// agents and tool output are left out.
    pub async fn search_messages(
        &self,
        query: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<crate::models::SearchHit>> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(vec![]);
        }
        let like = format!(
            "%{}%",
            q.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let rows = sqlx::query(
            "SELECT id, thread_id, role, agent_id, content, created_at FROM messages WHERE role IN ('user', 'assistant') AND thread_id NOT LIKE 'dm\\_%' ESCAPE '\\' AND thread_id NOT LIKE 'wa\\_%' ESCAPE '\\' AND content LIKE ? ESCAPE '\\' ORDER BY created_at DESC LIMIT ?",
        )
        .bind(like)
        .bind(limit)
        .fetch_all(&self.db)
        .await?;
        Ok(rows
            .iter()
            .map(|r| crate::models::SearchHit {
                message_id: r.get("id"),
                thread_id: r.get("thread_id"),
                role: r.get("role"),
                agent_id: r.get("agent_id"),
                content: r.get("content"),
                created_at: r.get("created_at"),
            })
            .collect())
    }

    /// Every file shared in a thread, newest first, with the message that carried it.
    pub async fn thread_files(
        &self,
        thread_id: &str,
    ) -> anyhow::Result<Vec<(Attachment, Message)>> {
        let rows = sqlx::query(
            "SELECT * FROM messages WHERE thread_id = ? AND attachments IS NOT NULL ORDER BY created_at DESC, rowid DESC",
        )
        .bind(thread_id)
        .fetch_all(&self.db)
        .await?;
        Ok(rows
            .iter()
            .map(message_from_row)
            .flat_map(|m| {
                m.attachments
                    .clone()
                    .into_iter()
                    .map(move |a| (a, m.clone()))
            })
            .collect())
    }

    pub async fn find_attachment(&self, id: &str) -> anyhow::Result<Attachment> {
        let pattern = format!("%\"id\":\"{}\"%", id.replace(['%', '_', '"'], ""));
        let row = sqlx::query("SELECT * FROM messages WHERE attachments LIKE ? LIMIT 1")
            .bind(pattern)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("file not found"))?;
        message_from_row(&row)
            .attachments
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| anyhow::anyhow!("file not found"))
    }

    pub async fn clear_messages(&self, thread_id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM messages WHERE thread_id = ?")
            .bind(thread_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn threads(&self) -> anyhow::Result<Vec<ThreadSummary>> {
        let agents = self.list_agents().await?;
        let groups = self.list_groups().await?;
        let mut out = Vec::new();
        for a in &agents {
            let (last, at) = self.last_preview(&a.id).await?;
            out.push(ThreadSummary {
                kind: ThreadKind::Agent,
                id: a.id.clone(),
                name: a.name.clone(),
                avatars: vec![a.avatar.clone()],
                last_message: last,
                last_at: at.or_else(|| Some(a.created_at.clone())),
                busy: false,
            });
        }
        for g in &groups {
            let (last, at) = self.last_preview(&g.id).await?;
            out.push(ThreadSummary {
                kind: ThreadKind::Group,
                id: g.id.clone(),
                name: g.name.clone(),
                avatars: g
                    .members
                    .iter()
                    .filter_map(|m| {
                        agents
                            .iter()
                            .find(|a| a.id == m.agent_id)
                            .map(|a| a.avatar.clone())
                    })
                    .collect(),
                last_message: last,
                last_at: at.or_else(|| Some(g.created_at.clone())),
                busy: false,
            });
        }
        out.sort_by(|a, b| b.last_at.cmp(&a.last_at));
        Ok(out)
    }

    async fn last_preview(
        &self,
        thread_id: &str,
    ) -> anyhow::Result<(Option<String>, Option<String>)> {
        let row = sqlx::query(
            "SELECT content, created_at FROM messages WHERE thread_id = ? AND role IN ('user','assistant','event') AND content != ''
             ORDER BY created_at DESC, rowid DESC LIMIT 1",
        )
        .bind(thread_id)
        .fetch_optional(&self.db)
        .await?;
        Ok(match row {
            Some(r) => {
                let content: String = r.get("content");
                let preview: String = content
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .chars()
                    .take(120)
                    .collect();
                (Some(preview), Some(r.get("created_at")))
            }
            None => (None, None),
        })
    }

    pub async fn usage(&self) -> anyhow::Result<Vec<UsageRow>> {
        let rows = sqlx::query(
            "SELECT m.agent_id, a.name, COUNT(*) AS n, SUM(m.input_tokens) AS i, SUM(m.output_tokens) AS o
             FROM messages m LEFT JOIN agents a ON a.id = m.agent_id
             WHERE m.role = 'assistant' GROUP BY m.agent_id ORDER BY o DESC",
        )
        .fetch_all(&self.db)
        .await?;
        Ok(rows
            .iter()
            .map(|r| UsageRow {
                agent_id: r.get("agent_id"),
                agent_name: r.get("name"),
                messages: r.get("n"),
                input_tokens: r.get::<Option<i64>, _>("i").unwrap_or(0),
                output_tokens: r.get::<Option<i64>, _>("o").unwrap_or(0),
            })
            .collect())
    }

    // ---------- providers ----------

    pub async fn list_providers(&self) -> anyhow::Result<Vec<ProviderConfig>> {
        let rows = sqlx::query("SELECT * FROM providers ORDER BY created_at")
            .fetch_all(&self.db)
            .await?;
        rows.iter().map(provider_from_row).collect()
    }

    pub async fn get_provider(&self, id: &str) -> anyhow::Result<ProviderConfig> {
        let row = sqlx::query("SELECT * FROM providers WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("provider not found"))?;
        provider_from_row(&row)
    }

    pub async fn upsert_provider(&self, p: &ProviderConfig) -> anyhow::Result<ProviderConfig> {
        let id = if p.id.is_empty() {
            new_id()
        } else {
            p.id.clone()
        };
        let kind = serde_json::to_value(p.kind)?
            .as_str()
            .unwrap_or_default()
            .to_string();
        sqlx::query(
            "INSERT INTO providers (id, kind, name, base_url, default_model, enabled, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET kind = excluded.kind, name = excluded.name, base_url = excluded.base_url,
             default_model = excluded.default_model, enabled = excluded.enabled",
        )
        .bind(&id)
        .bind(kind)
        .bind(&p.name)
        .bind(&p.base_url)
        .bind(&p.default_model)
        .bind(p.enabled as i64)
        .bind(now())
        .execute(&self.db)
        .await?;
        self.get_provider(&id).await
    }

    pub async fn delete_provider(&self, id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM providers WHERE id = ?")
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn default_provider_id(&self) -> anyhow::Result<Option<String>> {
        if let Some(id) = self.setting("default_provider").await? {
            if self.get_provider(&id).await.is_ok() {
                return Ok(Some(id));
            }
        }
        Ok(self
            .list_providers()
            .await?
            .into_iter()
            .find(|p| p.enabled)
            .map(|p| p.id))
    }

    /// First run: register local engines so there is always something to pick.
    pub async fn seed_defaults(&self) -> anyhow::Result<()> {
        if !self.list_providers().await?.is_empty() {
            return Ok(());
        }
        for kind in [ProviderKind::Ollama, ProviderKind::Lmstudio] {
            let meta = crate::providers::info(kind);
            self.upsert_provider(&ProviderConfig {
                id: String::new(),
                kind,
                name: meta.label.to_string(),
                base_url: None,
                default_model: meta.default_model.map(str::to_string),
                enabled: true,
                has_key: false,
            })
            .await?;
        }
        Ok(())
    }

    /// 0.3.1 builds memory on its own by default. Agents created before that stored the old
    /// default (off), so switch them on once; the user can still turn it off afterwards.
    pub async fn enable_auto_memory_once(&self) -> anyhow::Result<()> {
        const KEY: &str = "auto_memory_default_v031";
        if self.setting(KEY).await?.is_some() {
            return Ok(());
        }
        for agent in self.list_agents().await? {
            if !agent.settings.auto_memory {
                let mut settings = agent.settings.clone();
                settings.auto_memory = true;
                self.update_agent(
                    &agent.id,
                    AgentPatch {
                        settings: Some(settings),
                        ..Default::default()
                    },
                )
                .await?;
            }
        }
        self.set_setting(KEY, "done").await
    }

    // ---------- MCP servers ----------

    pub async fn list_mcp_servers(&self) -> anyhow::Result<Vec<McpServerConfig>> {
        let rows = sqlx::query("SELECT * FROM mcp_servers ORDER BY name")
            .fetch_all(&self.db)
            .await?;
        Ok(rows.iter().map(mcp_from_row).collect())
    }

    pub async fn upsert_mcp_server(&self, s: &McpServerConfig) -> anyhow::Result<McpServerConfig> {
        anyhow::ensure!(!s.name.trim().is_empty(), "server name is required");
        let id = if s.id.is_empty() {
            new_id()
        } else {
            s.id.clone()
        };
        sqlx::query(
            "INSERT INTO mcp_servers (id, name, transport, command, args, env, url, headers, enabled, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, transport = excluded.transport, command = excluded.command,
             args = excluded.args, env = excluded.env, url = excluded.url, headers = excluded.headers, enabled = excluded.enabled",
        )
        .bind(&id)
        .bind(s.name.trim())
        .bind(if s.transport == McpTransport::Http { "http" } else { "stdio" })
        .bind(&s.command)
        .bind(serde_json::to_string(&s.args)?)
        .bind(serde_json::to_string(&s.env)?)
        .bind(&s.url)
        .bind(serde_json::to_string(&s.headers)?)
        .bind(s.enabled as i64)
        .bind(now())
        .execute(&self.db)
        .await?;
        let row = sqlx::query("SELECT * FROM mcp_servers WHERE id = ?")
            .bind(&id)
            .fetch_one(&self.db)
            .await?;
        Ok(mcp_from_row(&row))
    }

    pub async fn delete_mcp_server(&self, id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM mcp_servers WHERE id = ?")
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    // ---------- settings ----------

    pub async fn setting(&self, key: &str) -> anyhow::Result<Option<String>> {
        Ok(sqlx::query("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.db)
            .await?
            .map(|r| r.get("value")))
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn all_settings(&self) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let rows = sqlx::query("SELECT key, value FROM settings")
            .fetch_all(&self.db)
            .await?;
        Ok(rows
            .iter()
            .map(|r| (r.get("key"), r.get("value")))
            .collect())
    }
}

fn automation_from_row(r: &sqlx::sqlite::SqliteRow) -> Option<crate::automations::Automation> {
    Some(crate::automations::Automation {
        id: r.get("id"),
        agent_id: r.get("agent_id"),
        name: r.get("name"),
        trigger: serde_json::from_str(r.get::<&str, _>("trigger")).ok()?,
        prompt: r.get("prompt"),
        enabled: r.get::<i64, _>("enabled") != 0,
        last_run_at: r.get("last_run_at"),
        last_status: r.get("last_status"),
        created_at: r.get("created_at"),
        next_run_at: None,
    })
}

fn email_from_row(r: &sqlx::sqlite::SqliteRow) -> crate::email::EmailAccount {
    crate::email::EmailAccount {
        agent_id: r.get("agent_id"),
        kind: crate::email::EmailKind::parse(r.get("kind")),
        address: r.get("address"),
        display_name: r.get("display_name"),
        imap_host: r.get("imap_host"),
        imap_port: r.get::<i64, _>("imap_port") as u16,
        smtp_host: r.get("smtp_host"),
        smtp_port: r.get::<i64, _>("smtp_port") as u16,
        username: r.get("username"),
        last_seen_uid: r.get::<i64, _>("last_seen_uid") as u32,
        has_password: false,
    }
}

impl Store {
    // ---------- automations ----------

    pub async fn list_automations(
        &self,
        agent_id: Option<&str>,
    ) -> anyhow::Result<Vec<crate::automations::Automation>> {
        let rows = match agent_id {
            Some(a) => {
                sqlx::query("SELECT * FROM automations WHERE agent_id = ? ORDER BY created_at")
                    .bind(a)
                    .fetch_all(&self.db)
                    .await?
            }
            None => {
                sqlx::query("SELECT * FROM automations ORDER BY created_at")
                    .fetch_all(&self.db)
                    .await?
            }
        };
        Ok(rows.iter().filter_map(automation_from_row).collect())
    }

    pub async fn get_automation(&self, id: &str) -> anyhow::Result<crate::automations::Automation> {
        let row = sqlx::query("SELECT * FROM automations WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("automation not found"))?;
        automation_from_row(&row).ok_or_else(|| anyhow::anyhow!("automation is corrupted"))
    }

    pub async fn save_automation(
        &self,
        input: &crate::automations::AutomationInput,
    ) -> anyhow::Result<crate::automations::Automation> {
        anyhow::ensure!(!input.name.trim().is_empty(), "automation name is required");
        anyhow::ensure!(!input.prompt.trim().is_empty(), "tell the agent what to do");
        input.trigger.validate()?;
        self.get_agent(&input.agent_id).await?;
        let id = input
            .id
            .clone()
            .filter(|i| !i.is_empty())
            .unwrap_or_else(new_id);
        sqlx::query(
            "INSERT INTO automations (id, agent_id, name, trigger_kind, trigger, prompt, enabled, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, trigger_kind = excluded.trigger_kind,
             trigger = excluded.trigger, prompt = excluded.prompt, enabled = excluded.enabled",
        )
        .bind(&id)
        .bind(&input.agent_id)
        .bind(input.name.trim())
        .bind(input.trigger.kind())
        .bind(serde_json::to_string(&input.trigger)?)
        .bind(input.prompt.trim())
        .bind(input.enabled as i64)
        .bind(now())
        .execute(&self.db)
        .await?;
        self.get_automation(&id).await
    }

    pub async fn mark_automation_run(&self, id: &str, status: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE automations SET last_run_at = ?, last_status = ? WHERE id = ?")
            .bind(now())
            .bind(status)
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn delete_automation(&self, id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM automations WHERE id = ?")
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    // ---------- email accounts ----------

    pub async fn email_account(
        &self,
        agent_id: &str,
    ) -> anyhow::Result<Option<crate::email::EmailAccount>> {
        Ok(
            sqlx::query("SELECT * FROM email_accounts WHERE agent_id = ?")
                .bind(agent_id)
                .fetch_optional(&self.db)
                .await?
                .as_ref()
                .map(email_from_row),
        )
    }

    pub async fn list_email_accounts(&self) -> anyhow::Result<Vec<crate::email::EmailAccount>> {
        let rows = sqlx::query("SELECT * FROM email_accounts")
            .fetch_all(&self.db)
            .await?;
        Ok(rows.iter().map(email_from_row).collect())
    }

    pub async fn save_email_account(&self, a: &crate::email::EmailAccount) -> anyhow::Result<()> {
        anyhow::ensure!(a.address.contains('@'), "enter a valid email address");
        sqlx::query(
            "INSERT INTO email_accounts (agent_id, kind, address, display_name, imap_host, imap_port, smtp_host, smtp_port, username, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(agent_id) DO UPDATE SET kind = excluded.kind, address = excluded.address, display_name = excluded.display_name,
             imap_host = excluded.imap_host, imap_port = excluded.imap_port, smtp_host = excluded.smtp_host,
             smtp_port = excluded.smtp_port, username = excluded.username",
        )
        .bind(&a.agent_id)
        .bind(a.kind.as_str())
        .bind(a.address.trim())
        .bind(&a.display_name)
        .bind(&a.imap_host)
        .bind(a.imap_port as i64)
        .bind(&a.smtp_host)
        .bind(a.smtp_port as i64)
        .bind(&a.username)
        .bind(now())
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn set_email_seen_uid(&self, agent_id: &str, uid: u32) -> anyhow::Result<()> {
        sqlx::query("UPDATE email_accounts SET last_seen_uid = ? WHERE agent_id = ?")
            .bind(uid as i64)
            .bind(agent_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn delete_email_account(&self, agent_id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM email_accounts WHERE agent_id = ?")
            .bind(agent_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod search_tests {
    use crate::models::Role;

    #[tokio::test]
    async fn search_finds_messages_but_not_agent_chats_or_tools() {
        let dir = tempfile::tempdir().unwrap();
        let core = crate::Core::open(crate::paths::Paths::at(dir.path()).unwrap())
            .await
            .unwrap();
        for (thread, role, text) in [
            (
                "a1",
                Role::User,
                "Remind me about the Invoice from 100% Acme_Co",
            ),
            ("a1", Role::Assistant, "Done: the invoice is in your inbox"),
            ("a1", Role::Tool, "invoice tool output"),
            ("dm_a1_a2", Role::Assistant, "invoice between agents"),
        ] {
            let m = crate::runtime::message(thread, role, None, text);
            core.store.insert_message(&m).await.unwrap();
        }
        let hits = core.store.search_messages("invoice", 10).await.unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|h| h.thread_id == "a1"));
        // % and _ are plain characters, not wildcards.
        assert_eq!(
            core.store
                .search_messages("100% Acme_Co", 10)
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(core
            .store
            .search_messages("100_ Acme", 10)
            .await
            .unwrap()
            .is_empty());
        assert!(core
            .store
            .search_messages("  ", 10)
            .await
            .unwrap()
            .is_empty());
    }
}
