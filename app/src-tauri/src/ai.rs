//! Turns questions (typed, or from hotkeys) into Claude companion turns.
//! Each conversation is its own Claude session and several can run at once;
//! every turn is told what the other open conversations are about.

use std::sync::Once;

use polr_ai::{agents, ClaudeCli, CliEvent};
use polr_model::skills_for_stage;
use polr_pob::Stage;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::{AppState, Message};

/// Where the question came from; hotkey answers go to the in-game thread and
/// the overlay.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Chat,
    Hotkey,
}

pub(crate) fn companion(state: &AppState, conv: u64) -> Result<ClaudeCli, String> {
    static WRITE_AGENTS: Once = Once::new();
    let work = state.data_dir.join("claude");
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    // Concurrent turns share one agents file; write it once per launch.
    WRITE_AGENTS.call_once(|| {
        let _ = agents::write_agents_file(&work);
    });
    let mut cli = ClaudeCli::new(work.clone());
    cli.agents_file = Some(work.join(agents::AGENTS_FILE));
    cli.builtin_tools.push("Agent".into());
    cli.allowed_tools.push("Agent".into());
    if let Some(config) = crate::mcp::config_for(state, conv) {
        let path = work.join(format!("mcp-{conv}.json"));
        std::fs::write(&path, config).map_err(|e| e.to_string())?;
        cli.mcp_config = Some(path);
        cli.allowed_tools
            .extend(polr_ai::tools::qualified(polr_ai::tools::IMPLEMENTED));
    }
    cli.disallowed_tools = agents::BUILT_IN_AGENTS.iter().map(|a| format!("Agent({a})")).collect();
    Ok(cli)
}
pub fn current_stage(act: Option<u8>, area_level: u32) -> Stage {
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
        "[Live context from PathOfLeastResistance. The polr tools are connected: live character state, the imported build, \
         game data for the current patch (gems, supports, item bases, mods, uniques, passives, areas) and the trade market. \
         Look facts up with them first; use the web only for what they don't cover (boss mechanics, patch notes) and say \
         what you could not verify.]"
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
                let supports: Vec<String> = s
                    .support_skills
                    .iter()
                    .map(|x| crate::gamedata::gem_display(state, &x.id))
                    .collect();
                if supports.is_empty() {
                    crate::gamedata::gem_display(state, &s.id)
                } else {
                    format!(
                        "{} [{}]",
                        crate::gamedata::gem_display(state, &s.id),
                        supports.join(", ")
                    )
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

fn clip(text: &str, max: usize) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        flat
    } else {
        format!("{}…", flat.chars().take(max).collect::<String>())
    }
}

/// What the other open conversations are about, so threads stay aware of
/// one another (decisions made in one apply in the others).
fn awareness(state: &AppState, conv_id: u64) -> String {
    let convs = state.conversations.lock().unwrap();
    let others: Vec<String> = convs
        .iter()
        .filter(|c| c.id != conv_id && !c.messages.is_empty())
        .rev()
        .take(6)
        .map(|c| {
            let last_q = c.messages.iter().rev().find(|m| m.role == "user");
            let last_a = c.messages.iter().rev().find(|m| m.role == "ai" && !m.error);
            let mut line = format!(
                "- #{} \"{}\"{}",
                c.id,
                c.title,
                if c.busy { " (answering right now)" } else { "" }
            );
            if let Some(q) = last_q {
                line.push_str(&format!("\n  last asked: {}", clip(&q.text, 300)));
            }
            if let Some(a) = last_a {
                line.push_str(&format!("\n  last answer: {}", clip(&a.text, 600)));
            }
            line
        })
        .collect();
    if others.is_empty() {
        return String::new();
    }
    format!(
        "\n\n[Other conversations the player has open in this app. They are the same player and character: \
         treat decisions, items and plans from them as known, refer to them by #number when relevant, and \
         point out conflicts.]\n{}",
        others.join("\n")
    )
}

fn emit(app: &AppHandle, conv: u64, mut payload: serde_json::Value) {
    payload["conv"] = json!(conv);
    let _ = app.emit("ai", payload);
}

