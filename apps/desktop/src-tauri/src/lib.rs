//! Tauri shell: exposes the OpenCue core to the React UI and keeps agents running from the tray.

mod browser;
mod commands;
mod notify;

use std::sync::Arc;

use opencue_core::Core;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, WindowEvent};

pub struct AppState {
    pub core: Arc<Core>,
    pub browser: Arc<browser::TauriBrowser>,
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// WebKitGTK blocks getUserMedia by default; enable it for the voice button and approve only
/// microphone/camera requests (the page is our own UI).
#[cfg(target_os = "linux")]
fn allow_microphone(app: &tauri::App) {
    use webkit2gtk::glib::prelude::*;
    use webkit2gtk::{PermissionRequestExt, SettingsExt, WebViewExt};
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.with_webview(|wv| {
            let view = wv.inner();
            if let Some(settings) = WebViewExt::settings(&view) {
                settings.set_enable_media_stream(true);
            }
            view.connect_permission_request(|_, req| {
                if req.is::<webkit2gtk::UserMediaPermissionRequest>() {
                    req.allow();
                    true
                } else {
                    false
                }
            });
        });
    }
}

#[cfg(not(target_os = "linux"))]
fn allow_microphone(_app: &tauri::App) {}

/// `opencue mcp-serve --agent <id>`: serve OpenCue's tools over stdio to a `claude` / `codex` CLI
/// agent instead of opening the window. Logs go to stderr; stdout carries the protocol.
fn serve_mcp(agent_id: &str) -> ! {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("warn")
        .init();
    let result = tokio::runtime::Runtime::new()
        .expect("tokio runtime")
        .block_on(async {
            let core = Core::open_secondary(opencue_core::paths::Paths::resolve()?).await?;
            opencue_core::mcp_server::serve_stdio(core, agent_id).await
        });
    if let Err(err) = result {
        eprintln!("opencue mcp-serve: {err:#}");
        std::process::exit(1);
    }
    std::process::exit(0)
}

/// Show the window and tell the UI what the tray menu asked for ("settings", "check-updates").
fn tray_action(app: &tauri::AppHandle, action: &str) {
    show_main(app);
    let _ = app.emit("tray-action", action);
}

pub fn run() {
    // A GUI app starts with a bare PATH; find the user's tools (claude, codex, grok, npx, docker).
    opencue_core::paths::fix_path_env();
    let args: Vec<String> = std::env::args().collect();
    if let Some(agent_id) = opencue_core::mcp_server::agent_from_args(&args) {
        serve_mcp(&agent_id);
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("opencue {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sqlx=warn".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app)
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            // Open inside the runtime so background tasks (MCP connections) can be spawned.
            let core = tauri::async_runtime::block_on(async {
                let core = Core::open_default().await?;
                core.start_background();
                anyhow::Ok(core)
            })?;

            // Forward core events to the UI.
            let handle = app.handle().clone();
            let mut rx = core.subscribe();
            let notify_core = core.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(ev) => {
                            notify::on_event(&handle, &notify_core, &ev).await;
                            let _ = handle.emit("core-event", ev);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            });
            let browser = browser::TauriBrowser::new(
                app.handle().clone(),
                core.paths().root().join("browser"),
            );
            core.set_browser_host(browser.clone());
            app.manage(AppState { core, browser });
            allow_microphone(app);

            // Tray: agents keep working while the window is closed. The menu opens on click, with
            // the app name and version on top.
            let title = MenuItem::with_id(
                app,
                "title",
                format!("OpenCue {}", env!("CARGO_PKG_VERSION")),
                false,
                None::<&str>,
            )?;
            let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
            let settings =
                MenuItem::with_id(app, "settings", "Settings…", true, Some("CmdOrCtrl+,"))?;
            let updates =
                MenuItem::with_id(app, "updates", "Check for Updates…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, Some("CmdOrCtrl+Q"))?;
            let menu = Menu::with_items(
                app,
                &[
                    &title,
                    &PredefinedMenuItem::separator(app)?,
                    &open,
                    &settings,
                    &updates,
                    &quit,
                ],
            )?;
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().cloned().expect("bundle icon"))
                .tooltip("OpenCue")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main(app),
                    "settings" => tray_action(app, "settings"),
                    "updates" => tray_action(app, "check-updates"),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                let to_tray =
                    tauri::async_runtime::block_on(state.core.store.setting("close_to_tray"))
                        .ok()
                        .flatten()
                        .map(|v| v != "false")
                        .unwrap_or(true);
                if to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::list_threads,
            commands::list_agents,
            commands::get_agent,
            commands::create_agent,
            commands::update_agent,
            commands::delete_agent,
            commands::get_instructions,
            commands::set_instructions,
            commands::get_soul,
            commands::set_soul,
            commands::greet_agent,
            commands::test_notification,
            commands::list_memory,
            commands::add_memory,
            commands::delete_memory,
            commands::get_memory_raw,
            commands::set_memory_raw,
            commands::list_groups,
            commands::get_group,
            commands::create_group,
            commands::update_group,
            commands::delete_group,
            commands::add_group_member,
            commands::remove_group_member,
            commands::get_messages,
            commands::search_messages,
            commands::import_grok_template,
            commands::send_message,
            commands::thread_files,
            commands::read_attachment,
            commands::open_attachment,
            commands::reveal_attachment,
            commands::save_attachment,
            commands::stop_thread,
            commands::clear_thread,
            commands::reset_thread,
            commands::set_call_mode,
            commands::resolve_approval,
            commands::answer_question,
            commands::catalog_status,
            commands::catalog_connect,
            commands::catalog_disconnect,
            commands::composio_apps,
            commands::pending_questions,
            commands::docker_setup,
            commands::docker_prepare,
            commands::docker_start_shared,
            commands::docker_start_engine,
            commands::stop_all_computers,
            commands::provider_catalog,
            commands::list_providers,
            commands::save_provider,
            commands::delete_provider,
            commands::list_models,
            commands::list_mcp_servers,
            commands::mcp_status,
            commands::save_mcp_server,
            commands::delete_mcp_server,
            commands::import_mcp_json,
            commands::import_mcp_file,
            commands::vmpal_get,
            commands::save_vmpal,
            commands::add_vmpal,
            commands::test_vmpal,
            commands::get_settings,
            commands::set_setting,
            commands::usage,
            commands::web_search,
            commands::list_skills,
            commands::get_skill,
            commands::save_skill,
            commands::delete_skill,
            commands::import_skill,
            commands::list_automations,
            commands::save_automation,
            commands::delete_automation,
            commands::run_automation,
            commands::new_webhook_token,
            commands::get_email_account,
            commands::save_email_account,
            commands::delete_email_account,
            commands::test_email,
            commands::computer_settings,
            commands::save_computer_settings,
            commands::test_space,
            commands::computer_exec,
            commands::computer_list,
            commands::computer_read,
            commands::computer_log,
            commands::computer_desktop,
            commands::reset_computer,
            commands::stop_computer,
            commands::open_external,
            commands::cli_limits,
            commands::messenger_status,
            commands::whatsapp_configure,
            commands::whatsapp_connect,
            commands::whatsapp_disconnect,
            commands::browser_place,
            commands::browser_hide,
            commands::browser_control,
            commands::browser_state,
            commands::browser_clear,
            commands::computer_running,
            commands::agent_workspace,
            commands::default_workspace,
            commands::default_vm_folder,
            commands::voice_settings,
            commands::save_voice_settings,
            commands::list_voices,
            commands::transcribe,
            commands::speak,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OpenCue");
}
