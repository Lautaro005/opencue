//! OpenCue core: everything except the UI. The desktop app and the CLI both drive a [`Core`].

pub mod artifacts;
pub mod automations;
pub mod bridge;
pub mod browser;
pub mod catalog;
pub mod coding;
pub mod computer;
pub mod council;
pub mod dashboard;
pub mod db;
pub mod email;
pub mod events;
pub mod files;
pub mod github;
pub mod limits;
pub mod local_models;
pub mod mcp;
pub mod mcp_server;
pub mod memory;
pub mod messenger;
pub mod models;
pub mod office;
pub mod paths;
pub mod personaplex;
pub mod providers;
pub mod secrets;
pub mod skills;
pub mod store;
pub mod templates;
pub mod tools;
pub mod vmpal;
pub mod voice;
pub mod web;

mod runtime;
mod services;

pub use services::{ComputerSettings, VoiceSettings};

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tokio::sync::{broadcast, oneshot};
use tokio_util::sync::CancellationToken;

pub use events::CoreEvent;
pub use runtime::MAX_TOOL_STEPS;

use crate::mcp::McpManager;
use crate::models::*;
use crate::paths::Paths;
use crate::providers::{ProviderConfig, ProviderInfo};
use crate::secrets::{provider_key_name, Secrets};
use crate::store::Store;
use crate::web::{WebConfig, WebService};

pub struct Core {
    pub store: Store,
    secrets: Secrets,
    events: broadcast::Sender<CoreEvent>,
    mcp: McpManager,
    web: WebService,
    /// Active run per thread: (run id, cancellation token).
    runs: Mutex<HashMap<String, (String, CancellationToken)>>,
    approvals: Mutex<HashMap<String, oneshot::Sender<bool>>>,
    /// False in the tool server a CLI agent starts: it can't show cards or run other agents.
    primary: bool,
    /// Chats that are on a voice call right now.
    call_threads: Mutex<std::collections::HashSet<String>>,
    /// Sign-ins to catalog apps waiting for the browser to come back.
    catalog_flows: Mutex<HashMap<String, CancellationToken>>,
    pub(crate) wa: Mutex<messenger::WaRuntime>,
    pub(crate) browser_host: Mutex<Option<Arc<dyn browser::BrowserHost>>>,
    pub(crate) browser_allowed: Mutex<std::collections::HashSet<String>>,
    /// Docker computers are shared by every agent (mirrors the setting, for sync callers).
    shared_vm: std::sync::atomic::AtomicBool,
    idle_started: std::sync::atomic::AtomicBool,
    /// When each Docker computer was last used, to stop idle ones.
    computer_used: Mutex<HashMap<String, std::time::Instant>>,
    bridge_started: std::sync::atomic::AtomicBool,
    /// Thread each agent is currently answering in, to place its questions.
    active_threads: Mutex<HashMap<String, String>>,
    /// What an @mention in a 1:1 chat tells the agent for the turn that follows.
    mention_hints: Mutex<HashMap<String, String>>,
    skills: skills::Skills,
    /// Recent terminal output per agent for the Computer view.
    computer_logs: Mutex<HashMap<String, String>>,
    /// Model downloads in progress (Settings → Models → Local).
    pub(crate) downloads: local_models::Downloads,
    /// The PersonaPlex server (real-time voice) this app started, if any.
    pub(crate) persona: personaplex::Runtime,
}

impl Core {
    pub async fn open(paths: Paths) -> anyhow::Result<Arc<Self>> {
        Self::open_with(paths, true).await
    }

    /// Open the data folder while another OpenCue process may own it (e.g. `mcp-serve` started
    /// by a CLI agent): runs nothing in the background and leaves in-flight messages alone.
    pub async fn open_secondary(paths: Paths) -> anyhow::Result<Arc<Self>> {
        Self::open_with(paths, false).await
    }

