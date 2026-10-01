//! Build import, tree data and writing stages to the in-game Build Planner.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use polr_data::{PassiveTree, TREE_EXPORT_URL};
use polr_gamefiles::{build_planner, paths};
use polr_model::{plan_stages, to_planner_build, SpecStage};
use polr_pob::{decode, parse_xml, resolve, BuildSource};
use serde::Serialize;

use crate::state::{AppState, Imported};

const USER_AGENT: &str = "PathOfLeastResistance/0.1 (personal PoE2 companion)";
const TREE_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);

pub fn http_get(url: &str) -> Result<String, String> {
    let mut response = ureq::get(url)
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
        link,
        name: format!("{name} (PoLR)"),
    };
    let v = view(&imported, warning);
    *state.imported.lock().unwrap() = Some(imported);
    Ok(v)
}

/// Writes one `.build` per chosen stage into the game's BuildPlanner folder.
pub fn write_stages(state: &AppState) -> Result<Vec<String>, String> {
    let tree = load_tree(state)?;
    let guard = state.imported.lock().unwrap();
    let imported = guard.as_ref().ok_or("Import a build first.")?;
    let dir = paths::user_dir()
        .map(|d| paths::build_planner_dir(&d))
        .ok_or("Couldn't find Documents/My Games/Path of Exile 2")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut written = Vec::new();
    for stage in imported.stages.iter().filter(|s| s.chosen) {
        let planner = to_planner_build(&imported.build, stage, &tree, &imported.name, imported.link.as_deref());
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
