//! IPC commands. Errors cross the bridge as strings.

use std::collections::BTreeMap;

use opencue_core::memory::MemoryEntry;
use opencue_core::models::*;
use opencue_core::providers::{ProviderConfig, ProviderInfo};
use opencue_core::web::SearchResult;
use serde::Serialize;
use tauri::State;

use crate::AppState;

type R<T> = Result<T, String>;

fn e(err: anyhow::Error) -> String {
    format!("{err:#}")
}

#[derive(Serialize)]
pub struct AppInfo {
    version: &'static str,
    platform: &'static str,
    data_dir: String,
    space_supported: bool,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS,
        data_dir: state.core.paths().root().display().to_string(),
        space_supported: cfg!(target_os = "linux"),
    }
}

#[tauri::command]
pub async fn list_threads(state: State<'_, AppState>) -> R<Vec<ThreadSummary>> {
    state.core.threads().await.map_err(e)
}

#[tauri::command]
pub async fn list_agents(state: State<'_, AppState>) -> R<Vec<Agent>> {
    state.core.store.list_agents().await.map_err(e)
}

#[tauri::command]
pub async fn get_agent(state: State<'_, AppState>, id: String) -> R<Agent> {
    state.core.store.get_agent(&id).await.map_err(e)
}

#[tauri::command]
pub async fn create_agent(state: State<'_, AppState>, agent: NewAgent) -> R<Agent> {
    state.core.create_agent(agent).await.map_err(e)
}

#[tauri::command]
pub async fn update_agent(state: State<'_, AppState>, id: String, patch: AgentPatch) -> R<Agent> {
    state.core.update_agent(&id, patch).await.map_err(e)
}

#[tauri::command]
pub async fn delete_agent(state: State<'_, AppState>, id: String) -> R<()> {
    state.core.delete_agent(&id).await.map_err(e)
}

#[tauri::command]
pub fn get_instructions(state: State<'_, AppState>, id: String) -> String {
    state.core.store.instructions(&id)
}

#[tauri::command]
pub fn set_instructions(state: State<'_, AppState>, id: String, text: String) -> R<()> {
    state.core.store.set_instructions(&id, &text).map_err(e)
}

fn memory_file(
    state: &AppState,
    scope: &str,
    agent_id: Option<&str>,
) -> R<opencue_core::memory::MemoryFile> {
    match (scope, agent_id) {
        ("global", _) => Ok(state.core.store.global_memory()),
        (_, Some(id)) => Ok(state.core.store.agent_memory(id)),
        _ => Err("agent id required for agent memory".into()),
    }
}

#[tauri::command]
pub fn list_memory(
    state: State<'_, AppState>,
    scope: String,
    agent_id: Option<String>,
) -> R<Vec<MemoryEntry>> {
    Ok(memory_file(&state, &scope, agent_id.as_deref())?.entries())
}

#[tauri::command]
pub fn add_memory(
    state: State<'_, AppState>,
    scope: String,
    agent_id: Option<String>,
    text: String,
) -> R<MemoryEntry> {
    memory_file(&state, &scope, agent_id.as_deref())?
        .add(&text)
        .map_err(e)
}

#[tauri::command]
pub fn delete_memory(
    state: State<'_, AppState>,
    scope: String,
    agent_id: Option<String>,
    id: usize,
) -> R<()> {
    memory_file(&state, &scope, agent_id.as_deref())?
        .remove(id)
        .map(|_| ())
        .map_err(e)
}

#[tauri::command]
pub fn get_memory_raw(
    state: State<'_, AppState>,
    scope: String,
    agent_id: Option<String>,
) -> R<String> {
    Ok(memory_file(&state, &scope, agent_id.as_deref())?.raw())
}

#[tauri::command]
pub fn set_memory_raw(
    state: State<'_, AppState>,
    scope: String,
    agent_id: Option<String>,
    text: String,
) -> R<()> {
    memory_file(&state, &scope, agent_id.as_deref())?
        .set_raw(&text)
        .map_err(e)
}

