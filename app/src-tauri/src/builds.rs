//! Build import, tree data and writing stages to the in-game Build Planner.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use lifeline_data::{PassiveTree, TREE_EXPORT_URL};
use lifeline_gamefiles::{build_planner, paths};
use lifeline_model::{plan_stages, realize, to_planner_build, BuildDesign, DesignReport, SpecStage};
use lifeline_pob::{decode, parse_xml, resolve, BuildSource};
use serde::{Deserialize, Serialize};

use crate::state::{AppState, Imported};

const USER_AGENT: &str = "Lifeline/0.1 (personal PoE2 companion)";
const TREE_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);

/// A web client that gives up instead of hanging on a stalled connection.
pub fn web() -> &'static ureq::Agent {
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_connect(Some(std::time::Duration::from_secs(15)))
            .timeout_recv_response(Some(std::time::Duration::from_secs(30)))
            .timeout_recv_body(Some(std::time::Duration::from_secs(180)))
            .build()
            .into()
    })
}

pub fn http_get(url: &str) -> Result<String, String> {
    let mut response = web()
        .get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| format!("download failed ({url}): {e}"))?;
    response
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("download failed ({url}): {e}"))
}

/// GGG's tree export, cached in the app data folder and refreshed weekly.
pub fn load_tree(state: &AppState) -> Result<Arc<PassiveTree>, String> {
    if let Some(tree) = state.tree.lock().unwrap().clone() {
        return Ok(tree);
    }
    let cache = state.data_dir.join("poe2-tree-export.json");
    let fresh = std::fs::metadata(&cache)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < TREE_MAX_AGE);
    let text = if fresh {
        std::fs::read_to_string(&cache).map_err(|e| e.to_string())?
    } else {
        match http_get(TREE_EXPORT_URL) {
            Ok(text) => {
                let _ = std::fs::write(&cache, &text);
                text
            }
            Err(e) => std::fs::read_to_string(&cache).map_err(|_| e)?,
        }
    };
    let tree = Arc::new(PassiveTree::from_json(&text).map_err(|e| format!("tree data unreadable: {e}"))?);
    *state.tree.lock().unwrap() = Some(tree.clone());
    Ok(tree)
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportView {
    pub name: String,
    pub source: String,
    pub class_name: Option<String>,
    pub ascendancy: Option<String>,
    pub level: Option<u32>,
    pub stages: Vec<SpecStage>,
    pub skill_sets: Vec<String>,
    pub item_sets: Vec<String>,
    pub warning: Option<String>,
}

pub fn view(imported: &Imported, warning: Option<String>) -> ImportView {
    let b = &imported.build;
    let titled = |t: &str| {
        if t.is_empty() {
            "(default)".to_string()
        } else {
            t.to_string()
        }
    };
    ImportView {
        name: imported.name.clone(),
        source: imported.source.clone(),
        class_name: b.class_name.clone(),
        ascendancy: b.ascend_class_name.clone(),
        level: b.level,
        stages: imported.stages.clone(),
        skill_sets: b.skill_sets.iter().map(|s| titled(&s.title)).collect(),
        item_sets: b.item_sets.iter().map(|s| titled(&s.title)).collect(),
        warning,
    }
}

