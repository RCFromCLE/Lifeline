//! Follows `Client.txt` and keeps the character current: who it is, its
//! level, where it is in the campaign, and which passives it has taken.

use std::time::Duration;

use lifeline_gamefiles::client_log::{self, plain_text, EventKind, LogEvent, LogTailer};
use lifeline_gamefiles::paths;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::{AppState, Character};

/// Hideouts, trials and other side areas the player visits mid-campaign.
fn is_side_area(area_id: &str) -> bool {
    let id = area_id.to_ascii_lowercase();
    ["hideout", "trial", "sanctum", "ultimatum", "chaos", "labyrinth", "menagerie", "tutorial"]
        .iter()
        .any(|k| id.contains(k))
}

/// Applies one event; true when the character changed.
fn apply(c: &mut Character, e: &LogEvent) -> bool {
    match &e.kind {
        EventKind::LevelUp { character, class, level } => {
            if c.name.as_deref() != Some(character.as_str()) {
                // Switching characters: keep each one's allocated passives.
                let mut others = std::mem::take(&mut c.others);
                if let Some(old) = c.name.take() {
                    others.insert(old, std::mem::take(&mut c.allocated));
                }
                let allocated = others.remove(character).unwrap_or_default();
                *c = Character {
                    zone: c.zone.clone(),
                    area_id: c.area_id.clone(),
                    area_level: c.area_level,
                    act: c.act,
                    allocated,
                    others,
                    ..Character::default()
                };
                c.name = Some(character.clone());
            }
            c.class = class.clone();
            c.level = *level;
        }
        EventKind::AreaGenerated { area_id, area_level, .. } => {
            c.area_id = area_id.clone();
            c.area_level = *area_level;
            // Hideouts and trials aren't on the campaign map: stay in the act
            // the character was in, so the plan doesn't jump to Interludes.
            match client_log::campaign_act(area_id) {
                Some(act) => c.act = Some(act),
                None if is_side_area(area_id) => {}
                None => c.act = None,
            }
        }
        EventKind::SceneEntered { name } => c.zone = name.clone(),
        EventKind::PassiveAllocated { id, .. } => {
            c.allocated.insert(id.clone());
        }
        EventKind::PassiveUnallocated { id, .. } => {
            c.allocated.remove(id);
        }
        _ => return false,
    }
    true
}

/// A passive was just taken in game: when it's on the plan, the chime plays
/// and the overlay shows it as done.
fn passive_taken(app: &AppHandle, state: &AppState, id: &str, name: &str) {
    if !crate::alignment::on_plan(state, id) {
        return;
    }
    if state.settings.lock().unwrap().passive_sound {
        crate::chime::play();
    }
    let _ = app.emit("passive-taken", plain_text(name));
}

pub fn spawn_log_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        // Keep looking (every 15 s) in case the game is installed or moved later.
        let mut warned = false;
        let path = loop {
            if let Some(p) = paths::find_client_log() {
                break p;
            }
            if !warned {
                warned = true;
                let _ = app.emit("notice", "Game log not found yet. Start PoE2 once; still looking.");
            }
            std::thread::sleep(Duration::from_secs(15));
        };
        let mut tailer = LogTailer::from_start(&path);
        let mut backfilled = false;
        crate::debug_log(&app.state::<AppState>(), &format!("log watcher started on {}", path.display()));
        loop {
            let started = std::time::Instant::now();
            if let Ok(lines) = tailer.poll() {
                let state = app.state::<AppState>();
                let mut changed = false;
                for line in lines {
                    if let Some(event) = client_log::parse_line(&line) {
                        changed |= apply(&mut state.character.lock().unwrap(), &event);
                        // Live play only: the first read of the log is history.
                        if let (true, EventKind::PassiveAllocated { id, name }) = (backfilled, &event.kind) {
                            passive_taken(&app, &state, id, name);
                        }
                    }
                }
                if changed || !backfilled {
                    let character = state.character.lock().unwrap().clone();
                    let _ = app.emit("character", &character);
                }
                if !backfilled {
                    let c = state.character.lock().unwrap().clone();
                    crate::debug_log(
                        &state,
                        &format!("log read in {:?}: {:?} level {} in {}", started.elapsed(), c.name, c.level, c.zone),
                    );
                }
                backfilled = true;
            }
            std::thread::sleep(Duration::from_millis(400));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(kind: EventKind) -> LogEvent {
        LogEvent { timestamp: String::new(), kind }
    }

    fn area(id: &str, level: u32) -> LogEvent {
        ev(EventKind::AreaGenerated { area_id: id.into(), area_level: level, seed: 1 })
    }

    #[test]
    fn hideouts_keep_the_act_and_alts_keep_their_passives() {
        let mut c = Character::default();
        apply(&mut c, &area("G2_3", 20));
        assert_eq!(c.act, Some(2));
        apply(&mut c, &area("HideoutFelled", 20));
        assert_eq!(c.act, Some(2), "a hideout isn't the Interludes");
        apply(&mut c, &area("MapUberBoss", 75));
        assert_eq!(c.act, None);

        let level = |name: &str, level: u32| ev(EventKind::LevelUp { character: name.into(), class: "Huntress".into(), level });
        apply(&mut c, &level("Main", 20));
        apply(&mut c, &ev(EventKind::PassiveAllocated { id: "evasion11".into(), name: "Evasion".into() }));
        apply(&mut c, &level("Alt", 2));
        assert!(c.allocated.is_empty());
        apply(&mut c, &level("Main", 21));
        assert!(c.allocated.contains("evasion11"), "back on the main, its passives are restored");
    }
}
