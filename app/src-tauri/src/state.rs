//! Shared app state and the persisted settings.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use polr_data::PassiveTree;
use polr_model::SpecStage;
use polr_pob::PobBuild;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize)]
pub struct Character {
    pub name: Option<String>,
    pub class: String,
    pub level: u32,
    pub zone: String,
    pub area_id: String,
    pub area_level: u32,
    pub act: Option<u8>,
    /// Elemental resistance penalty where the character is (PLAN.md §3.5).
    pub res_penalty: Option<i32>,
    pub deaths: u32,
    pub quest_points: u32,
    pub buffs: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeedItem {
    pub time: String,
    pub kind: String,
    pub text: String,
}

pub struct Imported {
    pub build: PobBuild,
    pub stages: Vec<SpecStage>,
    pub link: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Hotkeys {
    pub item_check: String,
    pub what_next: String,
    pub ask: String,
    pub toggle_overlay: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub league: String,
    pub hotkeys: Hotkeys,
    /// Show the HUD overlay automatically when a hotkey answer arrives.
    pub overlay_on_hotkey: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            league: "HC Forbidden Rites".into(),
            hotkeys: Hotkeys {
                item_check: "Alt+Shift+D".into(),
                what_next: "Alt+Shift+N".into(),
                ask: "Alt+Shift+A".into(),
                toggle_overlay: "Alt+Shift+O".into(),
            },
            overlay_on_hotkey: true,
        }
    }
}

impl Settings {
    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(dir.join("settings.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("settings.json"), text).map_err(|e| e.to_string())
    }
}

pub struct AppState {
    pub data_dir: PathBuf,
    pub character: Mutex<Character>,
    pub feed: Mutex<Vec<FeedItem>>,
    pub imported: Mutex<Option<Imported>>,
    pub tree: Mutex<Option<Arc<PassiveTree>>>,
    pub ai_session: Mutex<Option<String>>,
    pub ai_busy: AtomicBool,
    pub settings: Mutex<Settings>,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> Self {
        let settings = Settings::load(&data_dir);
        Self {
            data_dir,
            character: Mutex::new(Character::default()),
            feed: Mutex::new(Vec::new()),
            imported: Mutex::new(None),
            tree: Mutex::new(None),
            ai_session: Mutex::new(None),
            ai_busy: AtomicBool::new(false),
            settings: Mutex::new(settings),
        }
    }
}
