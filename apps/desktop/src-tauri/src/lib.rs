//! Tauri shell: exposes the OpenCue core to the React UI and keeps agents running from the tray.

mod commands;

use std::sync::Arc;

use opencue_core::Core;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, WindowEvent};

pub struct AppState {
    pub core: Arc<Core>,
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn run() {
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
            tauri::async_runtime::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(ev) => {
                            let _ = handle.emit("core-event", ev);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            });
            app.manage(AppState { core });

            // Tray: agents keep working while the window is closed.
            let show = MenuItem::with_id(app, "show", "Open OpenCue", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().cloned().expect("bundle icon"))
                .tooltip("OpenCue")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
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
            commands::send_message,
            commands::stop_thread,
            commands::clear_thread,
            commands::resolve_approval,
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
            commands::get_settings,
            commands::set_setting,
            commands::usage,
            commands::web_search,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OpenCue");
}
