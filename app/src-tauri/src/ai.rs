//! Turns questions (typed, or from hotkeys) into Claude companion turns and
//! streams the answer to the main window and the HUD overlay.

use std::sync::atomic::Ordering;

use polr_ai::{agents, ClaudeCli, CliEvent};
use polr_gamefiles::build_planner::gem_short_name;
use polr_model::skills_for_stage;
use polr_pob::Stage;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

/// Where the question came from; hotkey answers also go to the overlay.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Chat,
    Hotkey,
}

fn companion(state: &AppState) -> Result<ClaudeCli, String> {
    let work = state.data_dir.join("claude");
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let agents_file = agents::write_agents_file(&work).map_err(|e| e.to_string())?;
    let mut cli = ClaudeCli::new(work);
    cli.agents_file = Some(agents_file);
    cli.builtin_tools.push("Agent".into());
    cli.allowed_tools.push("Agent".into());
    cli.disallowed_tools = agents::BUILT_IN_AGENTS.iter().map(|a| format!("Agent({a})")).collect();
    Ok(cli)
}

fn current_stage(act: Option<u8>, area_level: u32) -> Stage {
    match act {
        Some(n) => Stage::Act(n),
        None if area_level >= 65 => Stage::Endgame,
        None => Stage::Interludes,
    }
}

/// Live context attached to every question until the MCP tools land (M3).
fn context(state: &AppState) -> String {
    let c = state.character.lock().unwrap().clone();
    let league = state.settings.lock().unwrap().league.clone();
    let mut lines = vec![
        "[Live context from PathOfLeastResistance. The polr tools are not connected in this build yet, \
         so use this context plus web lookups (poe2db.tw, poe2wiki.net, pathofexile.com) and say what you \
         could not verify.]"
            .to_string(),
        format!("League: {league}"),
    ];
    match &c.name {
        Some(name) => lines.push(format!("Character: {name} ({}) level {}", c.class, c.level)),
        None => lines.push("Character: unknown (no level-up seen in the log yet)".into()),
    }
    if !c.zone.is_empty() {
        let act = c.act.map_or("not campaign".to_string(), |a| format!("Act {a}"));
        lines.push(format!(
            "Zone: {} ({}, area level {}, {act})",
            c.zone, c.area_id, c.area_level
        ));
    }
    if let Some(p) = c.res_penalty {
        lines.push(format!("Elemental resistance penalty in this area: {p}%"));
    }
    lines.push(format!("Deaths this character: {}", c.deaths));
    if !c.buffs.is_empty() {
        lines.push(format!("Permanent buffs collected: {}", c.buffs.join("; ")));
    }
    if let Some(imported) = state.imported.lock().unwrap().as_ref() {
        let stage = current_stage(c.act, c.area_level);
        let skills: Vec<String> = skills_for_stage(&imported.build, stage)
            .iter()
            .map(|s| {
                let supports: Vec<&str> = s.support_skills.iter().map(|x| gem_short_name(&x.id)).collect();
                if supports.is_empty() {
                    gem_short_name(&s.id).to_string()
                } else {
                    format!("{} [{}]", gem_short_name(&s.id), supports.join(", "))
                }
            })
            .collect();
        lines.push(format!(
            "Imported build: {} — {} plan skills: {}",
            imported.name,
            polr_model::stage_label(stage),
            if skills.is_empty() {
                "(none listed)".into()
            } else {
                skills.join("; ")
            }
        ));
    }
    lines.join("\n")
}

pub fn ask(app: &AppHandle, question: String, label: &str, origin: Origin) {
    let state = app.state::<AppState>();
    if state.ai_busy.swap(true, Ordering::SeqCst) {
        let _ = app.emit(
            "ai",
            json!({"type": "error", "text": "Still answering the last question — one moment."}),
        );
        return;
    }
    let show_overlay = origin == Origin::Hotkey && state.settings.lock().unwrap().overlay_on_hotkey;
    if show_overlay {
        if let Some(w) = app.get_webview_window("overlay") {
            let _ = w.show();
        }
    }
    let _ = app.emit("ai", json!({"type": "start", "label": label, "question": question}));

    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let prompt = format!("{}\n\n{question}", context(&state));
        let resume = state.ai_session.lock().unwrap().clone();
        let result = companion(&state).and_then(|cli| {
            cli.run_turn(&prompt, resume.as_deref(), |event| match event {
                CliEvent::TextDelta(text) => {
                    let _ = app.emit("ai", json!({"type": "delta", "text": text}));
                }
                CliEvent::Assistant { tool_uses, .. } => {
                    for t in tool_uses {
                        let agent = t.input.get("subagent_type").and_then(|v| v.as_str());
                        let what = agent.map_or(t.name.clone(), |a| format!("consulting {a}"));
                        let _ = app.emit("ai", json!({"type": "tool", "text": what}));
                    }
                }
                CliEvent::RateLimit(info) => {
                    let windows: Vec<_> = info
                        .windows
                        .iter()
                        .map(|w| json!({"name": w.name, "pct": (w.utilization * 100.0).round(), "resets_at": w.resets_at}))
                        .collect();
                    let _ = app.emit("usage", json!({"status": info.status, "windows": windows}));
                }
                CliEvent::ApiRetry { attempt, error, .. } => {
                    let _ = app.emit("ai", json!({"type": "tool", "text": format!("retrying ({attempt}) {}", error.clone().unwrap_or_default())}));
                }
                _ => {}
            })
            .map_err(|e| e.to_string())
        });
        match result {
            Ok(outcome) => {
                if outcome.session_id.is_some() {
                    *state.ai_session.lock().unwrap() = outcome.session_id.clone();
                }
                let text = outcome.result.result.unwrap_or_default();
                let _ = app.emit(
                    "ai",
                    json!({"type": "done", "text": text, "error": outcome.result.is_error}),
                );
            }
            Err(e) => {
                let _ = app.emit("ai", json!({"type": "done", "text": e, "error": true}));
            }
        }
        state.ai_busy.store(false, Ordering::SeqCst);
    });
}

pub fn item_question(item: &str) -> String {
    format!(
        "Item check — I copied this item in game (Ctrl+Alt+C):\n```\n{item}\n```\n\
         Should I equip it, keep it for later, or sell/ignore it for my current stage? Delegate to the \
         gear-appraiser. Lead with the verdict and the two or three numbers that decide it."
    )
}

pub const WHAT_NEXT: &str = "What should I do next? Delegate to the route-coach. Give at most three short steps, \
     safety first if anything is pressing.";
