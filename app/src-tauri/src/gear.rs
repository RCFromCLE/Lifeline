//! Recording what the character wears. The game doesn't share equipment, so
//! each piece comes from its copied item text: either the Record gear hotkey
//! (Lifeline sends the game's copy shortcut) or Ctrl+C mode, where the player
//! copies items with the game's own Ctrl+C and Lifeline only reads the
//! clipboard — no key presses sent at all.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

/// How long Ctrl+C mode stays on after it's started.
const WATCH_FOR: Duration = Duration::from_secs(10 * 60);

/// Stores a copied item as worn, tells every window, and returns the message.
pub fn record(app: &AppHandle, item: &str) -> String {
    let state = app.state::<AppState>();
    let recorded = crate::state::record_equipped(&mut state.equipped.lock().unwrap(), item);
    let slot = match recorded {
        Ok(slot) => slot,
        Err(why) => return why,
    };
    state.save_equipped();
    let _ = app.emit("equipped", state.equipped.lock().unwrap().clone());
    crate::sound::play_with(app, crate::sound::Cue::Ready, &format!("Gear recorded: {slot}"));
    let name: Vec<&str> = item.lines().skip(2).take(2).filter(|l| !l.starts_with("--")).collect();
    crate::debug_log(&state, &format!("gear: recorded {slot}"));
    format!("✓ Recorded {slot}: {}", name.join(" · "))
}

/// The Record gear hotkey: copy the item under the cursor and record it.
pub fn record_hovered(app: &AppHandle) {
    let state = app.state::<AppState>();
    let key = state.settings.lock().unwrap().hotkeys.record_equipped.clone();
    crate::debug_log(&state, &format!("gear: {key} pressed"));
    let app = app.clone();
    std::thread::spawn(move || {
        let log = |m: &str| crate::debug_log(&app.state::<AppState>(), m);
        let message = match crate::input::copy_hovered_item(&key, &log) {
            Ok(item) => record(&app, &item),
            Err(e) => e,
        };
        notify(&app, &message);
    });
}

fn notify(app: &AppHandle, message: &str) {
    let _ = app.emit("notice", message);
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.show();
    }
}

/// Seconds Ctrl+C mode has left (0 when off).
pub fn watch_left(state: &AppState) -> u64 {
    state
        .gear_watch_until
        .lock()
        .unwrap()
        .and_then(|until| until.checked_duration_since(Instant::now()))
        .map_or(0, |d| d.as_secs())
}

/// Turns Ctrl+C mode on (for ten minutes) or off.
pub fn set_watch(app: &AppHandle, on: bool) -> u64 {
    let state = app.state::<AppState>();
    *state.gear_watch_until.lock().unwrap() = on.then(|| Instant::now() + WATCH_FOR);
    let left = watch_left(&state);
    let _ = app.emit("gear-watch", json!({"seconds": left}));
    if on && !state.gear_watch_running.swap(true, Ordering::SeqCst) {
        let app = app.clone();
        std::thread::spawn(move || {
            watch_loop(&app);
            let state = app.state::<AppState>();
            state.gear_watch_running.store(false, Ordering::SeqCst);
            *state.gear_watch_until.lock().unwrap() = None;
            let _ = app.emit("gear-watch", json!({"seconds": 0}));
        });
    }
    left
}

/// Reads the clipboard while the game is in front; every new item copied
/// there is recorded. Ends when the time runs out or the mode is turned off.
fn watch_loop(app: &AppHandle) {
    let Ok(mut clipboard) = arboard::Clipboard::new() else { return };
    // Whatever is on the clipboard already isn't a new copy.
    let mut last = clipboard.get_text().unwrap_or_default();
    while watch_left(&app.state::<AppState>()) > 0 {
        std::thread::sleep(Duration::from_millis(250));
        let Ok(text) = clipboard.get_text() else { continue };
        if text == last {
            continue;
        }
        last.clone_from(&text);
        // Lifeline's own copies (Item check, Record gear) aren't the player's.
        if crate::input::copying_now() {
            continue;
        }
        if crate::input::is_item_text(&text) && crate::input::game_in_front() {
            let message = record(app, &text);
            notify(app, &message);
        }
    }
}
