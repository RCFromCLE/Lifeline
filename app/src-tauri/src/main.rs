//! Lifeline desktop app: builds, passives and skills for Path of Exile 2.
//! The main window (build list, your build, passive tree, skills), the
//! second-screen dashboard, the live game log, and build import → in-game
//! planner. Claude runs for two things only: creating a full build and
//! suggesting skills and rotations.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// The MCP tool list is one large json! literal.
#![recursion_limit = "256"]

mod ai;
mod alignment;
mod builds;
mod chime;
mod game;
mod gamedata;
mod mcp;
mod overlay;
mod setup;
mod single;
mod skills;
mod state;
mod treeview;
mod wizard;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::state::{AppState, Character, Settings};

#[derive(Serialize)]
struct Snapshot {
    character: Character,
    settings: Settings,
    imported: Option<builds::ImportView>,
    planner_files: Vec<String>,
    problems: Vec<String>,
}

#[tauri::command]
fn snapshot(state: tauri::State<'_, AppState>) -> Snapshot {
    Snapshot {
        character: state.character.lock().unwrap().clone(),
        settings: state.settings.lock().unwrap().clone(),
        imported: state.imported.lock().unwrap().as_ref().map(|i| builds::view(i, None)),
        planner_files: builds::planner_files(),
        problems: state.startup_problems.lock().unwrap().clone(),
    }
}

#[tauri::command]
async fn import_build(app: AppHandle, input: String) -> Result<builds::ImportView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let view = builds::import(&state, &input)?;
        state.set_active_saved(None);
        let _ = app.emit("imported", &view);
        Ok(view)
    })
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
fn planner_list() -> Vec<builds::PlannerFile> {
    builds::planner_list()
}

#[tauri::command]
fn delete_planner_files(files: Vec<String>) -> Result<usize, String> {
    builds::delete_planner_files(&files)
}

#[tauri::command]
async fn follow_planner(app: AppHandle, files: Vec<String>) -> Result<builds::ImportView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let view = builds::follow_planner(&state, &files)?;
        state.set_active_saved(None);
        let _ = app.emit("imported", &view);
        Ok(view)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn open_planner_folder() -> Result<(), String> {
    builds::open_planner_folder()
}

/// The whole passive tree's layout (drawn once, then cached by the page).
#[tauri::command]
async fn tree_layout(app: AppHandle) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || treeview::layout(&app.state::<AppState>()))
        .await
        .map_err(|e| e.to_string())?
}

