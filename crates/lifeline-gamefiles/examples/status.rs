//! What the app can see of your PoE2 install today, from your real files.
//!
//! cargo run -p lifeline-gamefiles --example status              # snapshot
//! cargo run -p lifeline-gamefiles --example status -- --follow  # then stream events live while you play

use std::time::Duration;

use lifeline_gamefiles::client_log::{self, plain_text, EventKind, LogEvent, LogTailer};
use lifeline_gamefiles::config_ini::ConfigIni;
use lifeline_gamefiles::{build_planner, paths};

#[derive(Default)]
struct State {
    character: Option<String>,
    class: String,
    level: u32,
    area_id: String,
    area_level: u32,
    zone: String,
    deaths: u32,
    quest_points: u32,
    recent: Vec<String>,
}

/// Elemental resistance penalty by act / area level (PLAN.md §3.5, poe2wiki Resistance).
fn res_penalty(area_id: &str, area_level: u32) -> Option<i32> {
    if area_id.starts_with("Hideout") {
        return None;
    }
    match client_log::campaign_act(area_id) {
        Some(1) => Some(0),
        Some(2) => Some(-10),
        Some(3) => Some(-20),
        Some(4) => Some(-30),
        _ => match area_level {
            54..=59 => Some(-40),
            60..=64 => Some(-50),
            65.. => Some(-60),
            _ => None,
        },
    }
}

impl State {
    fn apply(&mut self, e: &LogEvent, live: bool) {
        let mine = |c: &str| self.character.as_deref() == Some(c);
        let note = match &e.kind {
            EventKind::LevelUp {
                character,
                class,
                level,
            } => {
                if self.character.as_deref() != Some(character) {
                    self.deaths = 0;
                    self.quest_points = 0;
                }
                self.character = Some(character.clone());
                self.class = class.clone();
                self.level = *level;
                Some(format!("{character} ({class}) reached level {level}"))
            }
            EventKind::AreaGenerated {
                area_id, area_level, ..
            } => {
                self.area_id = area_id.clone();
                self.area_level = *area_level;
                None
            }
            EventKind::SceneEntered { name } => {
                self.zone = name.clone();
                let act = client_log::campaign_act(&self.area_id).map_or(String::new(), |a| format!(", Act {a}"));
                let pen = res_penalty(&self.area_id, self.area_level)
                    .map_or(String::new(), |p| format!(" — resistance penalty {p}%"));
                Some(format!(
                    "Entered {name} ({}, area level {}{act}){pen}",
                    self.area_id, self.area_level
                ))
            }
            EventKind::Slain { character } if mine(character) => {
                self.deaths += 1;
                Some(format!(
                    "☠ {character} was slain in {} (area level {})",
                    self.zone, self.area_level
                ))
            }
            EventKind::PassiveAllocated { id, name } => Some(format!("Allocated passive {name} [{id}]")),
            EventKind::PassiveUnallocated { id, name } => Some(format!("Refunded passive {name} [{id}]")),
            EventKind::PassivePointsReceived { count, weapon_set } => {
                if !weapon_set {
                    self.quest_points += count;
                }
                Some(format!(
                    "Quest reward: {count} {}passive points",
                    if *weapon_set { "weapon set " } else { "" }
                ))
            }
            EventKind::PermanentBonus { bonus, lost, .. } => Some(format!(
                "Permanent buff {}: {}",
                if *lost { "removed" } else { "gained" },
                plain_text(bonus)
            )),
            EventKind::BuildPlannerLoaded { path } => Some(format!(
                "Game loaded planner build {}",
                path.rsplit('/').next().unwrap_or(path)
            )),
            _ => None,
        };
        if let Some(note) = note {
            if live {
                println!("[{}] {note}", e.timestamp);
            }
            self.recent.push(format!("[{}] {note}", e.timestamp));
            if self.recent.len() > 8 {
                self.recent.remove(0);
            }
        }
    }

    fn print(&self) {
        println!("\n== From Client.txt ==");
        match &self.character {
            Some(c) => println!("Last character: {c} ({}) level {}", self.class, self.level),
            None => println!("No level-up seen yet"),
        }
        let act = client_log::campaign_act(&self.area_id).map_or("not campaign".into(), |a| format!("Act {a}"));
        println!(
            "Last zone: {} ({}, area level {}, {act})",
            self.zone, self.area_id, self.area_level
        );
        if let Some(p) = res_penalty(&self.area_id, self.area_level) {
            println!("Resistance penalty there: {p}%");
        }
        println!("Deaths logged for this character: {}", self.deaths);
        println!("Recent events:");
        for r in &self.recent {
            println!("  {r}");
        }
    }
}

fn main() {
    let follow = std::env::args().any(|a| a == "--follow");
    let Some(user_dir) = paths::user_dir() else {
        eprintln!("Could not find your Documents folder");
        return;
    };
    println!("PoE2 folder: {}", user_dir.display());

    match std::fs::read_to_string(paths::config_file(&user_dir)) {
        Ok(text) => {
            let cfg = ConfigIni::parse(&text);
            println!("Selected item filter: {}", cfg.item_filter().unwrap_or("(none)"));
            for b in cfg.active_builds().unwrap_or_default() {
                println!(
                    "Active planner build for {}: {}",
                    b.character,
                    b.path.rsplit('/').next().unwrap_or(&b.path)
                );
            }
        }
        Err(e) => println!("Config not readable: {e}"),
    }

    match build_planner::read_dir(&paths::build_planner_dir(&user_dir)) {
        Ok(files) => {
            println!("\n== In-game Build Planner files ({}) ==", files.len());
            for (path, parsed) in files {
                match parsed {
                    Ok(b) => println!(
                        "  {:<42} ascendancy {:<10} passives {:>3}  skills {:>2}  gear slots {:>2}",
                        b.name,
                        b.ascendancy.as_deref().unwrap_or("-"),
                        b.shared_passive_ids().len(),
                        b.skills.len(),
                        b.inventory_slots.len()
                    ),
                    Err(e) => println!("  {}: could not parse ({e})", path.display()),
                }
            }
        }
        Err(e) => println!("Build Planner folder not readable: {e}"),
    }

    let Some(log) = paths::find_client_log() else {
        println!("\nClient.txt not found in the default Steam/standalone locations");
        return;
    };
    println!("\nGame log: {}", log.display());
    let mut state = State::default();
    let mut tailer = LogTailer::from_start(&log);
    match tailer.poll() {
        Ok(lines) => {
            for line in lines {
                if let Some(event) = client_log::parse_line(&line) {
                    state.apply(&event, false);
                }
            }
        }
        Err(e) => println!("Could not read log: {e}"),
    }
    state.print();

    if follow {
        println!("\n== Following live — play the game; Ctrl+C here to stop ==");
        loop {
            std::thread::sleep(Duration::from_millis(500));
            if let Ok(lines) = tailer.poll() {
                for line in lines {
                    if let Some(event) = client_log::parse_line(&line) {
                        state.apply(&event, true);
                    }
                }
            }
        }
    }
}
