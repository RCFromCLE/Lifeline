//! Global hotkeys. Each press triggers one app action; only the item check
//! sends anything to the game, and that is a single Ctrl+Alt+C.

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::ai::{self, Origin};
use crate::state::{AppState, Hotkeys};

#[derive(Debug, Clone, Copy)]
enum Action {
    ItemCheck,
    WhatNext,
    Ask,
    ToggleOverlay,
    RecordEquipped,
    MoveOverlay,
}

fn bindings(h: &Hotkeys) -> [(Action, &str); 6] {
    [
        (Action::ItemCheck, h.item_check.as_str()),
        (Action::WhatNext, h.what_next.as_str()),
        (Action::Ask, h.ask.as_str()),
        (Action::ToggleOverlay, h.toggle_overlay.as_str()),
        (Action::RecordEquipped, h.record_equipped.as_str()),
        (Action::MoveOverlay, h.move_overlay.as_str()),
    ]
}

pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let hotkeys = app.state::<AppState>().settings.lock().unwrap().hotkeys.clone();
            let action = bindings(&hotkeys)
                .into_iter()
                .find(|(_, s)| s.parse::<Shortcut>().is_ok_and(|p| p.id() == shortcut.id()))
                .map(|(a, _)| a);
            if let Some(action) = action {
                run(app, action);
            }
        })
        .build()
}

/// (Re)registers all hotkeys; returns the ones that failed to parse/register.
pub fn register(app: &AppHandle, hotkeys: &Hotkeys) -> Vec<String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut failed = Vec::new();
    for (_, text) in bindings(hotkeys) {
        match text.parse::<Shortcut>() {
            Ok(s) => {
                if let Err(e) = gs.register(s) {
                    failed.push(format!("{text}: {e}"));
                }
            }
            Err(e) => failed.push(format!("{text}: {e}")),
        }
    }
    failed
}

pub fn toggle_overlay(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("overlay") {
        let show = !w.is_visible().unwrap_or(false);
        let _ = if show { w.show() } else { w.hide() };
        remember_overlay(app, Some(show), None);
    }
}

/// Persists the HUD's on/off state and/or position (logical px) when they change.
pub fn remember_overlay(app: &AppHandle, visible: Option<bool>, pos: Option<(f64, f64)>) {
    update_overlay_settings(app, |s| {
        if let Some(v) = visible {
            s.overlay_visible = v;
        }
        if let Some(p) = pos {
            s.overlay_pos = Some((p.0.round(), p.1.round()));
        }
    });
}

/// Persists the HUD's size (logical px) after the player resizes it.
pub fn remember_overlay_size(app: &AppHandle, size: (f64, f64)) {
    update_overlay_settings(app, |s| s.overlay_size = Some((size.0.round(), size.1.round())));
}

fn update_overlay_settings(app: &AppHandle, change: impl FnOnce(&mut crate::state::Settings)) {
    let state = app.state::<AppState>();
    let mut settings = state.settings.lock().unwrap();
    let before = settings.clone();
    change(&mut settings);
    if *settings != before {
        let _ = settings.save(&state.data_dir);
    }
}

/// Unlocks the HUD for dragging, or locks it (click-through) and saves where it is.
pub fn toggle_move_mode(app: &AppHandle) {
    use std::sync::atomic::Ordering;
    let Some(w) = app.get_webview_window("overlay") else {
        return;
    };
    let state = app.state::<AppState>();
    let unlock = !state.overlay_unlocked.load(Ordering::SeqCst);
    state.overlay_unlocked.store(unlock, Ordering::SeqCst);
    let _ = w.show();
    let pos = (w.outer_position(), w.scale_factor());
    let pos = match pos {
        (Ok(p), Ok(scale)) if !unlock => {
            let l = p.to_logical::<f64>(scale);
            Some((l.x, l.y))
        }
        _ => None,
    };
    remember_overlay(app, Some(true), pos);
    let _ = app.emit("overlay-mode", unlock);
}

/// Runs a hotkey action by name (overlay buttons).
pub fn run_named(app: &AppHandle, name: &str) -> Result<(), String> {
    let action = match name {
        "item_check" => Action::ItemCheck,
        "what_next" => Action::WhatNext,
        "ask" => Action::Ask,
        "toggle_overlay" => Action::ToggleOverlay,
        "record_equipped" => Action::RecordEquipped,
        "move_overlay" => Action::MoveOverlay,
        "skills" => {
            crate::skills::run(app.clone());
            return Ok(());
        }
        other => return Err(format!("unknown action {other}")),
    };
    run(app, action);
    Ok(())
}

fn run(app: &AppHandle, action: Action) {
    match action {
        Action::ItemCheck => {
            let app = app.clone();
            std::thread::spawn(move || match crate::input::copy_hovered_item() {
                Ok(item) => {
                    let _ = app.emit("item", &item);
                    ai::ask(&app, None, ai::item_question(&item), "Item check", Origin::Hotkey);
                }
                Err(e) => {
                    let _ = app.emit("notice", &e);
                    if let Some(w) = app.get_webview_window("overlay") {
                        let _ = w.show();
                    }
                }
            });
        }
        Action::WhatNext => ai::ask(app, None, ai::WHAT_NEXT.into(), "What next", Origin::Hotkey),
        Action::Ask => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
            let _ = app.emit("focus-chat", ());
        }
        Action::ToggleOverlay => toggle_overlay(app),
        Action::RecordEquipped => {
            let app = app.clone();
            std::thread::spawn(move || {
                let message = match crate::input::copy_hovered_item() {
                    Ok(item) => {
                        let class = item
                            .lines()
                            .next()
                            .and_then(|l| l.strip_prefix("Item Class:"))
                            .map(|c| c.trim().to_owned())
                            .unwrap_or_else(|| "Unknown".into());
                        let name: Vec<&str> = item.lines().skip(2).take(2).collect();
                        let state = app.state::<AppState>();
                        state.equipped.lock().unwrap().insert(class.clone(), item.clone());
                        state.save_equipped();
                        let _ = app.emit("equipped", state.equipped.lock().unwrap().clone());
                        format!("Recorded as equipped ({class}): {}", name.join(" "))
                    }
                    Err(e) => e,
                };
                let _ = app.emit("notice", &message);
                if let Some(w) = app.get_webview_window("overlay") {
                    let _ = w.show();
                }
            });
        }
        Action::MoveOverlay => toggle_move_mode(app),
    }
}
