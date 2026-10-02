//! PathOfLeastResistance desktop app: main window, HUD overlay, global
//! hotkeys, live game log, build import → in-game planner, Opus 5.5 companion.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod builds;
mod game;
mod hotkeys;
mod input;
mod market;
mod mcp;
mod state;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::ai::Origin;
use crate::state::{AppState, Character, Conversation, FeedItem, Settings};

#[derive(Serialize)]
struct Snapshot {
    character: Character,
    feed: Vec<FeedItem>,
    settings: Settings,
    imported: Option<builds::ImportView>,
    planner_files: Vec<String>,
    hotkey_errors: Vec<String>,
    conversations: serde_json::Value,
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
        conversations: ai::conversation_list(&state),
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
fn ask(app: AppHandle, conv: u64, text: String) {
    ai::ask(&app, Some(conv), text, "Chat", Origin::Chat);
}

#[tauri::command]
fn what_next(app: AppHandle, conv: u64) {
    ai::ask(&app, Some(conv), ai::WHAT_NEXT.into(), "What next", Origin::Chat);
}

#[tauri::command]
fn check_item_text(app: AppHandle, conv: u64, text: String) {
    ai::ask(&app, Some(conv), ai::item_question(&text), "Item check", Origin::Chat);
}

#[tauri::command]
fn new_conversation(app: AppHandle) -> u64 {
    let state = app.state::<AppState>();
    let count = state.conversations.lock().unwrap().len() + 1;
    let id = state.new_conversation(&format!("New conversation {count}"), false);
    state.save_conversations();
    let _ = app.emit("conversations", ai::conversation_list(&state));
    id
}

#[tauri::command]
fn delete_conversation(app: AppHandle, conv: u64) -> Result<(), String> {
    let state = app.state::<AppState>();
    {
        let mut convs = state.conversations.lock().unwrap();
        if convs.iter().any(|c| c.id == conv && c.busy) {
            return Err("That conversation is still answering.".into());
        }
        convs.retain(|c| c.id != conv);
    }
    state.save_conversations();
    let _ = app.emit("conversations", ai::conversation_list(&state));
    Ok(())
}

#[tauri::command]
fn conversation(state: tauri::State<'_, AppState>, conv: u64) -> Option<Conversation> {
    state
        .conversations
        .lock()
        .unwrap()
        .iter()
        .find(|c| c.id == conv)
        .cloned()
}
#[tauri::command]
fn confirm_action(app: AppHandle, id: u64) -> Result<String, String> {
    market::confirm(&app, id)
}

#[tauri::command]
fn dismiss_action(state: tauri::State<'_, AppState>, id: u64) {
    state.actions.lock().unwrap().retain(|a| a.id != id);
}

#[tauri::command]
fn pending_actions(state: tauri::State<'_, AppState>) -> Vec<state::PendingAction> {
    state.actions.lock().unwrap().clone()
}

#[tauri::command]
fn open_trade_window(app: AppHandle) -> Result<(), String> {
    market::open_trade_window(&app, None).map(|_| ())
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
            match mcp::start(handle.clone()) {
                Ok(server) => *app.state::<AppState>().mcp.lock().unwrap() = Some(server),
                Err(e) => eprintln!("tool server not started: {e}"),
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
            new_conversation,
            delete_conversation,
            conversation,
            toggle_overlay,
            confirm_action,
            dismiss_action,
            pending_actions,
            open_trade_window,
            save_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running PathOfLeastResistance");
}
