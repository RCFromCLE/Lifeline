//! Build rating (F … S+ for the current stage) plus market picks for its
//! recommendations. The rating runs on Opus 5.5 (build-rater); each market
//! search runs on Sonnet 5.5 (market-scout) within the player's budget.

use std::sync::atomic::Ordering;

use polr_ai::agents::{MARKET_MODEL, MARKET_SCOUT};
use polr_ai::{parse_rating, CliEvent, Job};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

/// MCP scopes for rating runs (conversation ids are small numbers).
pub const RATING_SCOPE: u64 = 900_000;
pub const MARKET_SCOPE_BASE: u64 = 900_100;
const MARKET_PICKS: usize = 3;

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
        json!({"busy": true, "text": "Rating your build (Opus 5.5)…"}),
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
    let rating = parse_rating(&result.result).ok_or("the rater didn't return a rating")?;
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
        let slot = rec["slot"].as_str().unwrap_or("item");
        let _ = app.emit(
            "rating-status",
            json!({"busy": true, "text": format!("Searching the market for {slot} (Sonnet 5.5)…"), "index": i}),
        );
        let prompt = format!(
            "Shop for this upgrade for the player.\nSlot: {slot}\nGoal: {}\nWhy: {}\nLook for: {}\nBudget: at most {budget} per item.\n\
             Search Instant Buyout listings in their league within the budget, then call rate_listings with the best \
             3–4 listings, rated against what they wear in that slot (equipped_items) or the plan. Reply with one sentence.",
            rec["title"].as_str().unwrap_or(""),
            rec["why"].as_str().unwrap_or(""),
            rec["look_for"].as_str().unwrap_or(""),
        );
        let scope = MARKET_SCOPE_BASE + i as u64;
        let run = crate::ai::companion(state, scope).and_then(|mut cli| {
            cli.main_agent = Some(MARKET_SCOUT.into());
            cli.model = Some(MARKET_MODEL.into());
            cli.persist_session = false;
            cli.run_turn(&prompt, None, |e| usage(app, e))
                .map_err(|e| e.to_string())
        });
        if let Err(e) = run {
            let _ = app.emit(
                "rating-status",
                json!({"busy": true, "text": format!("Market search for {slot} failed: {e}"), "index": i}),
            );
        }
    }
    let _ = app.emit("rating-status", json!({"busy": false, "text": ""}));
}
