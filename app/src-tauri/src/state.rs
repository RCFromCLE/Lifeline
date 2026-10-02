//! Shared app state and the persisted settings.

use std::path::{Path, PathBuf};
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
    /// Passive ids allocated this character (from the log).
    #[serde(skip)]
    pub allocated: std::collections::HashSet<String>,
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
    /// Record the hovered item as what you have equipped in its slot.
    #[serde(default = "default_record_equipped")]
    pub record_equipped: String,
    /// Unlock the HUD to drag it; press again to lock it in place.
    #[serde(default = "default_move_overlay")]
    pub move_overlay: String,
}

fn default_record_equipped() -> String {
    "Alt+Shift+E".into()
}

fn default_move_overlay() -> String {
    "Alt+Shift+M".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub league: String,
    pub hotkeys: Hotkeys,
    /// Show the HUD overlay automatically when a hotkey answer arrives.
    pub overlay_on_hotkey: bool,
    /// Saved HUD position (logical px).
    #[serde(default)]
    pub overlay_pos: Option<(f64, f64)>,
    /// Max price per recommended item, e.g. "10 exalted".
    #[serde(default = "default_budget")]
    pub rating_budget: String,
    /// Re-rate automatically when entering a new act (off: the player triggers it).
    #[serde(default)]
    pub auto_rate_on_act: bool,
}

fn default_budget() -> String {
    "10 exalted".into()
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
                record_equipped: default_record_equipped(),
                move_overlay: default_move_overlay(),
            },
            overlay_pos: None,
            rating_budget: default_budget(),
            auto_rate_on_act: false,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// `user` or `ai`.
    pub role: String,
    pub label: String,
    pub text: String,
    #[serde(default)]
    pub error: bool,
}

/// One chat thread with its own Claude session. Several can run at once;
/// each turn is told what the others are about (see `ai::awareness`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: u64,
    pub title: String,
    /// Claude Code session to `--resume`.
    pub session: Option<String>,
    pub messages: Vec<Message>,
    /// The thread hotkey answers go to.
    #[serde(default)]
    pub in_game: bool,
    #[serde(skip)]
    pub busy: bool,
}

const CONVERSATIONS_FILE: &str = "conversations.json";
const EQUIPPED_FILE: &str = "equipped.json";
const RATING_FILE: &str = "rating.json";
const SKILLS_FILE: &str = "skills.json";

fn load_json<T: serde::de::DeserializeOwned>(dir: &Path, file: &str) -> Option<T> {
    std::fs::read_to_string(dir.join(file))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
}

