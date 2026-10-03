//! Lifeline desktop app: main window, HUD overlay, global
//! hotkeys, live game log, build import → in-game planner, Opus 5.5 companion.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// The MCP tool list is one large json! literal.
#![recursion_limit = "256"]

mod ai;
mod alignment;
mod builds;
mod game;
mod gear;
mod gamedata;
mod hotkeys;
mod hud_place;
mod input;
mod market;
mod mcp;
mod notify_settings;
mod rating;
mod setup;
mod single;
mod skills;
mod sound;
mod state;
mod trade_tools;
mod treeview;
mod wizard;

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
        hotkey_errors: state.startup_problems.lock().unwrap().clone(),
        conversations: ai::conversation_list(&state),
    }
}

#[tauri::command]
async fn import_build(app: AppHandle, input: String) -> Result<builds::ImportView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let view = builds::import(&state, &input)?;
        set_active_saved(&state, None);
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
fn planner_files() -> Vec<String> {
    builds::planner_files()
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
        set_active_saved(&state, None);
        let _ = app.emit("imported", &view);
        Ok(view)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The item on the clipboard (copied in game with Ctrl+C), if there is one.
#[tauri::command]
fn clipboard_item() -> Option<String> {
    let text = arboard::Clipboard::new().and_then(|mut c| c.get_text()).ok()?;
    input::is_item_text(&text).then_some(text)
}

/// Ctrl+C mode for recording gear: on (ten minutes) or off. Returns seconds left.
#[tauri::command]
fn gear_watch(app: AppHandle, on: bool) -> u64 {
    gear::set_watch(&app, on)
}

#[tauri::command]
fn gear_watch_left(state: tauri::State<'_, AppState>) -> u64 {
    gear::watch_left(&state)
}

/// Modifiers for the market pickers (item properties, then trade stats).
#[tauri::command]
async fn trade_modifiers(app: AppHandle) -> Result<Vec<serde_json::Value>, String> {
    tauri::async_runtime::spawn_blocking(move || trade_tools::modifiers(&app.state::<AppState>()))
        .await
        .map_err(|e| e.to_string())?
}

/// A market search the player set up (Market tab).
#[tauri::command]
async fn market_search(app: AppHandle, request: trade_tools::SearchRequest) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || trade_tools::search(&app, &request))
        .await
        .map_err(|e| e.to_string())?
}

/// Re-runs one Rating upgrade's market search with chosen modifiers.
#[tauri::command]
fn rerun_upgrade(app: AppHandle, index: usize, focus: rating::Focus, any_price: bool) -> Result<(), String> {
    rating::rerun_upgrade(app, index, focus, any_price)
}

/// Asks Lifeline in a new chat to rate market results for the build.
#[tauri::command]
fn rate_market_chat(app: AppHandle, search_id: String, listing_ids: Vec<String>) -> Result<u64, String> {
    trade_tools::rate_in_chat(&app, &search_id, &listing_ids)
}

/// Whether Travel shows the trade window (otherwise it works hidden).
#[tauri::command]
fn set_trade_on_travel(state: tauri::State<'_, AppState>, show: bool) -> Result<(), String> {
    let mut s = state.settings.lock().unwrap();
    s.show_trade_on_travel = show;
    s.save(&state.data_dir)
}

/// HUD pop-up and message switches (Settings → Sounds & notifications).
#[tauri::command]
fn notification_settings(state: tauri::State<'_, AppState>) -> serde_json::Value {
    notify_settings::current(&state)
}

