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
}

fn bindings(h: &Hotkeys) -> [(Action, &str); 4] {
    [
        (Action::ItemCheck, h.item_check.as_str()),
        (Action::WhatNext, h.what_next.as_str()),
        (Action::Ask, h.ask.as_str()),
        (Action::ToggleOverlay, h.toggle_overlay.as_str()),
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
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            let _ = w.show();
        }
    }
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
    }
}
