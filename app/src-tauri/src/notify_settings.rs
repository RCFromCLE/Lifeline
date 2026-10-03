//! Switches for what Lifeline shows on the HUD: each sound cue's 3-second
//! pop-up, and the short messages (gear recorded, Travel results,
//! problems). Sound on/off per cue lives with the rest of Settings.

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

const CUES: [&str; 6] = ["level_up", "new_act", "boss_area", "penalty", "death", "ready"];

/// Every switch, resolved (a cue never set shows its pop-up).
pub fn current(state: &AppState) -> Value {
    let s = state.settings.lock().unwrap();
    let popups: serde_json::Map<String, Value> = CUES
        .iter()
        .map(|c| ((*c).to_owned(), json!(s.cue_popups.get(*c).copied().unwrap_or(true))))
        .collect();
    json!({"cue_popups": popups, "hud_notices": s.hud_notices})
}

/// Turns one switch on or off: `kind` "popup" (with `cue`) or "hud_notices".
pub fn set(app: &AppHandle, kind: &str, cue: Option<&str>, on: bool) -> Result<Value, String> {
    let state = app.state::<AppState>();
    {
        let mut s = state.settings.lock().unwrap();
        match (kind, cue) {
            ("popup", Some(c)) if CUES.contains(&c) => {
                s.cue_popups.insert(c.to_owned(), on);
            }
            ("hud_notices", _) => s.hud_notices = on,
            _ => return Err(format!("unknown notification setting {kind}")),
        }
        s.save(&state.data_dir)?;
    }
    let now = current(&state);
    let _ = app.emit("notification-settings", &now);
    Ok(now)
}