#[tauri::command]
fn set_notification(app: AppHandle, kind: String, cue: Option<String>, on: bool) -> Result<serde_json::Value, String> {
    notify_settings::set(&app, &kind, cue.as_deref(), on)
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

/// Opens a "Build check" chat that compares the character with the plan.
#[tauri::command]
fn build_check_chat(app: AppHandle) -> u64 {
    let state = app.state::<AppState>();
    let id = state.new_conversation("Build check", false);
    state.save_conversations();
    let _ = app.emit("conversations", ai::conversation_list(&state));
    ai::ask(&app, Some(id), ai::BUILD_CHECK.into(), "Build check", Origin::Chat);
    id
}

#[tauri::command]
fn open_planner_folder() -> Result<(), String> {
    builds::open_planner_folder()
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

fn set_active_saved(state: &AppState, id: Option<u64>) {
    state.set_active_saved(id);
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
        set_active_saved(&state, None);
    }
    wizard::remove(&state, id)
}

#[tauri::command]
async fn use_saved(app: AppHandle, id: u64, write: bool) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let view = wizard::activate(&state, id)?;
        set_active_saved(&state, Some(id));
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
fn finish_setup(app: AppHandle, league: String, input: String) -> Result<Settings, String> {
    let state = app.state::<AppState>();
    setup::finish(&state, league, input)?;
    let settings = state.settings.lock().unwrap().clone();
    Ok(settings)
}

#[tauri::command]
async fn check_update() -> serde_json::Value {
    tauri::async_runtime::spawn_blocking(setup::update_available).await.ok().flatten().unwrap_or_default()
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    setup::open_url(&url)
}

/// Settings → Sounds: hear a cue.
#[tauri::command]
fn play_sound(name: String, volume: f32) {
    if let Some(cue) = sound::Cue::from_name(&name) {
        sound::preview(cue, volume);
    }
}

#[tauri::command]
fn build_status(state: tauri::State<'_, AppState>) -> serde_json::Value {
    wizard::status(&state)
}

/// Opens a "Build creator" chat: the companion asks a few questions, then
/// the build architect designs every stage and offers to write it in game.
#[tauri::command]
fn create_build_chat(app: AppHandle) -> u64 {
    let state = app.state::<AppState>();
    let id = state.new_conversation("Build creator", false);
    state.save_conversations();
    let _ = app.emit("conversations", ai::conversation_list(&state));
    ai::ask(&app, Some(id), ai::CREATE_BUILD.into(), "Create build", Origin::Chat);
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
async fn confirm_action(app: AppHandle, id: u64) -> Result<String, String> {
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
async fn open_trade_window(app: AppHandle) -> Result<(), String> {
    market::open_trade_window(&app, None).map(|_| ())
}

#[tauri::command]
async fn travel(app: AppHandle, listing_id: String, search_id: Option<String>) -> Result<String, String> {
    market::travel(&app, &listing_id, search_id.as_deref())
}

#[tauri::command]
fn equipped(state: tauri::State<'_, AppState>) -> std::collections::BTreeMap<String, String> {
    state.equipped.lock().unwrap().clone()
}

#[tauri::command]
fn forget_equipped(app: AppHandle, slot: String) {
    let state = app.state::<AppState>();
    state.equipped.lock().unwrap().remove(&slot);
    state.save_equipped();
    let _ = app.emit("equipped", state.equipped.lock().unwrap().clone());
}

#[tauri::command]
fn move_overlay(app: AppHandle) {
    hotkeys::toggle_move_mode(&app);
}

/// Everything the HUD shows besides the streamed answer.
#[tauri::command]
async fn hud(app: AppHandle) -> serde_json::Value {
    tauri::async_runtime::spawn_blocking(move || hud_now(&app.state::<AppState>()))
        .await
        .unwrap_or_default()
}

fn hud_now(state: &AppState) -> serde_json::Value {
    let c = state.character.lock().unwrap().clone();
    let next_penalty = match (c.act, c.area_level) {
        (Some(1), _) => Some("Act 2: −10%"),
        (Some(2), _) => Some("Act 3: −20%"),
        (Some(3), _) => Some("Act 4: −30%"),
        (Some(4), _) => Some("Interludes (area 54+): −40%"),
        (None, 54..=59) => Some("area 60+: −50%"),
        (None, 60..=64) => Some("endgame (area 65+): −60%"),
        _ => None,
    };
    let mut plan = serde_json::Value::Null;
    if let (Some(imported), Some(tree)) = (
        state.imported.lock().unwrap().as_ref(),
        state.tree.lock().unwrap().clone(),
    ) {
        let stage = ai::current_stage(c.act, c.area_level);
        let chosen = || imported.stages.iter().filter(|s| s.chosen);
        let pick = chosen()
            .find(|s| s.stage_key == stage)
            .or_else(|| chosen().filter(|s| s.stage_key <= stage).max_by_key(|s| s.stage_key))
            .or_else(|| chosen().next());
        if let Some(s) = pick {
            let spec = &imported.build.specs[s.spec_index];
            let planned: Vec<&lifeline_data::TreeNode> = spec
                .nodes
                .iter()
                .filter(|&&n| tree.is_plannable(n))
                .filter_map(|&n| tree.node(n))
                .collect();
            let done = planned.iter().filter(|n| c.allocated.contains(&n.id)).count();
            let mut missing: Vec<&&lifeline_data::TreeNode> =
                planned.iter().filter(|n| !c.allocated.contains(&n.id)).collect();
            missing.sort_by_key(|n| (!n.is_keystone, !n.is_notable, n.ascendancy_id.is_some()));
            plan = serde_json::json!({
                "stage": s.stage,
                "planned": planned.len(),
                "allocated": done,
                "next": missing.iter().take(4).map(|n| serde_json::json!({"name": n.name, "notable": n.is_notable || n.is_keystone})).collect::<Vec<_>>()
            });
        }
    }
    let settings = state.settings.lock().unwrap().clone();
    serde_json::json!({
        "character": c,
        "next_penalty": next_penalty,
        "plan": plan,
        "equipped_slots": state.equipped.lock().unwrap().len(),
        "rating": state.rating.lock().unwrap().clone(),
        "rotations": state.skills_plan.lock().unwrap().as_ref().map(|p| p["rotations"].clone()),
        "skills_busy": state.skills_busy.load(std::sync::atomic::Ordering::SeqCst),
        "hotkeys": settings.hotkeys,
        "unlocked": state.overlay_unlocked.load(std::sync::atomic::Ordering::SeqCst),
        "compact": input::foreground() == input::Foreground::Ours
    })
}

#[tauri::command]
fn rating_snapshot(state: tauri::State<'_, AppState>) -> serde_json::Value {
    rating::snapshot(&state)
}

#[tauri::command]
fn rate_build(app: AppHandle, budget: String, auto: bool) {
    {
        let state = app.state::<AppState>();
        let mut s = state.settings.lock().unwrap();
        s.rating_budget = budget;
        s.auto_rate_on_act = auto;
        let _ = s.save(&state.data_dir);
    }
    rating::run(app);
}

#[tauri::command]
fn overlay_action(app: AppHandle, action: String) -> Result<(), String> {
    hotkeys::run_named(&app, &action)
}

/// Brings the main window up on the in-game (hotkey) conversation.
#[tauri::command]
fn open_in_game_chat(app: AppHandle) {
    let state = app.state::<AppState>();
    let id = state.in_game_conversation();
    state.save_conversations();
    let _ = app.emit("conversations", ai::conversation_list(&state));
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
    let _ = app.emit("open-conversation", id);
}

/// Appends a line to debug.log in the app data folder (diagnostics).
pub fn debug_log(state: &AppState, msg: &str) {
    use std::io::Write;
    let path = state.data_dir.join("debug.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let _ = writeln!(f, "{t} {msg}");
    }
}

#[tauri::command]
fn log_debug(state: tauri::State<'_, AppState>, msg: String) {
    debug_log(&state, &msg);
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
fn rate_now(app: AppHandle) {
    debug_log(&app.state::<AppState>(), "rate_now invoked");
    rating::run(app);
}

#[tauri::command]
fn overlay_regions(state: tauri::State<'_, AppState>, rects: Vec<[f64; 4]>) {
    *state.overlay_regions.lock().unwrap() = rects;
}

/// Makes only the HUD's buttons clickable: polls the cursor and turns
/// click-through off while it is over a registered region.
fn spawn_overlay_hit_test(app: AppHandle) {
    std::thread::spawn(move || {
        let mut ignoring = true;
        let mut tick = 0u32;
        let mut last_fg: Option<input::Foreground> = None;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(40));
            let Some(w) = app.get_webview_window("overlay") else {
                continue;
            };
            tick = tick.wrapping_add(1);
            // Every ~200 ms: the HUD only shows while the game is running, and only
            // over the game (full) or this app (shrunk to the grade). Over anything
            // else, a minimized game, or no game at all, it hides.
            if tick % 5 == 0 {
                let fg = input::foreground();
                let enabled = app.state::<AppState>().settings.lock().unwrap().overlay_visible;
                let want = enabled
                    && match fg {
                        input::Foreground::Game => true,
                        input::Foreground::Ours => input::game_running(),
                        input::Foreground::Other => false,
                    };
                if want != w.is_visible().unwrap_or(false) {
                    // Always the top-left of the game's screen, at any size.
                    if want {
                        hud_place::pin_top_left(&w);
                    }
                    let _ = if want { w.show() } else { w.hide() };
                }
                if last_fg != Some(fg) {
                    last_fg = Some(fg);
                    let _ = app.emit_to("overlay", "hud-compact", fg == input::Foreground::Ours);
                }
            }
            let state = app.state::<AppState>();
            let unlocked = state.overlay_unlocked.load(std::sync::atomic::Ordering::SeqCst);
            let over = || -> Option<bool> {
                if !w.is_visible().ok()? {
                    return Some(false);
                }
                let cursor = app.cursor_position().ok()?;
                let origin = w.inner_position().ok()?;
                let scale = w.scale_factor().ok()?;
                let (x, y) = (
                    (cursor.x - origin.x as f64) / scale,
                    (cursor.y - origin.y as f64) / scale,
                );
                let regions = state.overlay_regions.lock().unwrap();
                Some(regions.iter().any(|r| x >= r[0] && x <= r[2] && y >= r[1] && y <= r[3]))
            };
            let ignore = !(unlocked || over().unwrap_or(false));
            // Don't drop clicks mid-drag (moving, resizing, scrolling) when the
            // cursor slips off the region it started on.
            if ignore && !ignoring && input::left_button_down() {
                continue;
            }
            if ignore != ignoring {
                ignoring = ignore;
                let _ = w.set_ignore_cursor_events(ignore);
            }
        }
    });
}

#[tauri::command]
fn toggle_overlay(app: AppHandle) {
    hotkeys::toggle_overlay(&app);
}

#[tauri::command]
fn save_settings(app: AppHandle, mut settings: Settings) -> Result<Vec<String>, String> {
    let state = app.state::<AppState>();
    {
        // The HUD's place and on/off state are owned by the HUD, not the form.
        let current = state.settings.lock().unwrap();
        settings.overlay_pos = current.overlay_pos;
        settings.overlay_visible = current.overlay_visible;
        settings.active_saved = current.active_saved;
        // Owned by the Rating screen and the welcome, not the Settings form.
        settings.rating_budget.clone_from(&current.rating_budget);
        settings.auto_rate_on_act = current.auto_rate_on_act;
        settings.show_trade_on_travel = current.show_trade_on_travel;
        settings.cue_popups.clone_from(&current.cue_popups);
        settings.hud_notices = current.hud_notices;
        settings.onboarded = settings.onboarded || current.onboarded;
    }
    settings.save(&state.data_dir)?;
    let failed = hotkeys::register(&app, &settings.hotkeys);
    *state.settings.lock().unwrap() = settings;
    Ok(failed)
}

/// The HUD: a tiny always-on-top bar the page sizes to its content (`fit_overlay`).
fn create_overlay(app: &AppHandle) -> tauri::Result<()> {
    const START_SIZE: (f64, f64) = (300.0, 44.0);
    let saved_pos = app.state::<AppState>().settings.lock().unwrap().overlay_pos;
    // Monitor bounds in logical px: (x, y, width, height).
    let bounds = |m: &tauri::Monitor| {
        let s = m.scale_factor();
        let p = m.position();
        let z = m.size();
        (p.x as f64 / s, p.y as f64 / s, z.width as f64 / s, z.height as f64 / s)
    };
    let monitors: Vec<_> = app.available_monitors().unwrap_or_default().iter().map(bounds).collect();
    let primary = app.primary_monitor().ok().flatten().map(|m| bounds(&m));
    // The saved spot if it's still on a connected monitor, else the top-left corner.
    let pos = saved_pos
        .and_then(|(x, y)| {
            monitors
                .iter()
                .find(|m| x + 40.0 >= m.0 && x < m.0 + m.2 - 40.0 && y + 10.0 >= m.1 && y < m.1 + m.3 - 30.0)
                .map(|m| (x.clamp(m.0, m.0 + m.2 - 60.0), y.clamp(m.1, m.1 + m.3 - 30.0)))
        })
        .or(primary.map(|m| (m.0 + 8.0, m.1 + 8.0)));
    let mut builder = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("overlay.html".into()))
        .title("Lifeline HUD")
        .inner_size(START_SIZE.0, START_SIZE.1)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .shadow(false)
        .focused(false)
        .focusable(false)
        .visible(false);
    if let Some((x, y)) = pos {
        builder = builder.position(x, y);
    }
    let window = builder.build()?;
    hud_place::pin_top_left(&window);
    window.set_ignore_cursor_events(true)?;
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Moved(pos) = event {
            if let Some(w) = handle.get_webview_window("overlay") {
                let l = pos.to_logical::<f64>(w.scale_factor().unwrap_or(1.0));
                hotkeys::remember_overlay(&handle, None, Some((l.x, l.y)));
            }
        }
    });
    Ok(())
}

