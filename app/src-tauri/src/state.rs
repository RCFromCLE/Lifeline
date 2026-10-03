//! Shared app state and the persisted settings.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use lifeline_data::PassiveTree;
use lifeline_model::SpecStage;
use lifeline_pob::PobBuild;
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
    /// Other characters' allocations, kept while you switch between them.
    #[serde(skip)]
    pub others: std::collections::HashMap<String, std::collections::HashSet<String>>,
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
    /// Where it came from: "Lifeline", "Path of Building" or "In-game planner".
    pub source: String,
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
    /// Whether the HUD was on when last toggled; restored at startup.
    #[serde(default = "default_true")]
    pub overlay_visible: bool,
    /// Max price per recommended item, e.g. "10 exalted".
    #[serde(default = "default_budget")]
    pub rating_budget: String,
    /// Re-rate automatically when entering a new act (off: the player triggers it).
    #[serde(default)]
    pub auto_rate_on_act: bool,
    /// Sound cues during play (level up, new act, boss areas, death…).
    #[serde(default = "default_true")]
    pub sound: bool,
    #[serde(default = "default_volume")]
    pub volume: f32,
    /// Each cue on or off ("level_up", "new_act", "boss_area", "penalty",
    /// "death", "ready"). Level up and death are off unless chosen.
    #[serde(default = "default_cues")]
    pub sound_cues: std::collections::BTreeMap<String, bool>,
    /// How the player plays: "playstation", "xbox" or "keyboard".
    #[serde(default = "default_input")]
    pub input: String,
    /// The first-run welcome (Claude Code setup, game, tour) is done.
    #[serde(default)]
    pub onboarded: bool,
    /// App text size: "auto" (by window size), "normal", "large", "xlarge".
    #[serde(default = "default_text_size")]
    pub text_size: String,
    /// The My builds entry being followed, if the current build came from there.
    #[serde(default)]
    pub active_saved: Option<u64>,
}

fn default_text_size() -> String {
    "auto".into()
}

fn default_input() -> String {
    "keyboard".into()
}

/// How to name controls for this player, for the AI.
pub fn controls(input: &str) -> &'static str {
    match input {
        "playstation" => "PlayStation controller: name buttons Cross, Circle, Square, Triangle, L1, R1, L2, R2 (and combos like R2+Triangle).",
        "xbox" => "Xbox controller: name buttons A, B, X, Y, LB, RB, LT, RT (and combos like RT+Y).",
        _ => "Mouse and keyboard: name skill slots by their keys (left, middle and right click, Q, W, E, R, T) and say \"dodge roll\" rather than guessing a key.",
    }
}

fn default_volume() -> f32 {
    0.6
}

pub fn default_cues() -> std::collections::BTreeMap<String, bool> {
    [("level_up", false), ("new_act", true), ("boss_area", true), ("penalty", true), ("death", false), ("ready", true)]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect()
}

