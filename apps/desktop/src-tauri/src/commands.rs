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

#[tauri::command]
pub fn get_soul(state: State<'_, AppState>, id: String) -> String {
    state.core.store.soul(&id)
}

#[tauri::command]
pub fn set_soul(state: State<'_, AppState>, id: String, text: String) -> R<()> {
    state.core.store.set_soul(&id, &text).map_err(e)
}

/// A new agent's first message, asking what its job will be. Only posts into an empty chat.
#[tauri::command]
pub async fn greet_agent(state: State<'_, AppState>, id: String, text: String) -> R<()> {
    state
        .core
        .greet_agent(&id, &text)
        .await
        .map(|_| ())
        .map_err(e)
}

#[tauri::command]
pub fn test_notification(app: tauri::AppHandle, title: String, body: String) -> R<()> {
    // The time makes each test different, so a repeat is never taken for the one before it.
    let stamp = chrono::Local::now().format("%H:%M:%S");
    crate::notify::show(&app, &title, &format!("{body} ({stamp})"))
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
    kind: Option<String>,
    config: Option<serde_json::Value>,
) -> R<Group> {
    state
        .core
        .create_group_kind(
            &name,
            &agent_ids,
            kind.as_deref().unwrap_or("chat"),
            config.unwrap_or_else(|| serde_json::json!({})),
        )
        .await
        .map_err(e)
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
pub async fn set_group_kind(state: State<'_, AppState>, id: String, kind: String) -> R<()> {
    state.core.set_group_kind(&id, &kind).await.map_err(e)
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
pub async fn import_grok_template(input: String) -> R<opencue_core::templates::GrokTemplate> {
    opencue_core::templates::import_grok_bot(&input)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn search_messages(
    state: State<'_, AppState>,
    query: String,
) -> R<Vec<opencue_core::models::SearchHit>> {
    state
        .core
        .store
        .search_messages(&query, 40)
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
    files: Option<Vec<NewAttachment>>,
) -> R<Message> {
    state
        .core
        .send_message_with_files(&thread_id, &text, &files.unwrap_or_default())
        .await
        .map_err(e)
}

/// Answer the last user message again, optionally after editing it.
#[tauri::command]
pub async fn regenerate_last(
    state: State<'_, AppState>,
    thread_id: String,
    text: Option<String>,
) -> R<()> {
    state
        .core
        .regenerate_last(&thread_id, text.as_deref())
        .await
        .map_err(e)
}

// ---------- artifacts and dashboard ----------

#[tauri::command]
pub fn get_artifact(
    state: State<'_, AppState>,
    id: String,
) -> R<opencue_core::artifacts::Artifact> {
    state.core.get_artifact(&id).map_err(e)
}

/// The user edited a canvas by hand: keep it as a new version.
#[tauri::command]
pub fn save_canvas(
    state: State<'_, AppState>,
    id: String,
    content: String,
) -> R<opencue_core::artifacts::Artifact> {
    state.core.save_canvas(&id, &content).map_err(e)
}

#[tauri::command]
pub fn list_artifacts(
    state: State<'_, AppState>,
    agent_id: String,
) -> Vec<opencue_core::artifacts::Artifact> {
    state.core.list_artifacts(&agent_id)
}

#[tauri::command]
pub async fn dashboard_data(state: State<'_, AppState>) -> R<serde_json::Value> {
    state.core.dashboard_data().await.map_err(e)
}

/// The page itself (for the preview outside the desktop app) and whether it is the built-in one.
#[tauri::command]
pub fn dashboard_info(state: State<'_, AppState>) -> serde_json::Value {
    serde_json::json!({ "html": state.core.dashboard_html(), "is_default": state.core.dashboard_is_default() })
}

#[tauri::command]
pub fn reset_dashboard(state: State<'_, AppState>) -> R<()> {
    state.core.reset_dashboard().map_err(e)
}

/// Where sandboxed frames load artifacts and the dashboard from.
#[tauri::command]
pub fn view_base() -> String {
    crate::views::base()
}

// ---------- files ----------

#[tauri::command]
pub async fn thread_files(
    state: State<'_, AppState>,
    thread_id: String,
) -> R<Vec<opencue_core::files::SharedFile>> {
    state.core.thread_files(&thread_id).await.map_err(e)
}

/// The file as a `data:` URL, for image previews.
#[tauri::command]
pub async fn read_attachment(state: State<'_, AppState>, id: String) -> R<String> {
    let (a, bytes) = state.core.read_attachment(&id).await.map_err(e)?;
    if bytes.len() > 15 * 1024 * 1024 {
        return Err("too large to preview".into());
    }
    Ok(format!(
        "data:{};base64,{}",
        a.mime,
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

/// Open a web link in the user's browser. Done here instead of through the plugin's URL scope,
/// which rejects long sign-in links (OAuth redirects carry paths and query strings).
#[tauri::command]
pub fn open_external(url: String) -> R<()> {
    let ok = url.starts_with("https://") || url.starts_with("http://");
    if !ok || url.chars().any(|c| c.is_control()) {
        return Err("only http and https links can be opened".into());
    }
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn cli_limits(state: State<'_, AppState>) -> R<Vec<opencue_core::limits::CliLimits>> {
    Ok(state.core.cli_limits().await)
}

// ---------- messenger (WhatsApp) ----------

#[tauri::command]
pub async fn messenger_status(
    state: State<'_, AppState>,
) -> R<opencue_core::messenger::MessengerStatus> {
    Ok(state.core.messenger_status().await)
}

#[tauri::command]
pub async fn whatsapp_configure(
    state: State<'_, AppState>,
    agent_id: Option<String>,
    allow: String,
) -> R<()> {
    state
        .core
        .whatsapp_configure(agent_id, allow)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn whatsapp_connect(state: State<'_, AppState>) -> R<()> {
    state.core.whatsapp_connect().await.map_err(e)
}

#[tauri::command]
pub async fn whatsapp_disconnect(state: State<'_, AppState>) -> R<()> {
    state.core.whatsapp_disconnect().await.map_err(e)
}

// ---------- built-in browser ----------

#[tauri::command]
pub fn browser_place(
    window: tauri::Window,
    state: State<'_, AppState>,
    agent_id: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    state.browser.place(window.label(), &agent_id, x, y, w, h);
}

#[tauri::command]
pub fn browser_hide(window: tauri::Window, state: State<'_, AppState>) {
    state.browser.hide(window.label());
}

#[tauri::command]
pub async fn browser_control(
    state: State<'_, AppState>,
    agent_id: String,
    action: String,
    url: Option<String>,
) -> R<()> {
    state
        .browser
        .control(&agent_id, &action, url)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn browser_state(state: State<'_, AppState>, agent_id: String) -> R<(String, String)> {
    Ok(state.browser.state(&agent_id).await)
}

#[tauri::command]
pub fn browser_clear(state: State<'_, AppState>, agent_id: String) -> R<()> {
    state.browser.clear(&agent_id).map_err(e)
}

#[tauri::command]
pub async fn browser_tabs(
    state: State<'_, AppState>,
    agent_id: String,
) -> R<Vec<crate::browser::TabInfo>> {
    Ok(state.browser.tabs(&agent_id).await)
}

#[tauri::command]
pub fn browser_tab_new(
    state: State<'_, AppState>,
    agent_id: String,
    url: Option<String>,
) -> R<u32> {
    state.browser.new_tab(&agent_id, url).map_err(e)
}

#[tauri::command]
pub fn browser_tab_select(state: State<'_, AppState>, agent_id: String, tab: u32) {
    state.browser.select_tab(&agent_id, tab);
}

#[tauri::command]
pub fn browser_tab_close(state: State<'_, AppState>, agent_id: String, tab: u32) {
    state.browser.close_tab(&agent_id, tab);
}

/// Open the file with the system's default app.
#[tauri::command]
pub async fn open_attachment(state: State<'_, AppState>, id: String) -> R<()> {
    let (_, path) = state.core.attachment(&id).await.map_err(e)?;
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn reveal_attachment(state: State<'_, AppState>, id: String) -> R<()> {
    let (_, path) = state.core.attachment(&id).await.map_err(e)?;
    tauri_plugin_opener::reveal_item_in_dir(path).map_err(|err| err.to_string())
}

/// Copy the file to a place the user picked in a save dialog.
#[tauri::command]
pub async fn save_attachment(state: State<'_, AppState>, id: String, dest: String) -> R<()> {
    let (_, path) = state.core.attachment(&id).await.map_err(e)?;
    std::fs::copy(path, dest)
        .map(|_| ())
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn stop_thread(state: State<'_, AppState>, thread_id: String) {
    state.core.stop(&thread_id)
}

#[tauri::command]
pub async fn reset_thread(state: State<'_, AppState>, thread_id: String) -> R<()> {
    state.core.reset_thread(&thread_id).await.map_err(e)
}

#[tauri::command]
pub fn set_call_mode(state: State<'_, AppState>, thread_id: String, on: bool) {
    state.core.set_call_mode(&thread_id, on);
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
pub async fn set_setting(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> R<()> {
    state
        .core
        .store
        .set_setting(&key, &value)
        .await
        .map_err(e)?;
    // Choosing where notifications go (system, or the notch) takes effect at once.
    if key == "notify_channel" {
        if value == "notch" {
            crate::notch::ensure(&app)?;
        } else {
            crate::notch::close(&app);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn notch_info(app: tauri::AppHandle) -> crate::notch::Info {
    crate::notch::info(&app)
}

#[tauri::command]
pub fn notch_place(app: tauri::AppHandle, width: f64, height: f64) -> R<()> {
    crate::notch::place(&app, width, height)
}

#[tauri::command]
pub fn notch_hide(app: tauri::AppHandle) {
    crate::notch::hide(&app)
}

/// Whether the main window is in front: the notch has nothing to add then.
#[tauri::command]
pub fn main_in_view(app: tauri::AppHandle) -> bool {
    use tauri::Manager;
    app.get_webview_window("main")
        .map(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false))
        .unwrap_or(false)
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
use opencue_core::{ComputerSettings, VoiceSettings};

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

// ---------- catalog (apps that sign in with OAuth) ----------

#[tauri::command]
pub async fn catalog_status(
    state: State<'_, AppState>,
) -> R<Vec<opencue_core::catalog::CatalogStatus>> {
    Ok(state.core.catalog_status().await)
}

/// Returns the sign-in page to open in the browser; the app finishes the connection itself.
#[tauri::command]
pub async fn catalog_connect(
    state: State<'_, AppState>,
    slug: String,
    client_id: Option<String>,
    client_secret: Option<String>,
) -> R<String> {
    state
        .core
        .catalog_connect(&slug, client_id, client_secret)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn composio_apps(state: State<'_, AppState>, refresh: bool) -> R<Vec<String>> {
    Ok(state.core.composio_apps(refresh).await)
}

#[tauri::command]
pub async fn catalog_disconnect(state: State<'_, AppState>, slug: String) -> R<()> {
    state.core.catalog_disconnect(&slug).await.map_err(e)
}

// ---------- questions from agents ----------

#[tauri::command]
pub async fn answer_question(
    state: State<'_, AppState>,
    question_id: String,
    answer: String,
) -> R<bool> {
    Ok(state.core.answer_question(&question_id, &answer).await)
}

#[tauri::command]
pub async fn pending_questions(
    state: State<'_, AppState>,
) -> R<Vec<opencue_core::bridge::Question>> {
    Ok(state.core.pending_questions().await)
}

// ---------- computers ----------

#[tauri::command]
pub async fn docker_setup(state: State<'_, AppState>) -> R<opencue_core::computer::DockerSetup> {
    Ok(state.core.docker_setup().await)
}

#[tauri::command]
pub async fn docker_prepare(state: State<'_, AppState>) -> R<()> {
    state.core.docker_prepare().await.map_err(e)
}

#[tauri::command]
pub async fn docker_start_engine(state: State<'_, AppState>) -> R<()> {
    state.core.docker_start_engine().await.map_err(e)
}

#[tauri::command]
pub async fn stop_all_computers(state: State<'_, AppState>) -> R<usize> {
    Ok(state.core.stop_all_computers().await)
}

#[tauri::command]
pub async fn docker_start_shared(state: State<'_, AppState>) -> R<()> {
    state.core.docker_start_shared().await.map_err(e)
}

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
pub async fn computer_desktop(state: State<'_, AppState>, agent_id: String) -> R<String> {
    state.core.computer_desktop(&agent_id).await.map_err(e)
}

#[tauri::command]
pub async fn stop_computer(state: State<'_, AppState>, agent_id: String) -> R<()> {
    state.core.stop_computer(&agent_id).await.map_err(e)
}

#[tauri::command]
pub async fn computer_running(state: State<'_, AppState>, agent_id: String) -> R<bool> {
    Ok(state.core.computer_running(&agent_id).await)
}

#[tauri::command]
pub async fn reset_computer(state: State<'_, AppState>, agent_id: String) -> R<()> {
    state.core.reset_computer(&agent_id).await.map_err(e)
}

/// The folder an agent works in right now (its own, or the one the user picked).
#[tauri::command]
pub async fn agent_workspace(state: State<'_, AppState>, id: String) -> R<String> {
    let agent = state.core.store.get_agent(&id).await.map_err(e)?;
    Ok(state.core.workspace_for(&agent).display().to_string())
}

/// Where an agent's local VM keeps its home when no folder is picked.
#[tauri::command]
pub fn default_vm_folder(state: State<'_, AppState>, id: String) -> String {
    state
        .core
        .paths()
        .agent_dir(&id)
        .join("vm")
        .display()
        .to_string()
}

/// Where an agent works when no folder is picked.
#[tauri::command]
pub fn default_workspace(state: State<'_, AppState>, id: String) -> String {
    state
        .core
        .paths()
        .agent_workspace(&id)
        .display()
        .to_string()
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

#[derive(Serialize)]
pub struct SpeechAudio {
    mime: &'static str,
    /// Base64 audio.
    data: String,
}

/// Returns the spoken audio (MP3 from ElevenLabs, WAV from a local voice) as base64.
#[tauri::command]
pub async fn speak(
    state: State<'_, AppState>,
    text: String,
    agent_id: Option<String>,
) -> R<SpeechAudio> {
    let speech = state
        .core
        .speak(&text, agent_id.as_deref())
        .await
        .map_err(e)?;
    Ok(SpeechAudio {
        mime: speech.mime,
        data: base64::engine::general_purpose::STANDARD.encode(speech.bytes),
    })
}

// ---------- PersonaPlex ----------

#[tauri::command]
pub async fn personaplex_status(
    state: State<'_, AppState>,
) -> R<opencue_core::personaplex::PersonaStatus> {
    Ok(state.core.personaplex_status().await)
}

#[tauri::command]
pub async fn personaplex_configure(
    state: State<'_, AppState>,
    lan: bool,
    offload: bool,
    token: Option<String>,
    engine: Option<String>,
) -> R<()> {
    state
        .core
        .personaplex_configure(lan, offload, token, engine)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn personaplex_install(state: State<'_, AppState>) -> R<()> {
    state.core.personaplex_install().await.map_err(e)
}

#[tauri::command]
pub async fn personaplex_start(state: State<'_, AppState>) -> R<()> {
    state.core.personaplex_start().await.map_err(e)
}

#[tauri::command]
pub async fn personaplex_stop(state: State<'_, AppState>) -> R<()> {
    state.core.personaplex_stop().await;
    Ok(())
}

// ---------- local models ----------

#[tauri::command]
pub fn models_status(state: State<'_, AppState>) -> Vec<opencue_core::local_models::ModelInfo> {
    state.core.models_status()
}

#[tauri::command]
pub async fn model_install(state: State<'_, AppState>, id: String) -> R<()> {
    state.core.model_install(&id).await.map_err(e)
}

#[tauri::command]
pub fn model_cancel(state: State<'_, AppState>, id: String) {
    state.core.model_cancel(&id)
}

#[tauri::command]
pub fn model_delete(state: State<'_, AppState>, id: String) -> R<()> {
    state.core.model_delete(&id).map_err(e)
}

#[derive(Serialize)]
pub struct VmPalInfo {
    toml: String,
    machines: Vec<opencue_core::vmpal::VmPalMachine>,
}

#[tauri::command]
pub async fn vmpal_get(state: State<'_, AppState>) -> R<VmPalInfo> {
    Ok(VmPalInfo {
        toml: state.core.vmpal_toml().await,
        machines: state.core.vmpal_machines().await,
    })
}

#[tauri::command]
pub async fn save_vmpal(state: State<'_, AppState>, toml: String) -> R<VmPalInfo> {
    let machines = state.core.save_vmpal(&toml).await.map_err(e)?;
    Ok(VmPalInfo {
        toml: state.core.vmpal_toml().await,
        machines,
    })
}

#[tauri::command]
pub async fn add_vmpal(state: State<'_, AppState>, toml: String) -> R<VmPalInfo> {
    state.core.add_vmpal(&toml).await.map_err(e)?;
    Ok(VmPalInfo {
        toml: state.core.vmpal_toml().await,
        machines: state.core.vmpal_machines().await,
    })
}

#[tauri::command]
pub async fn test_vmpal(state: State<'_, AppState>, name: String) -> R<McpServerStatus> {
    state.core.test_vmpal(&name).await.map_err(e)
}

// ---------- floating windows ----------

/// Open a panel (an artifact, a canvas or an agent's computer) in a window of its own that can
/// be dragged anywhere on the desktop. Opening the same one again brings it to the front.
#[tauri::command]
pub async fn open_float(app: tauri::AppHandle, kind: String, id: String, title: String) -> R<()> {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
    if !["artifact", "canvas", "computer"].contains(&kind.as_str()) {
        return Err("unknown panel".into());
    }
    let clean: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(64)
        .collect();
    let label = format!("float-{kind}-{clean}");
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return Ok(());
    }
    let wide = kind == "computer";
    // The page reads this to know what to show; nothing else about the window is shared.
    let init = format!(
        "window.__FLOAT__ = {};",
        serde_json::json!({ "kind": kind, "id": clean })
    );
    WebviewWindowBuilder::new(&app, label, WebviewUrl::App("index.html".into()))
        .title(title)
        .inner_size(if wide { 760.0 } else { 600.0 }, 720.0)
        .min_inner_size(320.0, 240.0)
        .resizable(true)
        .initialization_script(init)
        .build()
        .map_err(|err| err.to_string())?;
    Ok(())
}

/// Keep a floating window above the others (or let it go back).
#[tauri::command]
pub fn float_pin(window: tauri::WebviewWindow, on: bool) -> R<()> {
    window.set_always_on_top(on).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn close_float(window: tauri::WebviewWindow) -> R<()> {
    window.close().map_err(|err| err.to_string())
}

// ---------- coding groups: git and GitHub ----------

#[tauri::command]
pub async fn coding_set_repo(
    state: State<'_, AppState>,
    group_id: String,
    dir: String,
    worktrees: bool,
) -> R<opencue_core::github::RepoStatus> {
    state
        .core
        .coding_set_repo(&group_id, &dir, worktrees)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn github_status(
    state: State<'_, AppState>,
    group_id: String,
) -> R<opencue_core::github::RepoStatus> {
    state.core.github_status(&group_id).await.map_err(e)
}

#[tauri::command]
pub async fn github_pulls(
    state: State<'_, AppState>,
    group_id: String,
    pr_state: Option<String>,
) -> R<Vec<opencue_core::github::Pull>> {
    state
        .core
        .github_pulls(&group_id, pr_state.as_deref().unwrap_or("open"))
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn unqueue_message(
    state: State<'_, AppState>,
    thread_id: String,
    message_id: String,
) -> R<()> {
    state
        .core
        .unqueue_message(&thread_id, &message_id)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn coding_activity(
    state: State<'_, AppState>,
    group_id: String,
) -> R<Vec<opencue_core::coding::Lane>> {
    state.core.coding_activity(&group_id).await.map_err(e)
}

#[tauri::command]
pub async fn github_changes(
    state: State<'_, AppState>,
    group_id: String,
) -> R<opencue_core::github::Changes> {
    state.core.github_changes(&group_id).await.map_err(e)
}

#[tauri::command]
pub fn list_cli_skills() -> Vec<opencue_core::cli_skills::CliSkill> {
    opencue_core::cli_skills::list()
}

#[tauri::command]
pub async fn family_status(state: State<'_, AppState>) -> R<Vec<opencue_core::family::FamilyApp>> {
    Ok(state.core.family_status().await)
}

#[tauri::command]
pub async fn family_set_mcp(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> R<Vec<opencue_core::family::FamilyApp>> {
    state.core.family_set_mcp(&id, enabled).await.map_err(e)
}

#[tauri::command]
pub async fn family_start(state: State<'_, AppState>, id: String) -> R<()> {
    state.core.family_start(&id).await.map_err(e)
}

#[tauri::command]
pub async fn cli_versions() -> Vec<opencue_core::cli_update::CliVersion> {
    opencue_core::cli_update::versions().await
}

#[tauri::command]
pub async fn cli_update(kind: String) -> R<String> {
    opencue_core::cli_update::update(&kind).await.map_err(e)
}

#[tauri::command]
pub async fn github_commits(
    state: State<'_, AppState>,
    group_id: String,
) -> R<Vec<opencue_core::github::Commit>> {
    state.core.github_commits(&group_id).await.map_err(e)
}

#[tauri::command]
pub async fn github_commit_patch(
    state: State<'_, AppState>,
    group_id: String,
    sha: String,
) -> R<opencue_core::github::Changes> {
    state
        .core
        .github_commit_patch(&group_id, &sha)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn github_pull_changes(
    state: State<'_, AppState>,
    group_id: String,
    number: u64,
) -> R<opencue_core::github::Changes> {
    state
        .core
        .github_pull_changes(&group_id, number)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn github_create_pr(
    state: State<'_, AppState>,
    group_id: String,
    title: String,
    body: String,
    base: Option<String>,
    draft: bool,
) -> R<String> {
    state
        .core
        .github_create_pr(&group_id, &title, &body, base.as_deref(), draft)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn github_merge_pr(
    state: State<'_, AppState>,
    group_id: String,
    number: u64,
    method: String,
    auto: bool,
) -> R<String> {
    state
        .core
        .github_merge_pr(&group_id, number, &method, auto)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn github_comment(
    state: State<'_, AppState>,
    group_id: String,
    number: u64,
    body: String,
) -> R<()> {
    state
        .core
        .github_comment(&group_id, number, &body)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn coding_fix_ci(state: State<'_, AppState>, group_id: String, number: u64) -> R<()> {
    state.core.coding_fix_ci(&group_id, number).await.map_err(e)
}

// ---------- terminal sessions ----------

#[tauri::command]
pub fn term_tools(state: State<'_, AppState>) -> Vec<opencue_core::terminal::ToolInfo> {
    state.core.term_tools()
}

#[tauri::command]
pub async fn term_list(state: State<'_, AppState>) -> R<Vec<opencue_core::terminal::TermInfo>> {
    Ok(state.core.term_list().await)
}

#[tauri::command]
pub async fn term_create(
    state: State<'_, AppState>,
    tool: String,
    cwd: String,
    cols: u16,
    rows: u16,
) -> R<opencue_core::terminal::TermInfo> {
    state
        .core
        .term_create(&tool, &cwd, cols, rows)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn term_restart(state: State<'_, AppState>, id: String, cols: u16, rows: u16) -> R<()> {
    state.core.term_restart(&id, cols, rows).await.map_err(e)
}

#[tauri::command]
pub fn term_write(state: State<'_, AppState>, id: String, data: String) -> R<()> {
    state.core.term_write(&id, &data).map_err(e)
}

#[tauri::command]
pub fn term_resize(state: State<'_, AppState>, id: String, cols: u16, rows: u16) {
    state.core.term_resize(&id, cols, rows)
}

#[tauri::command]
pub fn term_snapshot(state: State<'_, AppState>, id: String) -> String {
    state.core.term_snapshot(&id)
}

#[tauri::command]
pub fn term_kill(state: State<'_, AppState>, id: String) {
    state.core.term_kill(&id)
}

#[tauri::command]
pub async fn term_close(state: State<'_, AppState>, id: String) -> R<()> {
    state.core.term_close(&id).await.map_err(e)
}

#[tauri::command]
pub async fn term_rename(state: State<'_, AppState>, id: String, label: String) -> R<()> {
    state.core.term_rename(&id, &label).await.map_err(e)
}

/// The user's home folder: where a new terminal session starts unless another is picked.
#[tauri::command]
pub fn home_dir() -> String {
    directories::UserDirs::new()
        .map(|d| d.home_dir().display().to_string())
        .unwrap_or_default()
}

// ---------- SPACE (Perplexity's sandbox) ----------

#[tauri::command]
pub async fn space_status(state: State<'_, AppState>) -> R<opencue_core::space_setup::SpaceStatus> {
    Ok(state.core.space_status().await)
}

#[tauri::command]
pub async fn space_install(state: State<'_, AppState>) -> R<()> {
    state.core.space_install().await.map_err(e)
}

#[tauri::command]
pub async fn space_start(state: State<'_, AppState>) -> R<()> {
    state.core.space_start().await.map_err(e)
}

#[tauri::command]
pub async fn space_stop(state: State<'_, AppState>) -> R<()> {
    state.core.space_stop().await.map_err(e)
}

#[tauri::command]
pub fn space_cancel(state: State<'_, AppState>) {
    state.core.space_cancel()
}