    async fn open_with(paths: Paths, primary: bool) -> anyhow::Result<Arc<Self>> {
        let db = db::open(&paths.database()).await?;
        if primary {
            // Anything left "streaming" by a crash is finished now.
            sqlx::query("UPDATE messages SET status = 'stopped' WHERE status IN ('streaming', 'running', 'awaiting_approval')")
                .execute(&db)
                .await?;
        }
        let store = Store {
            db: db.clone(),
            paths: paths.clone(),
        };
        store.seed_defaults().await?;
        if primary {
            store.enable_auto_memory_once().await?;
        }
        let (events, _) = broadcast::channel(1024);
        Ok(Arc::new(Self {
            store,
            secrets: Secrets::new(paths.secrets_fallback_file()),
            events,
            mcp: McpManager::new(paths.root().join("oauth")),
            web: WebService::new(db),
            runs: Mutex::new(HashMap::new()),
            approvals: Mutex::new(HashMap::new()),
            primary,
            call_threads: Mutex::new(Default::default()),
            catalog_flows: Mutex::new(HashMap::new()),
            wa: Mutex::new(Default::default()),
            browser_host: Mutex::new(None),
            browser_allowed: Mutex::new(Default::default()),
            shared_vm: std::sync::atomic::AtomicBool::new(false),
            idle_started: std::sync::atomic::AtomicBool::new(false),
            computer_used: Mutex::new(HashMap::new()),
            bridge_started: std::sync::atomic::AtomicBool::new(false),
            active_threads: Mutex::new(HashMap::new()),
            mention_hints: Mutex::new(HashMap::new()),
            skills: skills::Skills::new(paths.skills_dir()),
            computer_logs: Mutex::new(HashMap::new()),
            downloads: Default::default(),
            persona: Default::default(),
        }))
    }

    pub async fn open_default() -> anyhow::Result<Arc<Self>> {
        Self::open(Paths::resolve()?).await
    }

