//! Build rating (F … S+ for the current stage) plus market picks for its
//! recommendations. The rating runs on Opus 5.5 (build-rater); each market
//! search runs on Sonnet 5.5 (market-scout) within the player's budget.

use std::sync::atomic::Ordering;

use lifeline_ai::agents::{MARKET_MODEL, MARKET_SCOUT};
use lifeline_ai::{parse_rating, CliEvent, Job};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

/// MCP scopes for rating runs (conversation ids are small numbers).
pub const RATING_SCOPE: u64 = 900_000;
pub const MARKET_SCOPE_BASE: u64 = 900_100;
pub const MARKET_PICKS: usize = 3;

pub fn snapshot(state: &AppState) -> Value {
    let settings = state.settings.lock().unwrap().clone();
    json!({
        "rating": state.rating.lock().unwrap().clone(),
        "cards": state.rating_cards.lock().unwrap().clone(),
        "busy": state.rating_busy.load(Ordering::SeqCst),
        "budget": settings.rating_budget,
        "auto": settings.auto_rate_on_act,
    })
}

fn usage(app: &AppHandle, e: &CliEvent) {
    if let CliEvent::RateLimit(info) = e {
        let windows: Vec<_> = info
            .windows
            .iter()
            .map(|w| json!({"name": w.name, "pct": (w.utilization * 100.0).round(), "resets_at": w.resets_at}))
            .collect();
        let _ = app.emit("usage", json!({"status": info.status, "windows": windows}));
    }
}

/// Rates the build, then shops for the top recommendations. One run at a time.
pub fn run(app: AppHandle) {
    let state = app.state::<AppState>();
    if state.rating_busy.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = app.emit(
        "rating-status",
        json!({"busy": true, "text": "Rating…"}),
    );
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let outcome = rate(&app, &state);
        match outcome {
            Ok(()) => shop(&app, &state),
            Err(e) => {
                let _ = app.emit(
                    "rating-status",
                    json!({"busy": false, "text": format!("Rating failed: {e}")}),
                );
            }
        }
        state.rating_busy.store(false, Ordering::SeqCst);
        crate::sound::play(&app, crate::sound::Cue::Ready);
        state.save_rating();
        let _ = app.emit("rating", snapshot(&state));
    });
}

fn rate(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let c = state.character.lock().unwrap().clone();
    let context = json!({
        "character": c.name, "class": c.class, "level": c.level, "act": c.act, "area_level": c.area_level,
        "zone": c.zone, "resistance_penalty": c.res_penalty, "league": state.settings.lock().unwrap().league,
    });
    let cli = crate::ai::companion(state, RATING_SCOPE)?.for_job(Job::Rating);
    let result = cli
        .run_turn(&Job::Rating.prompt(&context.to_string()), None, |e| usage(app, e))
        .map_err(|e| e.to_string())?;
    let mut rating = parse_rating(&result.result).ok_or("the rater didn't return a rating")?;
    // Hard limits: what can't be checked can't be good.
    let slots = state.equipped.lock().unwrap().keys().filter(|k| crate::state::is_worn_class(k)).count();
    if slots == 0 {
        lifeline_ai::cap_grade(&mut rating, "D+", "no gear recorded, so nothing about it can be verified.");
    } else if slots < 5 {
        lifeline_ai::cap_grade(&mut rating, "C", "most gear slots aren't recorded.");
    }
    if state.imported.lock().unwrap().is_none() {
        lifeline_ai::cap_grade(&mut rating, "C-", "no build plan to measure progress against.");
    }
    complete_pieces(&mut rating);
    let stage = match c.act {
        Some(a) => format!("Act {a}"),
        None if c.area_level >= 65 => "Endgame".into(),
        None => "Interludes".into(),
    };
    let mut v = serde_json::to_value(&rating).map_err(|e| e.to_string())?;
    v["stage"] = json!(stage);
    v["level"] = json!(c.level);
    v["rated_at"] = json!(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0));
    *state.rating.lock().unwrap() = Some(v);
    state.rating_cards.lock().unwrap().clear();
    let _ = app.emit("rating", snapshot(state));
    Ok(())
}

/// Every gear slot shows up (ungraded ones as unrecorded), and pieces are
/// ordered by group the way the screen lists them.
fn complete_pieces(rating: &mut lifeline_ai::Rating) {
    let norm = |s: &str| s.to_lowercase().replace(['-', ' '], "");
    for slot in lifeline_ai::GEAR_SLOTS {
        if !rating.pieces.iter().any(|p| p.group == "Gear" && norm(&p.name) == norm(slot)) {
            rating.pieces.push(lifeline_ai::RatingPiece {
                group: "Gear".into(),
                name: slot.into(),
                grade: "F".into(),
                have: String::new(),
                note: "Not recorded".into(),
                verified: false,
            });
        }
    }
    let group = |g: &str| lifeline_ai::PIECE_GROUPS.iter().position(|x| *x == g).unwrap_or(usize::MAX);
    let slot = |p: &lifeline_ai::RatingPiece| {
        if p.group == "Gear" {
            lifeline_ai::GEAR_SLOTS.iter().position(|s| norm(s) == norm(&p.name)).unwrap_or(usize::MAX)
        } else {
            0
        }
    };
    // Stable: the rater's order within a group is kept (except gear slots).
    rating.pieces.sort_by_key(|p| (group(&p.group), slot(p)));
}