/// The followed build's tree plan for a stage ("Act 2"; default: current).
#[tauri::command]
async fn tree_plan(app: AppHandle, stage: Option<String>) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || treeview::plan(&app.state::<AppState>(), stage.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

/// Passives taken against the plan, and the stage's skills and supports.
#[tauri::command]
async fn build_alignment(app: AppHandle) -> serde_json::Value {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _ = builds::load_tree(&state);
        alignment::alignment(&state)
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
async fn showcase(app: AppHandle, prefs: lifeline_model::Preferences) -> serde_json::Value {
    tauri::async_runtime::spawn_blocking(move || wizard::showcase(&app.state::<AppState>(), &prefs))
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn class_art(app: AppHandle) -> serde_json::Value {
    tauri::async_runtime::spawn_blocking(move || wizard::art(&app.state::<AppState>()))
        .await
        .unwrap_or_default()
}

/// The My builds entry currently followed (if any).
#[tauri::command]
fn active_saved(state: tauri::State<'_, AppState>) -> Option<u64> {
    if let Some(id) = state.settings.lock().unwrap().active_saved {
        return Some(id);
    }
    // Followed before this was tracked: the newest saved design with that name.
    let imported = state.imported.lock().unwrap().as_ref().filter(|i| i.source == "Lifeline").map(|i| i.name.clone())?;
    wizard::library(&state)
        .into_iter()
        .filter(|e| e.design.as_ref().is_some_and(|d| d.name == imported))
        .max_by_key(|e| e.saved_at)
        .map(|e| e.id)
}

#[tauri::command]
fn library(state: tauri::State<'_, AppState>) -> Vec<wizard::SavedBuild> {
    wizard::library(&state)
}

#[tauri::command]
fn save_idea(state: tauri::State<'_, AppState>, id: String) -> Result<Vec<wizard::SavedBuild>, String> {
    wizard::save_idea(&state, &id)
}

#[tauri::command]
fn remove_saved(state: tauri::State<'_, AppState>, id: u64) -> Vec<wizard::SavedBuild> {
    if state.settings.lock().unwrap().active_saved == Some(id) {
        state.set_active_saved(None);
    }
    wizard::remove(&state, id)
}

#[tauri::command]
async fn use_saved(app: AppHandle, id: u64, write: bool) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let view = wizard::activate(&state, id)?;
        state.set_active_saved(Some(id));
        let _ = app.emit("imported", &view);
        let files = if write { builds::write_stages(&state)? } else { Vec::new() };
        Ok(serde_json::json!({"view": view, "files": files}))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn generate_build(app: AppHandle, id: String, prefs: lifeline_model::Preferences) -> Result<(), String> {
    wizard::generate(app, id, prefs)
}

#[tauri::command]
fn build_status(state: tauri::State<'_, AppState>) -> serde_json::Value {
    wizard::status(&state)
}

#[tauri::command]
fn skills_snapshot(state: tauri::State<'_, AppState>) -> serde_json::Value {
    skills::snapshot(&state)
}

#[tauri::command]
fn run_skills(app: AppHandle) {
    skills::run(app);
}

#[tauri::command]
async fn setup_status(app: AppHandle) -> serde_json::Value {
    tauri::async_runtime::spawn_blocking(move || setup::status(&app.state::<AppState>()))
        .await
        .unwrap_or_default()
}

#[tauri::command]
fn setup_action(action: String) -> Result<(), String> {
    match action.as_str() {
        "install_claude" => setup::install_claude(),
        "install_git" => setup::install_git(),
        "login" => setup::login_claude(),
        _ => Err("unknown setup action".into()),
    }
}

#[tauri::command]
fn finish_setup(app: AppHandle, input: String) -> Result<Settings, String> {
    let state = app.state::<AppState>();
    setup::finish(&state, input)?;
    let settings = state.settings.lock().unwrap().clone();
    Ok(settings)
}

#[tauri::command]
async fn check_update() -> serde_json::Value {
    tauri::async_runtime::spawn_blocking(setup::update_available).await.ok().flatten().unwrap_or_default()
}

/// The overlay page's content size; its window hugs it.
#[tauri::command]
fn fit_overlay(app: AppHandle, width: f64, height: f64) {
    overlay::fit(&app, width, height);
}

/// The overlay's lock: unlocked it can be dragged; locking remembers where.
#[tauri::command]
fn overlay_lock(app: AppHandle, locked: bool) -> Result<(), String> {
    overlay::set_locked(&app, locked)
}

/// Settings: hear the "right passive" chime.
#[tauri::command]
fn preview_chime() {
    chime::play();
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    setup::open_url(&url)
}

/// Appends a line to debug.log in the app data folder (diagnostics).
pub fn debug_log(state: &AppState, msg: &str) {
    use std::io::Write;
    let path = state.data_dir.join("debug.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 5 << 20) {
        let _ = std::fs::rename(&path, state.data_dir.join("debug.old.log"));
    }
    let msg: String = msg.chars().filter(|c| !c.is_control()).take(600).collect();
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let _ = writeln!(f, "{t} {msg}");
    }
}

/// The Settings form: controls and text size.
#[tauri::command]
fn save_settings(app: AppHandle, mut settings: Settings) -> Result<(), String> {
    let state = app.state::<AppState>();
    {
        // Owned by the welcome, My builds and the overlay, not the form.
        let current = state.settings.lock().unwrap();
        settings.active_saved = current.active_saved;
        settings.overlay_pos = current.overlay_pos;
        settings.onboarded = settings.onboarded || current.onboarded;
    }
    if !matches!(settings.input.as_str(), "playstation" | "xbox" | "keyboard") {
        settings.input = "keyboard".into();
    }
    settings.save(&state.data_dir)?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

/// The playthrough dashboard: a big window for a second screen with the
/// skills and supports and the passives. Opens maximized on a monitor other
/// than the main window's when there is one.
// Async: building a window inside a sync command deadlocks on Windows.
#[tauri::command]
async fn open_dashboard(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("dashboard") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    let main_monitor = app.get_webview_window("main").and_then(|w| w.current_monitor().ok().flatten());
    let monitors = app.available_monitors().unwrap_or_default();
    let target = monitors
        .iter()
        .find(|m| main_monitor.as_ref().map_or(true, |mm| m.position() != mm.position()))
        .or(main_monitor.as_ref());
    let mut builder = WebviewWindowBuilder::new(&app, "dashboard", WebviewUrl::App("dashboard.html".into()))
        .title("Lifeline Dashboard")
        .inner_size(1600.0, 900.0)
        .min_inner_size(900.0, 560.0);
    if let Some(m) = target {
        let p = m.position().to_logical::<f64>(m.scale_factor());
        builder = builder.position(p.x + 40.0, p.y + 40.0).maximized(true);
    }
    builder.build().map_err(|e| e.to_string())?;
    Ok(())
}

/// The app was called PathOfLeastResistance before 0.2; carry its data
/// (settings and saved builds) over once.
fn migrate_old_data(data_dir: &std::path::Path) {
    fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            let target = to.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                copy_dir(&entry.path(), &target)?;
            } else {
                std::fs::copy(entry.path(), target)?;
            }
        }
        Ok(())
    }
    if data_dir.exists() {
        return;
    }
    let Some(old) = data_dir.parent().map(|p| p.join(concat!("com.rudyc.", "pathofleastresistance"))) else {
        return;
    };
    if old.is_dir() {
        let _ = copy_dir(&old, data_dir);
    }
}