/// The HUD page reports its content size; the window hugs it.
#[tauri::command]
fn fit_overlay(app: AppHandle, width: f64, height: f64) {
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.set_size(tauri::LogicalSize::new(width.clamp(40.0, 720.0).ceil(), height.clamp(24.0, 720.0).ceil()));
    }
}

/// The app was called PathOfLeastResistance before 0.2; carry its data
/// (settings, chats, ratings, saved builds, recorded gear) over once.
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
        .plugin(hotkeys::plugin())
        .setup(move |app| {
            let data_dir = app.path().app_data_dir()?;
            migrate_old_data(&data_dir);
            std::fs::create_dir_all(&data_dir)?;
            app.manage(AppState::new(data_dir));
            if closed_copies > 0 {
                debug_log(&app.state::<AppState>(), &format!("closed {closed_copies} other running Lifeline copies"));
            }

            let handle = app.handle().clone();
            let hotkeys = app.state::<AppState>().settings.lock().unwrap().hotkeys.clone();
            let failed = hotkeys::register(&handle, &hotkeys);
            if !failed.is_empty() {
                debug_log(&app.state::<AppState>(), &format!("hotkeys not registered: {failed:?}"));
            }
            *app.state::<AppState>().startup_problems.lock().unwrap() = failed
                .iter()
                .map(|f| format!("Hotkey {f} couldn't be used (another app may own it). Pick another in Settings."))
                .collect();
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
            create_overlay(&handle)?;
            spawn_overlay_hit_test(handle.clone());
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
            planner_files,
            ask,
            what_next,
            check_item_text,
            new_conversation,
            delete_conversation,
            conversation,
            toggle_overlay,
            travel,
            equipped,
            forget_equipped,
            move_overlay,
            hud,
            rating_snapshot,
            rate_build,
            rate_now,
            log_debug,
            create_build_chat,
            showcase,
            class_art,
            library,
            save_idea,
            remove_saved,
            use_saved,
            generate_build,
            build_status,
            play_sound,
            setup_status,
            setup_action,
            finish_setup,
            open_url,
            planner_list,
            delete_planner_files,
            open_planner_folder,
            follow_planner,
            build_alignment,
            build_check_chat,
            tree_layout,
            gear_watch,
            gear_watch_left,
            trade_modifiers,
            market_search,
            rerun_upgrade,
            rate_market_chat,
            set_trade_on_travel,
            notification_settings,
            set_notification,
            clipboard_item,
            tree_plan,
            active_saved,
            check_update,
            fit_overlay,
            skills_snapshot,
            run_skills,
            overlay_action,
            open_in_game_chat,
            overlay_regions,
            confirm_action,
            dismiss_action,
            pending_actions,
            open_trade_window,
            save_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running Lifeline");
}
