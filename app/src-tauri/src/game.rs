//! Follows `Client.txt` and keeps the character card current.

use std::time::Duration;

use lifeline_gamefiles::client_log::{self, plain_text, EventKind, LogEvent, LogTailer};
use lifeline_gamefiles::paths;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::{AppState, Character, FeedItem};

const FEED_LIMIT: usize = 120;

/// Elemental resistance penalty by act / area level (PLAN.md §3.5).
pub fn res_penalty(area_id: &str, area_level: u32) -> Option<i32> {
    if area_id.is_empty() || area_id.starts_with("Hideout") {
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

/// Applies one event; returns a feed line when it's worth showing.
fn apply(c: &mut Character, e: &LogEvent) -> Option<(String, String)> {
    let mine = c.name.clone();
    let is_mine = |name: &str| mine.as_deref() == Some(name);
    match &e.kind {
        EventKind::LevelUp {
            character,
            class,
            level,
        } => {
            if !is_mine(character) {
                *c = Character {
                    zone: c.zone.clone(),
                    area_id: c.area_id.clone(),
                    area_level: c.area_level,
                    act: c.act,
                    res_penalty: c.res_penalty,
                    ..Character::default()
                };
                c.name = Some(character.clone());
            }
            c.class = class.clone();
            c.level = *level;
            Some(("level".into(), format!("{character} reached level {level}")))
        }
        EventKind::AreaGenerated {
            area_id, area_level, ..
        } => {
            c.area_id = area_id.clone();
            c.area_level = *area_level;
            c.act = client_log::campaign_act(area_id);
            c.res_penalty = res_penalty(area_id, *area_level);
            None
        }
        EventKind::SceneEntered { name } => {
            c.zone = name.clone();
            let act = c.act.map_or(String::new(), |a| format!(" · Act {a}"));
            let pen = c.res_penalty.map_or(String::new(), |p| format!(" · res {p}%"));
            Some(("zone".into(), format!("{name} (area {}){act}{pen}", c.area_level)))
        }
        EventKind::Slain { character } if is_mine(character) => {
            c.deaths += 1;
            Some(("death".into(), format!("{character} was slain in {}", c.zone)))
        }
        EventKind::PassiveAllocated { id, name } => {
            c.allocated.insert(id.clone());
            Some(("passive".into(), format!("Allocated {name}")))
        }
        EventKind::PassiveUnallocated { id, name } => {
            c.allocated.remove(id);
            Some(("passive".into(), format!("Refunded {name}")))
        }
        EventKind::PassivePointsReceived { count, weapon_set } => {
            if !weapon_set {
                c.quest_points += count;
            }
            let kind = if *weapon_set { "weapon set " } else { "" };
            Some(("reward".into(), format!("Quest reward: {count} {kind}passive points")))
        }
        EventKind::PermanentBonus { bonus, lost, .. } => {
            let text = plain_text(bonus);
            if *lost {
                c.buffs.retain(|b| b != &text);
            } else {
                c.buffs.push(text.clone());
            }
            Some((
                "reward".into(),
                format!("Permanent buff {}: {text}", if *lost { "removed" } else { "gained" }),
            ))
        }
        EventKind::BuildPlannerLoaded { path } => Some((
            "planner".into(),
            format!("Game loaded planner build {}", path.rsplit('/').next().unwrap_or(path)),
        )),
        _ => None,
    }
}

/// Sound cues for what just changed (live play only, never the backfill).
fn cues(state: &AppState, before: &Character, after: &Character, e: &LogEvent) -> Vec<crate::sound::Cue> {
    use crate::sound::Cue;
    let mut out = Vec::new();
    if after.deaths > before.deaths && before.name == after.name {
        out.push(Cue::Death);
    }
    if after.level > before.level && before.name == after.name {
        out.push(Cue::LevelUp);
    }
    if let EventKind::AreaGenerated { area_id, .. } = &e.kind {
        if let (Some(a), Some(b)) = (after.act, before.act) {
            if a > b {
                out.push(Cue::NewAct);
            }
        }
        if let (Some(a), Some(b)) = (after.res_penalty, before.res_penalty) {
            if a < b {
                out.push(Cue::Penalty);
            }
        }
        let boss_area = after.area_id != before.area_id
            && state.game.lock().unwrap().as_ref().is_some_and(|d| {
                d.areas.iter().any(|x| &x.id == area_id && !x.is_town && !x.bosses.is_empty())
            });
        if boss_area {
            out.push(Cue::BossArea);
        }
    }
    out
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
        let mut last_rated_act: Option<u8> = None;
        crate::debug_log(&app.state::<AppState>(), &format!("log watcher started on {}", path.display()));
        loop {
            let started = std::time::Instant::now();
            let polled = tailer.poll();
            if !backfilled {
                let state = app.state::<AppState>();
                match &polled {
                    Ok(lines) => crate::debug_log(&state, &format!("backfill read {} lines in {:?}", lines.len(), started.elapsed())),
                    Err(e) => crate::debug_log(&state, &format!("backfill read failed: {e}")),
                }
            }
            if let Ok(lines) = polled {
                let state = app.state::<AppState>();
                let mut changed = false;
                for line in lines {
                    let Some(event) = client_log::parse_line(&line) else {
                        continue;
                    };
                    let before = state.character.lock().unwrap().clone();
                    let note = apply(&mut state.character.lock().unwrap(), &event);
                    if backfilled {
                        let after = state.character.lock().unwrap().clone();
                        for cue in cues(&state, &before, &after, &event) {
                            crate::sound::play(&app, cue);
                        }
                    }
                    let new_act = state.character.lock().unwrap().act;
                    // Re-rate once per new act while playing (not during backfill).
                    if backfilled
                        && new_act.is_some()
                        && new_act != last_rated_act
                        && state.settings.lock().unwrap().auto_rate_on_act
                    {
                        last_rated_act = new_act;
                        crate::rating::run(app.clone());
                    }
                    changed = true;
                    if let Some((kind, text)) = note {
                        let item = FeedItem {
                            time: event.timestamp.clone(),
                            kind,
                            text,
                        };
                        let mut feed = state.feed.lock().unwrap();
                        feed.push(item.clone());
                        if feed.len() > FEED_LIMIT {
                            let excess = feed.len() - FEED_LIMIT;
                            feed.drain(..excess);
                        }
                        if backfilled {
                            let _ = app.emit("feed", &item);
                        }
                    }
                }
                if changed || !backfilled {
                    let character = state.character.lock().unwrap().clone();
                    let _ = app.emit("character", &character);
                }
                if !backfilled {
                    let c = state.character.lock().unwrap().clone();
                    last_rated_act = c.act;
                    crate::debug_log(
                        &state,
                        &format!("backfill applied in {:?}: {:?} level {} in {}", started.elapsed(), c.name, c.level, c.zone),
                    );
                }
                backfilled = true;
            }
            std::thread::sleep(Duration::from_millis(400));
        }
    });
}
