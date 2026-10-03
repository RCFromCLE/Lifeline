//! The Builds tab: the researched showcase (catalog), class art from GGG's
//! tree export, the player's saved builds, and generating a full staged
//! build from a showcase pick (build architect on Opus 5.5).

use std::sync::atomic::Ordering;

use lifeline_ai::{parse_report, CliEvent, Job};
use lifeline_model::{Archetype, BuildDesign, DesignReport, Preferences};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

/// MCP scope for wizard generation runs.
pub const BUILD_SCOPE: u64 = 900_200;
const LIBRARY_FILE: &str = "library.json";
const ART_BASE: &str = "https://raw.githubusercontent.com/grindinggear/poe2-skilltree-export/master/assets";

// ---- showcase ----

pub fn catalog() -> Vec<Archetype> {
    lifeline_model::catalog::bundled()
}

/// Ranked showcase for the wizard's picks, plus the classes to choose from.
pub fn showcase(state: &AppState, prefs: &Preferences) -> Value {
    let all = catalog();
    let ranked = lifeline_model::rank(&all, prefs);
    let c = state.character.lock().unwrap().clone();
    json!({
        "total": all.len(),
        "builds": ranked,
        "character": {"class": c.class, "level": c.level, "name": c.name},
        "classes": classes(state, &all),
    })
}

/// Released classes and ascendancies (from the tree), with catalog counts.
fn classes(state: &AppState, all: &[Archetype]) -> Value {
    let Ok(tree) = crate::builds::load_tree(state) else {
        return json!([]);
    };
    let list: Vec<Value> = tree
        .classes()
        .iter()
        .filter(|c| c.ascendancies.iter().any(|a| !a.name.is_empty()))
        .map(|c| {
            let ascs: Vec<Value> = c
                .ascendancies
                .iter()
                .filter(|a| !a.name.is_empty())
                .map(|a| {
                    let n = all.iter().filter(|x| x.class == c.name && x.ascendancy == a.name).count();
                    json!({"name": a.name, "builds": n})
                })
                .collect();
            let n = all.iter().filter(|x| x.class == c.name).count();
            json!({"name": c.name, "ascendancies": ascs, "builds": n})
        })
        .collect();
    json!(list)
}

// ---- art ----

/// Where each class's and ascendancy's illustration sits in GGG's art
/// atlases (`assets/background-<class>.webp`). Descriptors are cached.
pub fn art(state: &AppState) -> Value {
    let Ok(tree) = crate::builds::load_tree(state) else {
        return json!({});
    };
    let dir = state.data_dir.join("art");
    let _ = std::fs::create_dir_all(&dir);
    let mut out = serde_json::Map::new();
    for class in tree.classes().iter().filter(|c| c.ascendancies.iter().any(|a| !a.name.is_empty())) {
        let lower = class.name.to_lowercase();
        let cache = dir.join(format!("background-{lower}.json"));
        let text = std::fs::read_to_string(&cache).ok().or_else(|| {
            let t = crate::builds::http_get(&format!("{ART_BASE}/background-{lower}.json")).ok()?;
            let _ = std::fs::write(&cache, &t);
            Some(t)
        });
        let Some(desc) = text.and_then(|t| serde_json::from_str::<Value>(&t).ok()) else {
            continue;
        };
        let frame = |n: usize| desc["frames"][format!("class{}:Class{n}", class.name)]["frame"].clone();
        let mut frames = serde_json::Map::new();
        frames.insert(String::new(), frame(0));
        for (i, a) in class.ascendancies.iter().enumerate() {
            let f = frame(i + 1);
            if !a.name.is_empty() && f.is_object() {
                frames.insert(a.name.clone(), f);
            }
        }
        out.insert(
            class.name.clone(),
            json!({
                "url": format!("{ART_BASE}/background-{lower}.webp"),
                "w": desc["meta"]["size"]["w"], "h": desc["meta"]["size"]["h"],
                "frames": frames,
            }),
        );
    }
    Value::Object(out)
}

// ---- library ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedBuild {
    pub id: u64,
    /// The showcase pick this came from.
    pub archetype: Option<Archetype>,
    /// The generated, staged build (absent for saved ideas).
    #[serde(default)]
    pub design: Option<BuildDesign>,
    #[serde(default)]
    pub report: Option<DesignReport>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub risks: Vec<String>,
    pub saved_at: u64,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