#[tauri::command]
pub async fn list_groups(state: State<'_, AppState>) -> R<Vec<Group>> {
    state.core.store.list_groups().await.map_err(e)
}

#[tauri::command]
pub async fn get_group(state: State<'_, AppState>, id: String) -> R<Option<Group>> {
    state.core.store.get_group(&id).await.map_err(e)
}

#[tauri::command]
pub async fn create_group(
    state: State<'_, AppState>,
    name: String,
    agent_ids: Vec<String>,
) -> R<Group> {
    state.core.create_group(&name, &agent_ids).await.map_err(e)
}

#[tauri::command]
pub async fn update_group(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    max_auto_turns: Option<i64>,
) -> R<()> {
    state
        .core
        .store
        .update_group(&id, name, max_auto_turns)
        .await
        .map_err(e)?;
    state.core.store.get_group(&id).await.map_err(e)?;
    Ok(())
}

#[tauri::command]
pub async fn delete_group(state: State<'_, AppState>, id: String) -> R<()> {
    state.core.delete_group(&id).await.map_err(e)
}

#[tauri::command]
pub async fn add_group_member(
    state: State<'_, AppState>,
    group_id: String,
    agent_id: String,
) -> R<()> {
    state
        .core
        .store
        .add_group_member(&group_id, &agent_id)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn remove_group_member(
    state: State<'_, AppState>,
    group_id: String,
    agent_id: String,
) -> R<()> {
    state
        .core
        .store
        .remove_group_member(&group_id, &agent_id)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn get_messages(
    state: State<'_, AppState>,
    thread_id: String,
    limit: Option<i64>,
) -> R<Vec<Message>> {
    state
        .core
        .store
        .messages(&thread_id, limit.unwrap_or(300))
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn send_message(
    state: State<'_, AppState>,
    thread_id: String,
    text: String,
) -> R<Message> {
    state.core.send_message(&thread_id, &text).await.map_err(e)
}

#[tauri::command]
pub fn stop_thread(state: State<'_, AppState>, thread_id: String) {
    state.core.stop(&thread_id)
}

#[tauri::command]
pub async fn clear_thread(state: State<'_, AppState>, thread_id: String) -> R<()> {
    state.core.clear_thread(&thread_id).await.map_err(e)
}

#[tauri::command]
pub fn resolve_approval(state: State<'_, AppState>, approval_id: String, approved: bool) -> bool {
    state.core.resolve_approval(&approval_id, approved)
}

#[tauri::command]
pub fn provider_catalog(state: State<'_, AppState>) -> Vec<ProviderInfo> {
    state.core.provider_catalog().to_vec()
}

#[tauri::command]
pub async fn list_providers(state: State<'_, AppState>) -> R<Vec<ProviderConfig>> {
    state.core.providers().await.map_err(e)
}

#[tauri::command]
pub async fn save_provider(
    state: State<'_, AppState>,
    provider: ProviderConfig,
    api_key: Option<String>,
) -> R<ProviderConfig> {
    state.core.save_provider(provider, api_key).await.map_err(e)
}

#[tauri::command]
pub async fn delete_provider(state: State<'_, AppState>, id: String) -> R<()> {
    state.core.delete_provider(&id).await.map_err(e)
}

#[tauri::command]
pub async fn list_models(state: State<'_, AppState>, provider_id: String) -> R<Vec<String>> {
    state.core.list_models(&provider_id).await.map_err(e)
}

#[tauri::command]
pub async fn list_mcp_servers(state: State<'_, AppState>) -> R<Vec<McpServerConfig>> {
    state.core.store.list_mcp_servers().await.map_err(e)
}

#[tauri::command]
pub async fn mcp_status(state: State<'_, AppState>) -> R<Vec<McpServerStatus>> {
    state.core.mcp_status().await.map_err(e)
}

#[tauri::command]
pub async fn save_mcp_server(
    state: State<'_, AppState>,
    server: McpServerConfig,
) -> R<McpServerConfig> {
    state.core.save_mcp_server(server).await.map_err(e)
}

#[tauri::command]
pub async fn delete_mcp_server(state: State<'_, AppState>, id: String) -> R<()> {
    state.core.delete_mcp_server(&id).await.map_err(e)
}

#[tauri::command]
pub async fn import_mcp_json(state: State<'_, AppState>, json: String) -> R<Vec<McpServerConfig>> {
    state.core.import_mcp_json(&json).await.map_err(e)
}

#[tauri::command]
pub async fn import_mcp_file(state: State<'_, AppState>, path: String) -> R<Vec<McpServerConfig>> {
    let json = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
    state.core.import_mcp_json(&json).await.map_err(e)
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> R<BTreeMap<String, String>> {
    state.core.store.all_settings().await.map_err(e)
}

#[tauri::command]
pub async fn set_setting(state: State<'_, AppState>, key: String, value: String) -> R<()> {
    state.core.store.set_setting(&key, &value).await.map_err(e)
}

#[tauri::command]
pub async fn usage(state: State<'_, AppState>) -> R<Vec<UsageRow>> {
    state.core.store.usage().await.map_err(e)
}

#[tauri::command]
pub async fn web_search(state: State<'_, AppState>, query: String) -> R<Vec<SearchResult>> {
    state.core.web_search(&query, 8).await.map_err(e)
}

// ---------- skills ----------

use base64::Engine;
use opencue_core::automations::{Automation, AutomationInput};
use opencue_core::computer::{CommandResult, FileEntry};
use opencue_core::email::EmailAccount;
use opencue_core::skills::{Skill, SkillSummary};
use opencue_core::voice::Voice;
use opencue_core::{ComposioStatus, ComputerSettings, VoiceSettings};

#[tauri::command]
pub fn list_skills(state: State<'_, AppState>) -> Vec<SkillSummary> {
    state.core.skills()
}

#[tauri::command]
pub fn get_skill(state: State<'_, AppState>, name: String) -> Option<Skill> {
    state.core.skill(&name)
}

#[tauri::command]
pub fn save_skill(
    state: State<'_, AppState>,
    name: String,
    description: String,
    body: String,
) -> R<Skill> {
    state.core.save_skill(&name, &description, &body).map_err(e)
}

#[tauri::command]
pub fn delete_skill(state: State<'_, AppState>, name: String) -> R<()> {
    state.core.delete_skill(&name).map_err(e)
}

#[tauri::command]
pub async fn import_skill(state: State<'_, AppState>, url: String) -> R<Skill> {
    state.core.import_skill(&url).await.map_err(e)
}

// ---------- automations ----------

#[tauri::command]
pub async fn list_automations(
    state: State<'_, AppState>,
    agent_id: Option<String>,
) -> R<Vec<Automation>> {
    state.core.automations(agent_id.as_deref()).await.map_err(e)
}

#[tauri::command]
pub async fn save_automation(
    state: State<'_, AppState>,
    automation: AutomationInput,
) -> R<Automation> {
    state.core.save_automation(automation).await.map_err(e)
}

#[tauri::command]
pub async fn delete_automation(state: State<'_, AppState>, id: String) -> R<()> {
    state.core.delete_automation(&id).await.map_err(e)
}

#[tauri::command]
pub async fn run_automation(state: State<'_, AppState>, id: String) -> R<()> {
    let core = state.core.clone();
    tauri::async_runtime::spawn(async move {
        let _ = core.run_automation_now(&id).await;
    });
    Ok(())
}

#[tauri::command]
pub fn new_webhook_token() -> String {
    opencue_core::automations::new_token()
}

// ---------- email ----------

#[tauri::command]
pub async fn get_email_account(
    state: State<'_, AppState>,
    agent_id: String,
) -> R<Option<EmailAccount>> {
    state.core.email_account(&agent_id).await.map_err(e)
}

#[tauri::command]
pub async fn save_email_account(
    state: State<'_, AppState>,
    account: EmailAccount,
    password: Option<String>,
) -> R<EmailAccount> {
    state
        .core
        .save_email_account(account, password)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn delete_email_account(state: State<'_, AppState>, agent_id: String) -> R<()> {
    state.core.delete_email_account(&agent_id).await.map_err(e)
}

#[tauri::command]
pub async fn test_email(state: State<'_, AppState>, agent_id: String) -> R<()> {
    state.core.test_email(&agent_id).await.map_err(e)
}

// ---------- Composio ----------

#[tauri::command]
pub async fn composio_status(state: State<'_, AppState>) -> R<ComposioStatus> {
    Ok(state.core.composio_status().await)
}

#[tauri::command]
pub async fn save_composio(
    state: State<'_, AppState>,
    api_key: Option<String>,
    user_id: String,
) -> R<ComposioStatus> {
    state.core.save_composio(api_key, user_id).await.map_err(e)
}

// ---------- computers ----------

#[tauri::command]
pub async fn computer_settings(state: State<'_, AppState>) -> R<ComputerSettings> {
    Ok(state.core.computer_settings().await)
}

#[tauri::command]
pub async fn save_computer_settings(
    state: State<'_, AppState>,
    settings: ComputerSettings,
    space_token: Option<String>,
) -> R<ComputerSettings> {
    state
        .core
        .save_computer_settings(settings, space_token)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn test_space(state: State<'_, AppState>) -> R<usize> {
    state.core.test_space().await.map_err(e)
}

#[tauri::command]
pub async fn computer_exec(
    state: State<'_, AppState>,
    agent_id: String,
    command: String,
) -> R<CommandResult> {
    state
        .core
        .computer_exec(&agent_id, &command)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn computer_list(
    state: State<'_, AppState>,
    agent_id: String,
    path: String,
) -> R<Vec<FileEntry>> {
    state.core.computer_list(&agent_id, &path).await.map_err(e)
}

#[tauri::command]
pub async fn computer_read(
    state: State<'_, AppState>,
    agent_id: String,
    path: String,
) -> R<String> {
    state.core.computer_read(&agent_id, &path).await.map_err(e)
}

#[tauri::command]
pub fn computer_log(state: State<'_, AppState>, agent_id: String) -> String {
    state.core.computer_log(&agent_id)
}

// ---------- voice ----------

#[tauri::command]
pub async fn voice_settings(state: State<'_, AppState>) -> R<VoiceSettings> {
    Ok(state.core.voice_settings().await)
}

#[tauri::command]
pub async fn save_voice_settings(
    state: State<'_, AppState>,
    settings: VoiceSettings,
    api_key: Option<String>,
) -> R<VoiceSettings> {
    state
        .core
        .save_voice_settings(settings, api_key)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn list_voices(state: State<'_, AppState>) -> R<Vec<Voice>> {
    state.core.voices().await.map_err(e)
}

/// Audio arrives base64-encoded from the webview's MediaRecorder.
#[tauri::command]
pub async fn transcribe(
    state: State<'_, AppState>,
    audio_base64: String,
    mime: String,
) -> R<String> {
    let audio = base64::engine::general_purpose::STANDARD
        .decode(audio_base64)
        .map_err(|err| err.to_string())?;
    state.core.transcribe(audio, &mime).await.map_err(e)
}

/// Returns MP3 audio as base64.
#[tauri::command]
pub async fn speak(
    state: State<'_, AppState>,
    text: String,
    agent_id: Option<String>,
) -> R<String> {
    let audio = state
        .core
        .speak(&text, agent_id.as_deref())
        .await
        .map_err(e)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(audio))
}