fn default_true() -> bool {
    true
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
            overlay_visible: true,
            rating_budget: default_budget(),
            auto_rate_on_act: false,
            sound: true,
            volume: default_volume(),
            sound_cues: default_cues(),
            input: default_input(),
            text_size: default_text_size(),
            onboarded: false,
            active_saved: None,
            overlay_on_hotkey: true,
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

/// Item class of a copied item ("Item Class: Rings" → "Rings").
pub fn item_class(item: &str) -> String {
    item.lines()
        .next()
        .and_then(|l| l.strip_prefix("Item Class:"))
        .map(|c| c.trim().to_owned())
        .unwrap_or_else(|| "Unknown".into())
}

/// Second ring slot key (the first is "Rings").
pub const SECOND_RING: &str = "Rings (2)";

/// Item classes a character wears (everything else — flasks, charms,
/// gems, currency, jewels — isn't gear for a slot).
pub const ARMOUR_CLASSES: [&str; 7] = ["Helmets", "Body Armours", "Gloves", "Boots", "Amulets", "Rings", "Belts"];
pub const OFFHAND_CLASSES: [&str; 4] = ["Shields", "Bucklers", "Quivers", "Foci"];
pub const WEAPON_CLASSES: [&str; 18] = [
    "Bows", "Crossbows", "Spears", "Quarterstaves", "One Hand Maces", "Two Hand Maces", "Wands", "Staves", "Sceptres",
    "Talismans", "Flails", "Daggers", "Claws", "One Hand Swords", "Two Hand Swords", "One Hand Axes", "Two Hand Axes",
    "Warstaves",
];

pub fn is_worn_class(class: &str) -> bool {
    ARMOUR_CLASSES.contains(&class) || OFFHAND_CLASSES.contains(&class) || WEAPON_CLASSES.contains(&class) || class == SECOND_RING
}

/// Name and base of a copied item, ignoring how it was copied (Ctrl+C or
/// the advanced Ctrl+Alt+C with its extra "{ … }" lines).
fn item_identity(item: &str) -> String {
    let lines: Vec<&str> = item
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('{') && !l.starts_with("--------"))
        .collect();
    let start = lines.iter().position(|l| l.starts_with("Rarity:")).map_or(0, |i| i + 1);
    lines.iter().skip(start).take(2).copied().collect::<Vec<_>>().join("|")
}

/// Stores a copied item as worn, keyed by item class. Rings fill two slots:
/// a new ring goes in "Rings" and the one there moves to "Rings (2)", so the
/// two most recent different rings are kept. Returns the slot name, or why
/// the item isn't gear.
pub fn record_equipped(equipped: &mut std::collections::BTreeMap<String, String>, item: &str) -> Result<String, String> {
    let class = item_class(item);
    if !is_worn_class(&class) {
        return Err(format!("That's not worn gear ({class}). Copy a weapon, armour piece, ring, amulet or belt."));
    }
    if class == "Rings" {
        let same = |slot: &str| equipped.get(slot).is_some_and(|t| item_identity(t) == item_identity(item));
        if same("Rings") || same(SECOND_RING) {
            let slot = if same("Rings") { "Rings" } else { SECOND_RING };
            equipped.insert(slot.into(), item.to_owned());
            return Ok("ring (updated)".into());
        }
        if let Some(old) = equipped.insert("Rings".into(), item.to_owned()) {
            equipped.insert(SECOND_RING.into(), old);
            return Ok("Ring 1 (previous ring is now Ring 2)".into());
        }
        return Ok("Ring 1".into());
    }
    equipped.insert(class.clone(), item.to_owned());
    Ok(class)
}