pub fn library(state: &AppState) -> Vec<SavedBuild> {
    std::fs::read_to_string(state.data_dir.join(LIBRARY_FILE))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_library(state: &AppState, list: &[SavedBuild]) {
    if let Ok(t) = serde_json::to_string_pretty(list) {
        let _ = crate::state::write_atomic(&state.data_dir.join(LIBRARY_FILE), &t);
    }
}

fn add(state: &AppState, mut entry: SavedBuild) -> SavedBuild {
    let mut list = library(state);
    entry.id = list.iter().map(|e| e.id).max().unwrap_or(0) + 1;
    entry.saved_at = now();
    list.insert(0, entry.clone());
    save_library(state, &list);
    entry
}

/// Saves a showcase pick as an idea (no-op if it's already saved as one).
pub fn save_idea(state: &AppState, archetype_id: &str) -> Result<Vec<SavedBuild>, String> {
    let a = catalog()
        .into_iter()
        .find(|a| a.id == archetype_id)
        .ok_or("That build isn't in the showcase.")?;
    let exists = library(state)
        .iter()
        .any(|e| e.design.is_none() && e.archetype.as_ref().is_some_and(|x| x.id == a.id));
    if !exists {
        add(
            state,
            SavedBuild {
                id: 0,
                archetype: Some(a),
                design: None,
                report: None,
                summary: String::new(),
                risks: Vec::new(),
                saved_at: 0,
            },
        );
    }
    Ok(library(state))
}

pub fn remove(state: &AppState, id: u64) -> Vec<SavedBuild> {
    let mut list = library(state);
    list.retain(|e| e.id != id);
    save_library(state, &list);
    list
}

/// Makes a generated build the one the player follows.
pub fn activate(state: &AppState, id: u64) -> Result<crate::builds::ImportView, String> {
    let entry = library(state).into_iter().find(|e| e.id == id).ok_or("That build is gone.")?;
    let design = entry.design.ok_or("Generate this build first.")?;
    crate::builds::activate(state, &design)
}

// ---- generation ----

fn step(app: &AppHandle, text: impl Into<String>) {
    let state = app.state::<AppState>();
    let text = text.into();
    state.build_steps.lock().unwrap().push(text.clone());
    let _ = app.emit("build-progress", json!({"step": text}));
}

/// A friendly line for a tool call the architect makes.
fn describe(name: &str, input: &Value) -> Option<String> {
    let arg = |k: &str| input[k].as_str().unwrap_or_default().to_owned();
    let short = name.rsplit("__").next().unwrap_or(name);
    Some(match short {
        "lookup_gem" => format!("Looking up {}", arg("name")),
        "lookup_supports_for" => format!("Checking supports for {}", arg("skill")),
        "lookup_passive" => format!("Finding passives: {}", arg("query")),
        "lookup_unique" => format!("Checking unique {}", arg("name")),
        "lookup_base" => format!("Checking bases: {}", arg("query")),
        "character_state" => "Reading your character".into(),
        "design_build" => "Laying out every stage…".into(),
        "Agent" | "Task" => format!("Consulting the {}", arg("subagent_type")),
        "WebSearch" => format!("Searching: {}", arg("query")),
        "WebFetch" => "Reading a guide".into(),
        _ => return None,
    })
}

/// Called by the MCP server after each wizard design_build attempt.
pub fn attempt(app: &AppHandle, report: &DesignReport) {
    let missed: usize = report.stages.iter().map(|s| s.missed.len()).sum();
    let problems: usize = report.problems.len() + report.stages.iter().map(|s| s.problems.len()).sum::<usize>();
    let unspent = report.stages.last().map_or(0, |s| s.points_unspent);
    step(
        app,
        if missed + problems == 0 {
            format!("Design checks out ({unspent} points left to place)")
        } else {
            format!("Fixing {missed} unreachable picks and {problems} problems")
        },
    );
}

pub fn status(state: &AppState) -> Value {
    json!({
        "busy": state.build_busy.load(Ordering::SeqCst),
        "steps": state.build_steps.lock().unwrap().clone(),
    })
}

/// Generates the full staged build for a showcase pick; the result lands in
/// the library ("build-done").
pub fn generate(app: AppHandle, archetype_id: String, prefs: Preferences) -> Result<(), String> {
    let state = app.state::<AppState>();
    let archetype = catalog()
        .into_iter()
        .find(|a| a.id == archetype_id)
        .ok_or("That build isn't in the showcase.")?;
    if state.build_busy.swap(true, Ordering::SeqCst) {
        return Err("A build is already being generated.".into());
    }
    state.build_steps.lock().unwrap().clear();
    *state.last_design.lock().unwrap() = None;
    step(&app, format!("Designing {} (Opus 5.5)", archetype.name));
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let c = state.character.lock().unwrap().clone();
        let league = state.settings.lock().unwrap().league.clone();
        let for_new = !c.class.eq_ignore_ascii_case(&archetype.class);
        let context = json!({
            "character": {"class": c.class, "level": c.level, "league": league},
            "for_new_character": for_new,
            "archetype": archetype,
            "preferences": prefs,
        });
        let result = crate::ai::companion(&state, BUILD_SCOPE).and_then(|cli| {
            cli.for_job(Job::DesignBuild)
                .run_turn(&Job::DesignBuild.prompt(&context.to_string()), None, |e| match e {
                    CliEvent::Assistant { tool_uses, .. } => {
                        for t in tool_uses {
                            if let Some(s) = describe(&t.name, &t.input) {
                                step(&app, s);
                            }
                        }
                    }
                    CliEvent::RateLimit(info) => {
                        let windows: Vec<_> = info
                            .windows
                            .iter()
                            .map(|w| json!({"name": w.name, "pct": (w.utilization * 100.0).round()}))
                            .collect();
                        let _ = app.emit("usage", json!({"windows": windows}));
                    }
                    _ => {}
                })
                .map_err(|e| e.to_string())
        });
        let outcome = match (result, state.last_design.lock().unwrap().take()) {
            (Ok(out), Some((design, report))) => {
                let job = parse_report(&out.result);
                let entry = add(
                    &state,
                    SavedBuild {
                        id: 0,
                        archetype: Some(archetype),
                        design: Some(design),
                        report: Some(report),
                        summary: job.as_ref().map(|j| j.summary.clone()).unwrap_or_default(),
                        risks: job
                            .map(|j| j.findings.into_iter().map(|f| format!("{}: {}", f.title, f.detail)).collect())
                            .unwrap_or_default(),
                        saved_at: 0,
                    },
                );
                step(&app, "Saved to My builds");
                json!({"ok": true, "entry": entry})
            }
            (Ok(_), None) => json!({"ok": false, "error": "The architect didn't save a design. Try again."}),
            (Err(e), _) => json!({"ok": false, "error": e}),
        };
        state.build_busy.store(false, Ordering::SeqCst);
        crate::sound::play(&app, crate::sound::Cue::Ready);
        let _ = app.emit("build-done", outcome);
        let _ = app.emit("library", library(&state));
    });
    Ok(())
}
