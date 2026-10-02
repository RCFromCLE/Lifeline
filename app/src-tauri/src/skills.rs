//! Skills, supports, buttons and rotations for the current stage (skill-coach
//! on Opus 5.5). Shown on the Skills tab and, as a rotation, on the HUD.

use std::sync::atomic::Ordering;

use polr_ai::{structured, CliEvent, Job};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

pub const SKILLS_SCOPE: u64 = 900_050;

pub fn snapshot(state: &AppState) -> Value {
    json!({
        "plan": state.skills_plan.lock().unwrap().clone(),
        "busy": state.skills_busy.load(Ordering::SeqCst),
    })
}

pub fn run(app: AppHandle) {
    let state = app.state::<AppState>();
    if state.skills_busy.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = app.emit(
        "skills-status",
        json!({"busy": true, "text": "Setting up your skills and rotations (Opus 5.5)…"}),
    );
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let c = state.character.lock().unwrap().clone();
        let context =
            json!({"character": c.name, "class": c.class, "level": c.level, "act": c.act, "area_level": c.area_level});
        let result = crate::ai::companion(&state, SKILLS_SCOPE).and_then(|cli| {
            cli.for_job(Job::Skills)
                .run_turn(&Job::Skills.prompt(&context.to_string()), None, |e| {
                    if let CliEvent::RateLimit(info) = e {
                        let windows: Vec<_> = info
                            .windows
                            .iter()
                            .map(|w| json!({"name": w.name, "pct": (w.utilization * 100.0).round()}))
                            .collect();
                        let _ = app.emit("usage", json!({"windows": windows}));
                    }
                })
                .map_err(|e| e.to_string())
        });
        let text = match result.as_ref().ok().and_then(|o| structured(&o.result)) {
            Some(mut plan) => {
                plan["level"] = json!(c.level);
                plan["stage"] = json!(match c.act {
                    Some(a) => format!("Act {a}"),
                    None if c.area_level >= 65 => "Endgame".into(),
                    None => "Interludes".into(),
                });
                *state.skills_plan.lock().unwrap() = Some(plan);
                state.save_skills();
                String::new()
            }
            None => format!(
                "Skill setup failed: {}",
                result.err().unwrap_or_else(|| "no plan returned".into())
            ),
        };
        state.skills_busy.store(false, Ordering::SeqCst);
        let _ = app.emit("skills-status", json!({"busy": false, "text": text}));
        let _ = app.emit("skills", snapshot(&state));
    });
}