fn load_equipped(dir: &Path) -> std::collections::BTreeMap<String, String> {
    std::fs::read_to_string(dir.join(EQUIPPED_FILE))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

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

fn load_conversations(dir: &Path) -> Vec<Conversation> {
    let mut convs: Vec<Conversation> = read_json(&dir.join(CONVERSATIONS_FILE)).unwrap_or_default();
    // Older chats stored a button's full instructions as the question.
    for m in convs.iter_mut().flat_map(|c| c.messages.iter_mut()) {
        if m.role == "user" {
            let short = crate::ai::shown(&m.text);
            if short.len() != m.text.len() {
                m.text = short.to_owned();
            }
        }
    }
    convs
}

impl AppState {
    /// Which My builds entry is being followed (None for any other source).
    pub fn set_active_saved(&self, id: Option<u64>) {
        let mut s = self.settings.lock().unwrap();
        s.active_saved = id;
        let _ = s.save(&self.data_dir);
    }

    pub fn save_conversations(&self) {
        let convs = self.conversations.lock().unwrap().clone();
        if let Ok(text) = serde_json::to_string_pretty(&convs) {
            let _ = write_atomic(&self.data_dir.join(CONVERSATIONS_FILE), &text);
        }
    }

    pub fn save_rating(&self) {
        let v = serde_json::json!({
            "rating": self.rating.lock().unwrap().clone(),
            "cards": self.rating_cards.lock().unwrap().clone(),
        });
        let _ = write_atomic(&self.data_dir.join(RATING_FILE), &v.to_string());
    }

    pub fn save_skills(&self) {
        if let Some(p) = self.skills_plan.lock().unwrap().as_ref() {
            let _ = write_atomic(&self.data_dir.join(SKILLS_FILE), &p.to_string());
        }
    }

    pub fn save_equipped(&self) {
        let e = self.equipped.lock().unwrap().clone();
        if let Ok(text) = serde_json::to_string_pretty(&e) {
            let _ = write_atomic(&self.data_dir.join(EQUIPPED_FILE), &text);
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
    pub game: Mutex<Option<Arc<lifeline_data::GameData>>>,
    pub conversations: Mutex<Vec<Conversation>>,
    pub actions: Mutex<Vec<PendingAction>>,
    pub market: lifeline_trade::Market,
    /// listing id → search id, so travel can re-fetch the listing.
    pub listings: Mutex<std::collections::HashMap<String, String>>,
    /// Full listing data from recent searches (for image cards).
    pub listing_data: Mutex<std::collections::HashMap<String, lifeline_trade::Listing>>,
    /// A Travel pressed while the trade window wasn't on pathofexile.com
    /// (still loading, or the player signing in): (listing id, search id).
    pub pending_travel: Mutex<Option<(String, String)>>,
    /// Ctrl+C mode: record items copied in game until this time.
    pub gear_watch_until: Mutex<Option<std::time::Instant>>,
    pub gear_watch_running: std::sync::atomic::AtomicBool,
    /// Problems found at startup, shown once in the app.
    pub startup_problems: Mutex<Vec<String>>,
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
    /// The wizard's latest design_build result (design, report), before it
    /// goes into the library.
    pub last_design: Mutex<Option<(lifeline_model::BuildDesign, lifeline_model::DesignReport)>>,
    /// A wizard build is being generated.
    pub build_busy: std::sync::atomic::AtomicBool,
    /// Steps shown while a wizard build is generated.
    pub build_steps: Mutex<Vec<String>>,
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
            market: lifeline_trade::Market::new(),
            listings: Mutex::new(Default::default()),
            listing_data: Mutex::new(Default::default()),
            pending_travel: Mutex::new(None),
            gear_watch_until: Mutex::new(None),
            gear_watch_running: std::sync::atomic::AtomicBool::new(false),
            startup_problems: Mutex::new(Vec::new()),
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
            last_design: Mutex::new(None),
            build_busy: std::sync::atomic::AtomicBool::new(false),
            build_steps: Mutex::new(Vec::new()),
            data_dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_rings_are_kept_and_only_gear_is_recorded() {
        let mut worn = std::collections::BTreeMap::new();
        let ring = |n: &str| format!("Item Class: Rings\nRarity: Rare\n{n}\nRuby Ring");
        assert_eq!(record_equipped(&mut worn, &ring("Blood Loop")).unwrap(), "Ring 1");
        record_equipped(&mut worn, &ring("Storm Band")).unwrap();
        assert!(worn["Rings"].contains("Storm Band") && worn[SECOND_RING].contains("Blood Loop"));
        // The same ring copied with Ctrl+Alt+C (extra "{ }" lines) is the same ring.
        let advanced = format!("{}\n{{ Prefix Modifier }}\n+20 to maximum Life", ring("Storm Band"));
        record_equipped(&mut worn, &advanced).unwrap();
        assert!(worn[SECOND_RING].contains("Blood Loop"), "re-recording the same ring keeps the other");
        let boots = "Item Class: Boots\nRarity: Rare\nGale Stride\nLeather Shoes";
        assert_eq!(record_equipped(&mut worn, boots).unwrap(), "Boots");
        assert!(record_equipped(&mut worn, "Item Class: Life Flasks\nRarity: Magic\nLesser Life Flask").is_err());
        assert!(record_equipped(&mut worn, "Item Class: Stackable Currency\nRarity: Currency\nExalted Orb").is_err());
        assert_eq!(worn.len(), 3);
    }
}