    /// Connect enabled MCP servers and start automations in the background.
    pub fn start_background(self: &Arc<Self>) {
        self.start_bridge();
        self.start_catalog();
        self.wa_autostart();
        self.forget_composio();
        self.start_idle_monitor();
        self.start_automations();
        let core = self.clone();
        tokio::spawn(async move {
            if let Ok(servers) = core.store.list_mcp_servers().await {
                // VMPal machines start the first time an agent that picked them talks.
                for s in servers
                    .into_iter()
                    .filter(|s| s.enabled && !vmpal::is_vmpal_id(&s.id))
                {
                    let core = core.clone();
                    tokio::spawn(async move {
                        core.mcp.connect(&s).await;
                        core.emit(CoreEvent::McpChanged);
                    });
                }
            }
        });
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CoreEvent> {
        self.events.subscribe()
    }

    pub(crate) fn emit(&self, event: CoreEvent) {
        let _ = self.events.send(event);
    }

    pub fn paths(&self) -> &Paths {
        &self.store.paths
    }

    // ---------- threads / agents ----------

    pub async fn threads(&self) -> anyhow::Result<Vec<ThreadSummary>> {
        let busy: HashSet<String> = self.runs.lock().unwrap().keys().cloned().collect();
        let mut threads = self.store.threads().await?;
        for t in &mut threads {
            t.busy = busy.contains(&t.id);
        }
        Ok(threads)
    }

    pub async fn create_agent(&self, new: NewAgent) -> anyhow::Result<Agent> {
        let agent = self.store.create_agent(new).await?;
        self.emit(CoreEvent::ThreadsChanged);
        Ok(agent)
    }

    pub async fn update_agent(&self, id: &str, patch: AgentPatch) -> anyhow::Result<Agent> {
        let mut patch = patch;
        // A working folder must be a real folder; make it if it does not exist yet.
        if let Some(settings) = patch.settings.as_mut() {
            let dir = settings
                .working_dir
                .take()
                .map(|d| d.trim().to_string())
                .filter(|d| !d.is_empty());
            if let Some(dir) = dir {
                let path = std::path::Path::new(&dir);
                anyhow::ensure!(path.is_absolute(), "the working folder must be a full path");
                std::fs::create_dir_all(path)
                    .map_err(|e| anyhow::anyhow!("can't use {dir} as a working folder: {e}"))?;
                anyhow::ensure!(path.is_dir(), "{dir} is not a folder");
                settings.working_dir = Some(dir);
            }
        }
        let agent = self.store.update_agent(id, patch).await?;
        self.emit(CoreEvent::ThreadsChanged);
        Ok(agent)
    }

    pub async fn delete_agent(&self, id: &str) -> anyhow::Result<()> {
        self.stop(id);
        let _ = self.delete_email_account(id).await;
        self.store.delete_agent(id).await?;
        self.emit(CoreEvent::ThreadsChanged);
        Ok(())
    }

    pub async fn create_group(&self, name: &str, agent_ids: &[String]) -> anyhow::Result<Group> {
        let g = self.store.create_group(name, agent_ids).await?;
        self.emit(CoreEvent::ThreadsChanged);
        Ok(g)
    }

    pub async fn create_group_kind(
        &self,
        name: &str,
        agent_ids: &[String],
        kind: &str,
        config: serde_json::Value,
    ) -> anyhow::Result<Group> {
        let g = self
            .store
            .create_group_kind(name, agent_ids, kind, config)
            .await?;
        self.emit(CoreEvent::ThreadsChanged);
        Ok(g)
    }

    pub async fn set_group_config(&self, id: &str, config: serde_json::Value) -> anyhow::Result<()> {
        self.store.set_group_config(id, &config).await?;
        self.emit(CoreEvent::ThreadsChanged);
        Ok(())
    }

    pub async fn delete_group(&self, id: &str) -> anyhow::Result<()> {
        self.stop(id);
        self.store.delete_group(id).await?;
        self.emit(CoreEvent::ThreadsChanged);
        Ok(())
    }

    pub async fn clear_thread(&self, thread_id: &str) -> anyhow::Result<()> {
        self.stop(thread_id);
        self.store.clear_messages(thread_id).await?;
        // The summary covered messages that are now gone.
        let _ = self
            .store
            .set_setting(&format!("summary:{thread_id}"), "")
            .await;
        self.emit(CoreEvent::ThreadsChanged);
        Ok(())
    }

    /// A fresh start: the messages, the summary of them, the plan and the conversations the agent
    /// had with other agents are all gone (memory, instructions and files stay).
    pub async fn reset_thread(&self, thread_id: &str) -> anyhow::Result<()> {
        self.clear_thread(thread_id).await?;
        let mut agents = vec![];
        if let Some(g) = self.store.get_group(thread_id).await? {
            agents.extend(g.members.iter().map(|m| m.agent_id.clone()));
        } else {
            agents.push(thread_id.to_string());
        }
        for id in agents {
            let _ = self.store.set_plan(&id, "");
            for like in [format!("dm_%{id}%"), format!("wa_{id}_%")] {
                let _ = sqlx::query("DELETE FROM messages WHERE thread_id LIKE ?")
                    .bind(&like)
                    .execute(&self.store.db)
                    .await;
            }
        }
        self.emit(CoreEvent::ThreadsChanged);
        Ok(())
    }

    /// Mark a chat as a voice call (or end it): while it lasts the agent answers like speech.
    pub fn set_call_mode(&self, thread_id: &str, on: bool) {
        let mut calls = self.call_threads.lock().unwrap();
        if on {
            calls.insert(thread_id.to_string());
        } else {
            calls.remove(thread_id);
        }
    }

    // ---------- providers ----------

    pub fn provider_catalog(&self) -> &'static [ProviderInfo] {
        providers::CATALOG
    }

    pub async fn providers(&self) -> anyhow::Result<Vec<ProviderConfig>> {
        let mut list = self.store.list_providers().await?;
        for p in &mut list {
            p.has_key = self
                .secrets
                .get(&provider_key_name(&p.id))
                .is_some_and(|k| !k.is_empty());
        }
        Ok(list)
    }

    pub async fn save_provider(
        &self,
        config: ProviderConfig,
        api_key: Option<String>,
    ) -> anyhow::Result<ProviderConfig> {
        let mut saved = self.store.upsert_provider(&config).await?;
        if let Some(key) = api_key {
            self.secrets
                .set(&provider_key_name(&saved.id), key.trim())?;
        }
        saved.has_key = self.secrets.get(&provider_key_name(&saved.id)).is_some();
        Ok(saved)
    }

    pub async fn delete_provider(&self, id: &str) -> anyhow::Result<()> {
        self.secrets.delete(&provider_key_name(id))?;
        self.store.delete_provider(id).await
    }

