//! PathOfLeastResistance desktop app: main window, HUD overlay, global
//! hotkeys, live game log, build import → in-game planner, Opus 5.5 companion.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod builds;
mod game;
mod hotkeys;
mod input;
mod state;

use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::ai::Origin;
use crate::state::{AppState, Character, FeedItem, Settings};

#[derive(Serialize)]
struct Snapshot {
    character: Character,
    feed: Vec<FeedItem>,
    settings: Settings,
    imported: Option<builds::ImportView>,
    planner_files: Vec<String>,
    hotkey_errors: Vec<String>,
}

#[tauri::command]
fn snapshot(state: tauri::State<'_, AppState>) -> Snapshot {
    Snapshot {
        character: state.character.lock().unwrap().clone(),
        feed: state.feed.lock().unwrap().clone(),
        settings: state.settings.lock().unwrap().clone(),
        imported: state.imported.lock().unwrap().as_ref().map(|i| builds::view(i, None)),
        planner_files: builds::planner_files(),
        hotkey_errors: Vec::new(),
    }
}

#[tauri::command]
async fn import_build(app: AppHandle, input: String) -> Result<builds::ImportView, String> {
    tauri::async_runtime::spawn_blocking(move || builds::import(&app.state::<AppState>(), &input))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn write_stages(app: AppHandle) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || builds::write_stages(&app.state::<AppState>()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn planner_files() -> Vec<String> {
    builds::planner_files()
}

#[tauri::command]
fn ask(app: AppHandle, text: String) {
    ai::ask(&app, text, "Chat", Origin::Chat);
}

#[tauri::command]
fn what_next(app: AppHandle) {
    ai::ask(&app, ai::WHAT_NEXT.into(), "What next", Origin::Chat);
}

#[tauri::command]
fn check_item_text(app: AppHandle, text: String) {
    ai::ask(&app, ai::item_question(&text), "Item check", Origin::Chat);
}

#[tauri::command]
fn new_chat(state: tauri::State<'_, AppState>) {
    *state.ai_session.lock().unwrap() = None;
}

#[tauri::command]
fn toggle_overlay(app: AppHandle) {
    hotkeys::toggle_overlay(&app);
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<Vec<String>, String> {
    let state = app.state::<AppState>();
    settings.save(&state.data_dir)?;
    let failed = hotkeys::register(&app, &settings.hotkeys);
    *state.settings.lock().unwrap() = settings;
    Ok(failed)
}

fn create_overlay(app: &AppHandle) -> tauri::Result<()> {
    let mut builder = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("overlay.html".into()))
        .title("PoLR HUD")
        .inner_size(440.0, 300.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(false)
        .visible(false);
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let scale = monitor.scale_factor();
        let width = monitor.size().width as f64 / scale;
        builder = builder.position(width - 470.0, 90.0);
    }
    let window = builder.build()?;
    window.set_ignore_cursor_events(true)?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(hotkeys::plugin())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            app.manage(AppState::new(data_dir));

            let handle = app.handle().clone();
            let hotkeys = app.state::<AppState>().settings.lock().unwrap().hotkeys.clone();
            let failed = hotkeys::register(&handle, &hotkeys);
            if !failed.is_empty() {
                eprintln!("hotkeys not registered: {failed:?}");
            }
            create_overlay(&handle)?;
            game::spawn_log_watcher(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            import_build,
            write_stages,
            planner_files,
            ask,
            what_next,
            check_item_text,
            new_chat,
            toggle_overlay,
            save_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running PathOfLeastResistance");
}
