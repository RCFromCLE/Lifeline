//! Shared app state and the persisted settings.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use lifeline_data::PassiveTree;
use lifeline_model::SpecStage;
use lifeline_pob::PobBuild;
use serde::{Deserialize, Serialize};

/// The character the game log shows: who, how far, and which passives.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Character {
    pub name: Option<String>,
    pub class: String,
    pub level: u32,
    pub zone: String,
    pub area_id: String,
    pub area_level: u32,
    pub act: Option<u8>,
    /// Passive ids allocated this character (from the log).
    #[serde(skip)]
    pub allocated: std::collections::HashSet<String>,
    /// Other characters' allocations, kept while you switch between them.
    #[serde(skip)]
    pub others: std::collections::HashMap<String, std::collections::HashSet<String>>,
}

pub struct Imported {
    pub build: PobBuild,
    pub stages: Vec<SpecStage>,
    pub link: Option<String>,
    pub name: String,
    /// Where it came from: "Lifeline", "Path of Building" or "In-game planner".
    pub source: String,
}

/// Settings from older versions carry more fields (hotkeys, sounds, HUD);
/// they're ignored on load and dropped on the next save.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// How the player plays: "playstation", "xbox" or "keyboard".
    pub input: String,
    /// The first-run welcome (Claude Code setup, game, tour) is done.
    pub onboarded: bool,
    /// App text size: "auto" (by window size), "normal", "large", "xlarge".
    pub text_size: String,
    /// The My builds entry being followed, if the current build came from there.
    pub active_saved: Option<u64>,
}

/// How to name controls for this player, for the skill coach.
pub fn controls(input: &str) -> &'static str {
    match input {
        "playstation" => "PlayStation controller: name buttons Cross, Circle, Square, Triangle, L1, R1, L2, R2 (and combos like R2+Triangle).",
        "xbox" => "Xbox controller: name buttons A, B, X, Y, LB, RB, LT, RT (and combos like RT+Y).",
        _ => "Mouse and keyboard: name skill slots by their keys (left, middle and right click, Q, W, E, R, T) and say \"dodge roll\" rather than guessing a key.",
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            input: "keyboard".into(),
            onboarded: false,
            text_size: "auto".into(),
            active_saved: None,
        }
    }
}

impl Settings {
    pub fn load(dir: &Path) -> Self {
        read_json(&dir.join("settings.json")).unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        write_atomic(&dir.join("settings.json"), &text).map_err(|e| e.to_string())
    }
}

const SKILLS_FILE: &str = "skills.json";

/// Writes through a temporary file, so a crash mid-write never leaves a
/// half-written file behind.
pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Reads a JSON file. One that exists but can't be read is kept as
/// `name.bad` (not silently overwritten with defaults later).
pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str(&text) {
        Ok(v) => Some(v),
        Err(_) => {
            let _ = std::fs::copy(path, path.with_extension("bad"));
            None
        }
    }
}

pub struct AppState {
    pub data_dir: PathBuf,
    pub character: Mutex<Character>,
    pub imported: Mutex<Option<Imported>>,
    pub tree: Mutex<Option<Arc<PassiveTree>>>,
    pub game: Mutex<Option<Arc<lifeline_data::GameData>>>,
    /// Problems found at startup, shown once in the app.
    pub startup_problems: Mutex<Vec<String>>,
    /// Latest skill-coach plan (skills, supports, buttons, rotations).
    pub skills_plan: Mutex<Option<serde_json::Value>>,
    pub skills_busy: std::sync::atomic::AtomicBool,
    /// (port, bearer token) of the local MCP server.
    pub mcp: Mutex<Option<(u16, String)>>,
    pub settings: Mutex<Settings>,
    /// The latest design_build result (design, report) of the build being
    /// created, before it goes into the library.
    pub last_design: Mutex<Option<(lifeline_model::BuildDesign, lifeline_model::DesignReport)>>,
    /// A build is being created.
    pub build_busy: std::sync::atomic::AtomicBool,
    /// Steps shown while a build is created.
    pub build_steps: Mutex<Vec<String>>,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> Self {
        let settings = Settings::load(&data_dir);
        Self {
            character: Mutex::new(Character::default()),
            imported: Mutex::new(None),
            tree: Mutex::new(None),
            game: Mutex::new(None),
            startup_problems: Mutex::new(Vec::new()),
            skills_plan: Mutex::new(read_json(&data_dir.join(SKILLS_FILE))),
            skills_busy: std::sync::atomic::AtomicBool::new(false),
            mcp: Mutex::new(None),
            settings: Mutex::new(settings),
            last_design: Mutex::new(None),
            build_busy: std::sync::atomic::AtomicBool::new(false),
            build_steps: Mutex::new(Vec::new()),
            data_dir,
        }
    }

    /// Which My builds entry is being followed (None for any other source).
    pub fn set_active_saved(&self, id: Option<u64>) {
        let mut s = self.settings.lock().unwrap();
        s.active_saved = id;
        let _ = s.save(&self.data_dir);
    }

    pub fn save_skills(&self) {
        if let Some(p) = self.skills_plan.lock().unwrap().as_ref() {
            let _ = write_atomic(&self.data_dir.join(SKILLS_FILE), &p.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_from_an_older_version_still_load() {
        let old = r#"{"league":"HC Forbidden Rites","hotkeys":{"item_check":"Alt+Shift+D"},"overlay_visible":true,
                      "sound":true,"input":"playstation","onboarded":true,"text_size":"large","active_saved":7}"#;
        let s: Settings = serde_json::from_str(old).unwrap();
        assert_eq!((s.input.as_str(), s.onboarded, s.text_size.as_str(), s.active_saved), ("playstation", true, "large", Some(7)));
        let fresh: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(fresh, Settings::default());
    }
}