fn main() {
    // Only one Lifeline: this launch closes any other running copy.
    let closed_copies = single::close_other_copies();
    tauri::Builder::default()
        .setup(move |app| {
            let data_dir = app.path().app_data_dir()?;
            migrate_old_data(&data_dir);
            std::fs::create_dir_all(&data_dir)?;
            app.manage(AppState::new(data_dir));
            if closed_copies > 0 {
                debug_log(&app.state::<AppState>(), &format!("closed {closed_copies} other running Lifeline copies"));
            }

            let handle = app.handle().clone();
            match mcp::start(handle.clone()) {
                Ok(server) => *app.state::<AppState>().mcp.lock().unwrap() = Some(server),
                Err(e) => {
                    debug_log(&app.state::<AppState>(), &format!("tool server not started: {e}"));
                    app.state::<AppState>()
                        .startup_problems
                        .lock()
                        .unwrap()
                        .push(format!("The AI's game-data link didn't start ({e}). Restart Lifeline."));
                }
            }
            overlay::create(&handle)?;
            overlay::spawn_visibility(handle.clone());
            gamedata::preload(handle.clone());
            {
                // The last imported or created build survives restarts.
                let handle = handle.clone();
                std::thread::spawn(move || {
                    if let Some(view) = builds::restore(&handle.state::<AppState>()) {
                        let _ = handle.emit("imported", &view);
                    }
                });
            }
            game::spawn_log_watcher(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            import_build,
            write_stages,
            planner_list,
            delete_planner_files,
            open_planner_folder,
            follow_planner,
            tree_layout,
            tree_plan,
            build_alignment,
            showcase,
            class_art,
            active_saved,
            library,
            save_idea,
            remove_saved,
            use_saved,
            generate_build,
            build_status,
            skills_snapshot,
            run_skills,
            setup_status,
            setup_action,
            finish_setup,
            check_update,
            open_url,
            preview_chime,
            fit_overlay,
            overlay_lock,
            open_dashboard,
            save_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running Lifeline");
}