pub fn import(state: &AppState, input: &str) -> Result<ImportView, String> {
    let (code, link) = match resolve(input) {
        BuildSource::Code(code) => (code, None),
        BuildSource::CodeUrl(url) => (http_get(&url)?, Some(input.trim().to_owned())),
        BuildSource::MaxrollPlannerUrl(_) => {
            return Err("Maxroll planner links aren't supported yet — use the guide's Path of Building link (maxroll.gg/poe2/pob/…).".into())
        }
        BuildSource::Unsupported { site, advice } => return Err(format!("{site}: {advice}")),
    };
    let build = parse_xml(&decode(&code).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let (tree, warning) = match load_tree(state) {
        Ok(t) => (Some(t), None),
        Err(e) => (
            None,
            Some(format!(
                "Passive tree data unavailable ({e}); stage estimates are rough and planner export is disabled."
            )),
        ),
    };
    let stages = plan_stages(&build, tree.as_deref());
    let name = build
        .ascend_class_name
        .clone()
        .or_else(|| build.class_name.clone())
        .unwrap_or_else(|| "Build".into());
    let imported = Imported {
        build,
        stages,
        link: link.clone(),
        name: format!("{name} (Lifeline)"),
        source: "Path of Building".into(),
    };
    let v = view(&imported, warning);
    *state.imported.lock().unwrap() = Some(imported);
    save(state, &SavedBuild::Pob { code, link });
    Ok(v)
}

/// The current build, kept across restarts (`build.json`).
#[derive(Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "lowercase")]
enum SavedBuild {
    Pob { code: String, link: Option<String> },
    Design { design: BuildDesign },
    /// Following a build that's in the game's Build Planner (its stage files).
    Planner { files: Vec<String> },
}

const SAVED_BUILD: &str = "build.json";

fn save(state: &AppState, saved: &SavedBuild) {
    if let Ok(text) = serde_json::to_string_pretty(saved) {
        let _ = crate::state::write_atomic(&state.data_dir.join(SAVED_BUILD), &text);
    }
}

/// Turns `design` into a staged build (paths computed, gems checked)
/// without making it active. The report says what didn't fit.
pub fn realize_design(state: &AppState, design: &BuildDesign) -> Result<(Imported, DesignReport), String> {
    let tree = load_tree(state)?;
    let data = crate::gamedata::load(state)?;
    let realized = realize(design, &tree, &data)?;
    let stages = plan_stages(&realized.build, Some(&tree));
    let imported = Imported {
        build: realized.build,
        stages,
        link: None,
        name: design.name.trim().to_owned(),
        source: "Lifeline".into(),
    };
    Ok((imported, realized.report))
}

/// Makes `design` the build the player follows (and keeps it across restarts).
pub fn activate(state: &AppState, design: &BuildDesign) -> Result<ImportView, String> {
    let (imported, _) = realize_design(state, design)?;
    let v = view(&imported, None);
    *state.imported.lock().unwrap() = Some(imported);
    save(state, &SavedBuild::Design { design: design.clone() });
    Ok(v)
}

/// Realizes and activates in one go (the chat path).
pub fn create(state: &AppState, design: &BuildDesign) -> Result<(DesignReport, ImportView), String> {
    let (imported, report) = realize_design(state, design)?;
    let v = view(&imported, None);
    *state.imported.lock().unwrap() = Some(imported);
    save(state, &SavedBuild::Design { design: design.clone() });
    Ok((report, v))
}

/// Restores the saved build at startup; returns its view.
pub fn restore(state: &AppState) -> Option<ImportView> {
    let text = std::fs::read_to_string(state.data_dir.join(SAVED_BUILD)).ok()?;
    match serde_json::from_str::<SavedBuild>(&text).ok()? {
        SavedBuild::Pob { code, link } => {
            let build = parse_xml(&decode(&code).ok()?).ok()?;
            let tree = load_tree(state).ok();
            let stages = plan_stages(&build, tree.as_deref());
            let name = build
                .ascend_class_name
                .clone()
                .or_else(|| build.class_name.clone())
                .unwrap_or_else(|| "Build".into());
            let imported = Imported {
                build,
                stages,
                link,
                name: format!("{name} (Lifeline)"),
                source: "Path of Building".into(),
            };
            let v = view(&imported, None);
            *state.imported.lock().unwrap() = Some(imported);
            Some(v)
        }
        SavedBuild::Design { design } => create(state, &design).ok().map(|(_, v)| v),
        SavedBuild::Planner { files } => follow_planner(state, &files).ok(),
    }
}

/// Follows a build that's in the game's Build Planner: its stage files
/// ("Act 1 - Name.build", "Act 2 - Name.build", …) become the active build.
pub fn follow_planner(state: &AppState, files: &[String]) -> Result<ImportView, String> {
    let dir = planner_dir().ok_or("Couldn't find the Build Planner folder.")?;
    let tree = load_tree(state)?;
    let data = crate::gamedata::load(state).ok();
    let mut stages = Vec::new();
    let mut name = String::new();
    for f in files {
        if f.contains(['/', '\\']) || !f.ends_with(".build") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(dir.join(f)) else { continue };
        let Ok(b) = build_planner::PlannerBuild::from_json(&text) else { continue };
        let (label, build_name) = lifeline_model::split_file_name(f);
        if build_name.len() > name.len() {
            name = build_name;
        }
        stages.push((if label.is_empty() { "Endgame".to_owned() } else { label }, b));
    }
    if stages.is_empty() {
        return Err("None of those planner files could be read.".into());
    }
    let build = lifeline_model::from_planner(&stages, &tree, data.as_deref());
    let plan = plan_stages(&build, Some(&tree));
    let imported = Imported {
        build,
        stages: plan,
        link: None,
        name,
        source: "In-game planner".into(),
    };
    let v = view(&imported, None);
    *state.imported.lock().unwrap() = Some(imported);
    save(state, &SavedBuild::Planner { files: files.to_vec() });
    Ok(v)
}

/// Writes one `.build` per chosen stage into the game's BuildPlanner folder.
pub fn write_stages(state: &AppState) -> Result<Vec<String>, String> {
    let tree = load_tree(state)?;
    let data = crate::gamedata::load(state).ok();
    let guard = state.imported.lock().unwrap();
    let imported = guard.as_ref().ok_or("Import a build first.")?;
    let dir = paths::user_dir()
        .map(|d| paths::build_planner_dir(&d))
        .ok_or("Couldn't find Documents/My Games/Path of Exile 2")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut written = Vec::new();
    for stage in imported.stages.iter().filter(|s| s.chosen) {
        let mut planner = to_planner_build(&imported.build, stage, &tree, &imported.name, imported.link.as_deref());
        // Supports the player can use at this stage: unique, cuttable, every socket filled.
        if let Some(data) = data.as_deref() {
            let weapon = lifeline_model::stage_weapon(&imported.build, stage.stage_key, data);
            lifeline_model::supports::fit_skills(&mut planner.skills, data, stage.stage_key, weapon);
            lifeline_model::supports::complete(&mut planner.skills, data, stage.stage_key);
        }
        let path = build_planner::write(&dir, &planner).map_err(|e| e.to_string())?;
        written.push(
            path.file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default(),
        );
    }
    Ok(written)
}

pub fn planner_files() -> Vec<String> {
    paths::user_dir()
        .map(|d| paths::build_planner_dir(&d))
        .and_then(|dir| build_planner::read_dir(&dir).ok())
        .map(|files| {
            files
                .into_iter()
                .map(|(p, b)| match b {
                    Ok(b) => b.name,
                    Err(_) => format!("{} (unreadable)", p.display()),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The game's Build Planner folder.
pub fn planner_dir() -> Option<std::path::PathBuf> {
    paths::user_dir().map(|d| paths::build_planner_dir(&d))
}

#[derive(Debug, Clone, Serialize)]
pub struct PlannerFile {
    pub file: String,
    pub name: String,
    pub author: Option<String>,
}

/// Every .build file in the planner folder, with its name and author.
pub fn planner_list() -> Vec<PlannerFile> {
    let Some(dir) = planner_dir() else {
        return Vec::new();
    };
    let mut out: Vec<PlannerFile> = build_planner::read_dir(&dir)
        .unwrap_or_default()
        .into_iter()
        .map(|(path, parsed)| {
            let file = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            match parsed {
                Ok(b) => PlannerFile { file, name: b.name, author: b.author },
                Err(_) => PlannerFile { name: format!("{file} (unreadable)"), file, author: None },
            }
        })
        .collect();
    out.sort_by_key(|a| a.file.to_lowercase());
    out
}

/// Deletes planner files by file name. Only plain `.build` names inside the
/// planner folder are accepted.
pub fn delete_planner_files(files: &[String]) -> Result<usize, String> {
    let dir = planner_dir().ok_or("Couldn't find the Build Planner folder.")?;
    let mut deleted = 0;
    for f in files {
        let plain = !f.contains(['/', '\\', ':']) && !f.contains("..") && f.ends_with(".build");
        if !plain {
            return Err(format!("Not a Build Planner file: {f}"));
        }
        let path = dir.join(f);
        if path.is_file() {
            std::fs::remove_file(&path).map_err(|e| format!("Couldn't delete {f}: {e}"))?;
            deleted += 1;
        }
    }
    Ok(deleted)
}

/// Opens the planner folder in Explorer.
pub fn open_planner_folder() -> Result<(), String> {
    let dir = planner_dir().ok_or("Couldn't find the Build Planner folder.")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::process::Command::new("explorer.exe").arg(&dir).spawn().map(|_| ()).map_err(|e| e.to_string())
}