/// Asks in conversation `conv` (or the in-game thread for hotkeys / `None`).
pub fn ask(app: &AppHandle, conv: Option<u64>, question: String, label: &str, origin: Origin) {
    let state = app.state::<AppState>();
    let conv_id = match (origin, conv) {
        (Origin::Chat, Some(id)) => id,
        _ => state.in_game_conversation(),
    };
    {
        let mut convs = state.conversations.lock().unwrap();
        let Some(c) = convs.iter_mut().find(|c| c.id == conv_id) else {
            emit(
                app,
                conv_id,
                json!({"type": "error", "text": "That conversation no longer exists."}),
            );
            return;
        };
        if c.busy {
            emit(
                app,
                conv_id,
                json!({"type": "error", "text": "Still answering. Start a new chat to ask in parallel."}),
            );
            return;
        }
        c.busy = true;
        if c.messages.is_empty() && !c.in_game && c.title.starts_with("New conversation") {
            c.title = clip(&question, 40);
        }
        c.messages.push(Message {
            role: "user".into(),
            label: label.into(),
            text: question.clone(),
            error: false,
        });
    }
    let _ = app.emit("conversations", conversation_list(&state));
    if origin == Origin::Hotkey && state.settings.lock().unwrap().overlay_on_hotkey {
        crate::hotkeys::remember_overlay(app, Some(true), None);
    }
    emit(
        app,
        conv_id,
        json!({"type": "start", "label": label, "question": question, "hotkey": origin == Origin::Hotkey}),
    );

    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        // Hotkey answers land on a tiny in-game HUD.
        let hud = if origin == Origin::Hotkey {
            "\n\n(Shown on a tiny in-game HUD: at most 25 words, one line or up to 3 very short bullets, no headings.)"
        } else {
            ""
        };
        let prompt = format!("{}{}\n\n{question}{hud}", context(&state), awareness(&state, conv_id));
        let resume = state
            .conversations
            .lock()
            .unwrap()
            .iter()
            .find(|c| c.id == conv_id)
            .and_then(|c| c.session.clone());
        let result = companion(&state, conv_id).and_then(|cli| {
            cli.run_turn(&prompt, resume.as_deref(), |event| match event {
                CliEvent::TextDelta(text) => emit(&app, conv_id, json!({"type": "delta", "text": text})),
                CliEvent::Assistant { tool_uses, .. } => {
                    for t in tool_uses {
                        let agent = t.input.get("subagent_type").and_then(|v| v.as_str());
                        let what = agent.map_or(t.name.clone(), |a| format!("consulting {a}"));
                        emit(&app, conv_id, json!({"type": "tool", "text": what}));
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
                CliEvent::ApiRetry { attempt, error, .. } => emit(
                    &app,
                    conv_id,
                    json!({"type": "tool", "text": format!("retrying ({attempt}) {}", error.clone().unwrap_or_default())}),
                ),
                _ => {}
            })
            .map_err(|e| e.to_string())
        });
        let (text, error, session) = match result {
            Ok(outcome) => (
                outcome.result.result.clone().unwrap_or_default(),
                outcome.result.is_error,
                outcome.session_id,
            ),
            Err(e) => (e, true, None),
        };
        {
            let mut convs = state.conversations.lock().unwrap();
            if let Some(c) = convs.iter_mut().find(|c| c.id == conv_id) {
                c.busy = false;
                if session.is_some() {
                    c.session = session;
                }
                c.messages.push(Message {
                    role: "ai".into(),
                    label: "Companion".into(),
                    text: text.clone(),
                    error,
                });
            }
        }
        state.save_conversations();
        emit(&app, conv_id, json!({"type": "done", "text": text, "error": error}));
        let _ = app.emit("conversations", conversation_list(&state));
    });
}

/// Sidebar view: id, title, busy, in-game flag, message count.
pub fn conversation_list(state: &AppState) -> serde_json::Value {
    let convs = state.conversations.lock().unwrap();
    json!(convs
        .iter()
        .map(|c| json!({"id": c.id, "title": c.title, "busy": c.busy, "in_game": c.in_game, "count": c.messages.len()}))
        .collect::<Vec<_>>())
}
pub fn item_question(item: &str) -> String {
    format!(
        "Item check — I copied this item in game (Ctrl+Alt+C):\n```\n{item}\n```\n\
         Equip, keep or sell for my current stage? Delegate to the gear-appraiser. \
         Verdict first, then the one or two numbers that decide it."
    )
}

pub const CREATE_BUILD: &str = "Create a complete build for me, Act 1 to Endgame, that I can follow in the game's \
     Build Planner. Ask me what you need first.";

pub const WHAT_NEXT: &str = "What should I do next? Delegate to the route-coach. At most three steps of a few words \
     each, safety first if anything is pressing.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_conversation_sees_the_others_but_not_itself() {
        let dir = std::env::temp_dir().join(format!("polr-aware-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let state = AppState::new(dir.clone());
        let boots = state.new_conversation("Boots for Act 3", false);
        let ring = state.new_conversation("Ring upgrade", false);
        {
            let mut convs = state.conversations.lock().unwrap();
            for c in convs.iter_mut() {
                let (q, a) = if c.id == boots {
                    (
                        "Find boots with cold res",
                        "Bought Stormrider Boots: 25% MS, +32% cold res.",
                    )
                } else {
                    ("Is this ring good?", "Keep it: +40 life, +20% fire.")
                };
                c.messages.push(Message {
                    role: "user".into(),
                    label: "Chat".into(),
                    text: q.into(),
                    error: false,
                });
                c.messages.push(Message {
                    role: "ai".into(),
                    label: "Companion".into(),
                    text: a.into(),
                    error: false,
                });
            }
            convs.iter_mut().find(|c| c.id == boots).unwrap().busy = true;
        }
        let seen_from_ring = awareness(&state, ring);
        assert!(seen_from_ring.contains("\"Boots for Act 3\" (answering right now)"));
        assert!(seen_from_ring.contains("+32% cold res"));
        assert!(!seen_from_ring.contains("Ring upgrade"));
        assert!(awareness(&state, boots).contains("+40 life"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