    pub(crate) async fn provider_for(
        &self,
        provider_id: &str,
    ) -> anyhow::Result<(ProviderConfig, Arc<dyn providers::LlmProvider>)> {
        let cfg = self.store.get_provider(provider_id).await?;
        let key = self.secrets.get(&provider_key_name(&cfg.id));
        let p = providers::build(&cfg, key)?;
        Ok((cfg, p))
    }

    pub async fn list_models(&self, provider_id: &str) -> anyhow::Result<Vec<String>> {
        let (_, p) = self.provider_for(provider_id).await?;
        p.list_models().await
    }

    // ---------- MCP ----------

    pub async fn mcp_status(&self) -> anyhow::Result<Vec<McpServerStatus>> {
        let servers = self.store.list_mcp_servers().await?;
        Ok(self.mcp.status(&servers).await)
    }

    pub async fn save_mcp_server(
        self: &Arc<Self>,
        server: McpServerConfig,
    ) -> anyhow::Result<McpServerConfig> {
        let saved = self.store.upsert_mcp_server(&server).await?;
        self.restart_mcp(&saved).await;
        Ok(saved)
    }

    pub async fn import_mcp_json(
        self: &Arc<Self>,
        json: &str,
    ) -> anyhow::Result<Vec<McpServerConfig>> {
        let existing = self.store.list_mcp_servers().await?;
        let mut saved = Vec::new();
        for mut s in mcp::parse_config_json(json)? {
            if let Some(old) = existing.iter().find(|e| e.name == s.name) {
                s.id = old.id.clone(); // re-import updates in place
            }
            let s = self.store.upsert_mcp_server(&s).await?;
            self.restart_mcp(&s).await;
            saved.push(s);
        }
        Ok(saved)
    }

    pub async fn delete_mcp_server(&self, id: &str) -> anyhow::Result<()> {
        self.mcp.disconnect(id).await;
        self.store.delete_mcp_server(id).await?;
        self.emit(CoreEvent::McpChanged);
        Ok(())
    }

    async fn restart_mcp(self: &Arc<Self>, s: &McpServerConfig) {
        self.mcp.disconnect(&s.id).await;
        self.emit(CoreEvent::McpChanged);
        if s.enabled {
            let core = self.clone();
            let s = s.clone();
            tokio::spawn(async move {
                core.mcp.connect(&s).await;
                core.emit(CoreEvent::McpChanged);
            });
        }
    }

    // ---------- web ----------

    pub async fn web_config(&self) -> WebConfig {
        WebConfig {
            searxng_url: self
                .store
                .setting("searxng_url")
                .await
                .ok()
                .flatten()
                .filter(|s| !s.trim().is_empty()),
        }
    }

    pub async fn web_search(
        &self,
        query: &str,
        max: usize,
    ) -> anyhow::Result<Vec<web::SearchResult>> {
        self.web.search(query, max, &self.web_config().await).await
    }

    pub async fn web_fetch(&self, url: &str, offset: usize) -> anyhow::Result<String> {
        self.web.fetch(url, offset, 12_000).await
    }

    pub async fn global_memory_enabled(&self) -> bool {
        self.store
            .setting("global_memory_enabled")
            .await
            .ok()
            .flatten()
            .map(|v| v != "false")
            .unwrap_or(true)
    }

    // ---------- approvals / control ----------

    pub fn resolve_approval(&self, approval_id: &str, approved: bool) -> bool {
        let tx = self.approvals.lock().unwrap().remove(approval_id);
        match tx {
            Some(tx) => {
                let _ = tx.send(approved);
                self.emit(CoreEvent::ApprovalResolved {
                    approval_id: approval_id.to_string(),
                    approved,
                });
                true
            }
            None => false,
        }
    }

    /// Stop whatever is running in a thread.
    pub fn stop(&self, thread_id: &str) {
        if let Some((_, token)) = self.runs.lock().unwrap().remove(thread_id) {
            token.cancel();
        }
    }

    pub fn is_busy(&self, thread_id: &str) -> bool {
        self.runs.lock().unwrap().contains_key(thread_id)
    }
}