fn shop(app: &AppHandle, state: &AppState) {
    let recs = state
        .rating
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|r| r["recommendations"].as_array().cloned())
        .unwrap_or_default();
    let budget = state.settings.lock().unwrap().rating_budget.clone();
    for (i, rec) in recs.iter().take(MARKET_PICKS).enumerate() {
        shop_one(app, state, i, rec, None, Some(&budget));
    }
    let _ = app.emit("rating-status", json!({"busy": false, "text": ""}));
}

/// Modifiers the player wants an upgrade search to put first (from the
/// trade site's stat list or an item property such as total Armour).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Focus {
    pub primary: Option<String>,
    pub secondary: Option<String>,
}

/// Shops for one recommendation: within `budget`, or with no price limit
/// (None: the best piece the character can wear, just to see what's there).
fn shop_one(app: &AppHandle, state: &AppState, i: usize, rec: &Value, focus: Option<&Focus>, budget: Option<&str>) {
    let slot = rec["slot"].as_str().unwrap_or("item");
    let _ = app.emit(
        "rating-status",
        json!({"busy": true, "text": format!("Searching {slot}…"), "index": i}),
    );
    let level = state.character.lock().unwrap().level;
    let focus_text = focus
        .map(|f| {
            let mut lines = Vec::new();
            if let Some(p) = f.primary.as_deref().filter(|s| !s.is_empty()) {
                lines.push(format!("Primary modifier (most important, sort by it): {p}"));
            }
            if let Some(s) = f.secondary.as_deref().filter(|s| !s.is_empty()) {
                lines.push(format!("Secondary modifier (next most important): {s}"));
            }
            if lines.is_empty() {
                String::new()
            } else {
                format!(
                    "\n{}\nRequire these and rank by them first (stat ids via trade_find_stat; total Armour, Evasion, Energy \
                     Shield, Spirit, Block and weapon DPS are equipment_filters such as ar, ev, es — use those, sorted \
                     descending). Then judge the rest for the build as usual.",
                    lines.join("\n")
                )
            }
        })
        .unwrap_or_default();
    let money = match budget {
        Some(b) => format!("Budget: at most {b} per item."),
        None => format!(
            "No budget: money is no object. Find the strongest pieces the character can wear right now (requirements \
             filter lvl max {level}), sorted by the most important modifier, whatever they cost."
        ),
    };
    let prompt = format!(
        "Shop for this upgrade for the player.\nSlot: {slot}\nGoal: {}\nWhy: {}\nLook for: {}\n{money}{focus_text}\n\
         Search Instant Buyout listings in their league (trade_search with max_listings 20), then call rate_listings \
         once with every listing returned (up to 20), each rated against what they wear in that slot (equipped_items) \
         or the plan. Reply with one sentence.",
        rec["title"].as_str().unwrap_or(""),
        rec["why"].as_str().unwrap_or(""),
        rec["look_for"].as_str().unwrap_or(""),
    );
    let scope = MARKET_SCOPE_BASE + i as u64;
    let run = crate::ai::companion(state, scope).and_then(|mut cli| {
        cli.main_agent = Some(MARKET_SCOUT.into());
        cli.model = Some(MARKET_MODEL.into());
        cli.persist_session = false;
        cli.run_turn(&prompt, None, |e| usage(app, e)).map_err(|e| e.to_string())
    });
    if let Err(e) = run {
        let _ = app.emit(
            "rating-status",
            json!({"busy": true, "text": format!("Market search for {slot} failed: {e}"), "index": i}),
        );
    }
}

/// Re-runs one upgrade's market search with the player's focus modifiers,
/// within the Rating budget or (`any_price`) with no budget.
pub fn rerun_upgrade(app: AppHandle, index: usize, focus: Focus, any_price: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let rec = state
        .rating
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|r| r["recommendations"].get(index).cloned())
        .ok_or("That upgrade is gone. Rate again.")?;
    if index >= MARKET_PICKS {
        return Err("Only the top upgrades have market searches.".into());
    }
    if state.rating_busy.swap(true, Ordering::SeqCst) {
        return Err("Still rating or searching. Try again in a moment.".into());
    }
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let budget = state.settings.lock().unwrap().rating_budget.clone();
        shop_one(&app, &state, index, &rec, Some(&focus), (!any_price).then_some(budget.as_str()));
        state.rating_busy.store(false, Ordering::SeqCst);
        let _ = app.emit("rating-status", json!({"busy": false, "text": ""}));
        state.save_rating();
        let _ = app.emit("rating", snapshot(&state));
        crate::sound::play(&app, crate::sound::Cue::Ready);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gear_slot_is_listed_in_order() {
        let piece = |group: &str, name: &str| lifeline_ai::RatingPiece {
            group: group.into(),
            name: name.into(),
            grade: "B".into(),
            have: String::new(),
            note: String::new(),
            verified: true,
        };
        let mut r = lifeline_ai::Rating {
            grade: "C".into(),
            score: 40.0,
            summary: String::new(),
            explanation: String::new(),
            categories: vec![],
            pieces: vec![piece("Defences", "Fire res"), piece("Gear", "Boots"), piece("Skills", "Spear Throw"), piece("Gear", "off hand")],
            recommendations: vec![],
        };
        complete_pieces(&mut r);
        let gear: Vec<&str> = r.pieces.iter().filter(|p| p.group == "Gear").map(|p| p.name.as_str()).collect();
        assert_eq!(gear.len(), 10);
        assert_eq!(&gear[..2], &["Weapon", "off hand"]);
        assert_eq!(r.pieces[10].name, "Spear Throw");
        assert_eq!(r.pieces[11].name, "Fire res");
        assert!(r.pieces.iter().any(|p| p.name == "Weapon" && p.grade == "F" && !p.verified));
    }
}
