//! Loads RePoE game data (cached per game version, refreshed on patch) and
//! looks up unique-item mods on the PoE2 wiki.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use lifeline_data::{GameData, REPOE_BASE, REPOE_FILES};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::builds::http_get;
use crate::state::AppState;

const VERSION_CHECK_EVERY: Duration = Duration::from_secs(12 * 3600);

/// The loaded game data, downloading/refreshing it if needed.
pub fn load(state: &AppState) -> Result<Arc<GameData>, String> {
    if let Some(d) = state.game.lock().unwrap().clone() {
        return Ok(d);
    }
    let root = state.data_dir.join("repoe");
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let stamp = root.join("version.txt");
    let cached_version = std::fs::read_to_string(&stamp).ok().map(|s| s.trim().to_owned());
    let recent = std::fs::metadata(&stamp)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < VERSION_CHECK_EVERY);
    let version = match (&cached_version, recent) {
        (Some(v), true) => v.clone(),
        _ => match http_get(&format!("{REPOE_BASE}/version.txt")) {
            Ok(v) => v.trim().to_owned(),
            Err(e) => cached_version.clone().ok_or(e)?,
        },
    };
    let dir = root.join(&version);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut files = HashMap::new();
    for name in REPOE_FILES {
        let path = dir.join(format!("{name}.json"));
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => {
                let t = http_get(&format!("{REPOE_BASE}/{name}.min.json"))?;
                let _ = std::fs::write(&path, &t);
                t
            }
        };
        files.insert(name.to_string(), text);
    }
    let _ = std::fs::write(&stamp, &version);
    let data = Arc::new(GameData::parse(&version, &files)?);
    *state.game.lock().unwrap() = Some(data.clone());
    Ok(data)
}

/// Loads in the background at startup so the first question is fast.
pub fn preload(app: AppHandle) {
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        match load(&state) {
            Ok(d) => {
                let _ = app.emit(
                    "notice",
                    format!(
                        "Game data {} loaded ({} gems, {} item bases).",
                        d.version,
                        d.gems.len(),
                        d.bases.len()
                    ),
                );
            }
            Err(e) => {
                let _ = app.emit(
                    "notice",
                    format!("Game data not loaded yet ({e}). The AI will use the web."),
                );
            }
        }
        let _ = crate::builds::load_tree(&state);
    });
}

fn wiki_clean(s: &str) -> String {
    let s = s
        .replace("&lt;br&gt;", "\n")
        .replace("<br>", "\n")
        .replace("&#039;", "'")
        .replace("&quot;", "\"");
    // [[Page|Text]] → Text, [[Text]] → Text
    let mut out = String::new();
    let mut rest = s.as_str();
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find("]]") else { break };
        let inner = &rest[start + 2..start + end];
        out.push_str(inner.rsplit('|').next().unwrap_or(inner));
        rest = &rest[start + end + 2..];
    }
    out.push_str(rest);
    out
}

/// Unique item stats from the poe2wiki Cargo API (CC BY-NC-SA).
pub fn wiki_unique(name: &str) -> Result<Vec<Value>, String> {
    let safe: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || " '-,".contains(*c))
        .collect();
    let where_clause = format!("items.name=\"{safe}\" AND items.rarity=\"Unique\"");
    let encoded: String = where_clause
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    let url = format!(
        "https://www.poe2wiki.net/api.php?action=cargoquery&format=json&tables=items\
         &fields=items.name,items.base_item,items.required_level,items.implicit_stat_text,items.explicit_stat_text\
         &where={encoded}&limit=3"
    );
    let v: Value = serde_json::from_str(&http_get(&url)?).map_err(|e| e.to_string())?;
    Ok(v["cargoquery"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| {
            let t = &row["title"];
            json!({
                "name": t["name"],
                "base": t["base item"],
                "required_level": t["required level"],
                "implicits": wiki_clean(t["implicit stat text"].as_str().unwrap_or("")),
                "mods": wiki_clean(t["explicit stat text"].as_str().unwrap_or("")),
                "source": "poe2wiki.net (CC BY-NC-SA)"
            })
        })
        .collect())
}

/// In-game name for a gem metadata id when game data is loaded, else the id's
/// short form (`SupportGemMartialTempo` → "Rapid Attacks I" / "MartialTempo").
pub fn gem_display(state: &AppState, id: &str) -> String {
    let loaded = state.game.lock().unwrap().clone();
    loaded
        .and_then(|d| d.gem_name(id).map(str::to_owned))
        .unwrap_or_else(|| lifeline_gamefiles::build_planner::gem_short_name(id).to_owned())
}