fn load_equipped(dir: &Path) -> std::collections::BTreeMap<String, String> {
    std::fs::read_to_string(dir.join(EQUIPPED_FILE))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn load_conversations(dir: &Path) -> Vec<Conversation> {
    std::fs::read_to_string(dir.join(CONVERSATIONS_FILE))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

impl AppState {
    pub fn save_conversations(&self) {
        let convs = self.conversations.lock().unwrap().clone();
        if let Ok(text) = serde_json::to_string_pretty(&convs) {
            let _ = std::fs::write(self.data_dir.join(CONVERSATIONS_FILE), text);
        }
    }

    pub fn save_rating(&self) {
        let v = serde_json::json!({
            "rating": self.rating.lock().unwrap().clone(),
            "cards": self.rating_cards.lock().unwrap().clone(),
        });
        let _ = std::fs::write(self.data_dir.join(RATING_FILE), v.to_string());
    }

    pub fn save_skills(&self) {
        if let Some(p) = self.skills_plan.lock().unwrap().as_ref() {
            let _ = std::fs::write(self.data_dir.join(SKILLS_FILE), p.to_string());
        }
    }

    pub fn save_equipped(&self) {
        let e = self.equipped.lock().unwrap().clone();
        if let Ok(text) = serde_json::to_string_pretty(&e) {
            let _ = std::fs::write(self.data_dir.join(EQUIPPED_FILE), text);
        }
    }

    /// Creates a conversation and returns its id.
    pub fn new_conversation(&self, title: &str, in_game: bool) -> u64 {
        let mut convs = self.conversations.lock().unwrap();
        let id = convs.iter().map(|c| c.id).max().unwrap_or(0) + 1;
        convs.push(Conversation {
            id,
            title: title.to_owned(),
            session: None,
            messages: Vec::new(),
            in_game,
            busy: false,
        });
        id
    }

    /// The pinned "In-game" thread for hotkey answers, created on demand.
    pub fn in_game_conversation(&self) -> u64 {
        let existing = self
            .conversations
            .lock()
            .unwrap()
            .iter()
            .find(|c| c.in_game)
            .map(|c| c.id);
        existing.unwrap_or_else(|| self.new_conversation("In-game (hotkeys)", true))
    }
}

/// An action the AI proposed; nothing happens until the player confirms.
#[derive(Debug, Clone, Serialize)]
pub struct PendingAction {
    pub id: u64,
    pub conv: u64,
    /// `travel` or `open_search`.
    pub kind: String,
    pub summary: String,
    pub listing_id: Option<String>,
    pub search_id: Option<String>,
}

pub struct AppState {
    pub data_dir: PathBuf,
    pub character: Mutex<Character>,
    pub feed: Mutex<Vec<FeedItem>>,
    pub imported: Mutex<Option<Imported>>,
    pub tree: Mutex<Option<Arc<PassiveTree>>>,
    pub game: Mutex<Option<Arc<polr_data::GameData>>>,
    pub conversations: Mutex<Vec<Conversation>>,
    pub actions: Mutex<Vec<PendingAction>>,
    pub market: polr_trade::Market,
    /// listing id → search id, so travel can re-fetch the listing.
    pub listings: Mutex<std::collections::HashMap<String, String>>,
    /// Full listing data from recent searches (for image cards).
    pub listing_data: Mutex<std::collections::HashMap<String, polr_trade::Listing>>,
    /// Item text the player recorded as equipped, by item class ("Boots").
    pub equipped: Mutex<std::collections::BTreeMap<String, String>>,
    pub overlay_unlocked: std::sync::atomic::AtomicBool,
    /// Latest build rating (JSON) and market cards per recommendation index.
    pub rating: Mutex<Option<serde_json::Value>>,
    pub rating_cards: Mutex<std::collections::BTreeMap<usize, serde_json::Value>>,
    pub rating_busy: std::sync::atomic::AtomicBool,
    /// Clickable areas of the HUD (CSS px: left, top, right, bottom); the rest is click-through.
    pub overlay_regions: Mutex<Vec<[f64; 4]>>,
    /// Latest skill-coach plan (skills, supports, buttons, rotations).
    pub skills_plan: Mutex<Option<serde_json::Value>>,
    pub skills_busy: std::sync::atomic::AtomicBool,
    /// (port, bearer token) of the local MCP server.
    pub mcp: Mutex<Option<(u16, String)>>,
    pub settings: Mutex<Settings>,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> Self {
        let settings = Settings::load(&data_dir);
        Self {
            character: Mutex::new(Character::default()),
            feed: Mutex::new(Vec::new()),
            imported: Mutex::new(None),
            tree: Mutex::new(None),
            game: Mutex::new(None),
            conversations: Mutex::new(load_conversations(&data_dir)),
            actions: Mutex::new(Vec::new()),
            market: polr_trade::Market::new(),
            listings: Mutex::new(Default::default()),
            listing_data: Mutex::new(Default::default()),
            equipped: Mutex::new(load_equipped(&data_dir)),
            overlay_unlocked: std::sync::atomic::AtomicBool::new(false),
            rating: Mutex::new(
                load_json(&data_dir, RATING_FILE).and_then(|v: serde_json::Value| v.get("rating").cloned()),
            ),
            rating_cards: Mutex::new(
                load_json(&data_dir, RATING_FILE)
                    .and_then(|v: serde_json::Value| serde_json::from_value(v["cards"].clone()).ok())
                    .unwrap_or_default(),
            ),
            rating_busy: std::sync::atomic::AtomicBool::new(false),
            overlay_regions: Mutex::new(Vec::new()),
            skills_plan: Mutex::new(load_json(&data_dir, SKILLS_FILE)),
            skills_busy: std::sync::atomic::AtomicBool::new(false),
            mcp: Mutex::new(None),
            settings: Mutex::new(settings),
            data_dir,
        }
    }
}
