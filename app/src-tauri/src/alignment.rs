//! How the character lines up with the build being followed, for the stage
//! that fits its level: passives allocated (from the game log) against the
//! plan, and the skills and supports planned for the stage (the game doesn't
//! log gems, so those are the plan to socket).

use std::collections::HashSet;

use serde_json::{json, Value};

use crate::state::AppState;

/// Whether a passive (its game id, as the log names it) is in the followed
/// build's plan, in any stage.
pub fn on_plan(state: &AppState, id: &str) -> bool {
    let Ok(tree) = crate::builds::load_tree(state) else { return false };
    let guard = state.imported.lock().unwrap();
    let Some(imported) = guard.as_ref() else { return false };
    imported.build.specs.iter().any(|s| {
        s.nodes
            .iter()
            .any(|&n| tree.node_for(n, s.ascendancy_internal_id.as_deref()).is_some_and(|x| x.id == id))
    })
}

pub fn alignment(state: &AppState) -> Value {
    // Load (maybe download) game data before taking the build lock.
    let data = crate::gamedata::load(state).ok();
    let guard = state.imported.lock().unwrap();
    let Some(imported) = guard.as_ref() else {
        return json!({"build": null, "note": "No build is being followed yet. Pick one in Builds."});
    };
    let Some(tree) = state.tree.lock().unwrap().clone() else {
        return json!({"build": imported.name, "note": "Passive tree data isn't loaded yet."});
    };
    let c = state.character.lock().unwrap().clone();
    let stage = crate::ai::current_stage(c.act, c.area_level);
    let chosen: Vec<_> = imported.stages.iter().filter(|s| s.chosen).collect();
    let current = chosen
        .iter()
        .find(|s| s.stage_key == stage)
        .or_else(|| chosen.iter().filter(|s| s.stage_key <= stage).max_by_key(|s| s.stage_key))
        .or(chosen.first())
        .copied();
    let Some(current) = current else {
        return json!({"build": imported.name, "note": "This build has no stages."});
    };
    let spec = &imported.build.specs[current.spec_index];
    let asc = spec.ascendancy_internal_id.as_deref();

    // Passives: planned for this stage vs allocated in game.
    let name_of = |n: u32| tree.node_for(n, asc).map(|x| x.name.clone()).unwrap_or_default();
    let planned: Vec<(u32, String)> = spec
        .nodes
        .iter()
        .filter(|&&n| tree.is_plannable(n))
        .filter_map(|&n| tree.node_for(n, asc).map(|x| (n, x.id.clone())))
        .collect();
    let planned_ids: HashSet<&str> = planned.iter().map(|(_, id)| id.as_str()).collect();
    let all_planned: HashSet<String> = imported
        .build
        .specs
        .iter()
        .flat_map(|s| s.nodes.iter().filter_map(|&n| tree.node_for(n, s.ascendancy_internal_id.as_deref()).map(|x| x.id.clone())))
        .collect();
    let done = planned.iter().filter(|(_, id)| c.allocated.contains(id)).count();
    // In the order they're taken (outward from the class start), with the
    // stat to pick on each "+5 to any Attribute" node.
    let main_planned: HashSet<u32> = planned
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| tree.node(*n).is_some_and(|x| x.ascendancy_id.is_none()))
        .collect();
    let start = imported.build.class_name.as_deref().and_then(|cl| tree.class_start(cl));
    let order = lifeline_model::attributes::allocation_order(&tree, start, &main_planned);
    let needs = lifeline_model::attributes::stage_needs(&tree, &imported.build, current.stage_key, data.as_deref());
    let picks = lifeline_model::attributes::plan(&tree, &order, &needs).choices;
    let id_of = |n: u32| planned.iter().find(|(m, _)| *m == n).map(|(_, id)| id.clone()).unwrap_or_default();
    // Each with its kind: "keystone", "notable" (a major passive), or "minor".
    let missing: Vec<(&str, String)> = order
        .iter()
        .chain(planned.iter().map(|(n, _)| n).filter(|n| !main_planned.contains(n)))
        .copied()
        .filter(|n| !c.allocated.contains(&id_of(*n)))
        .map(|n| {
            let kind = match tree.node(n) {
                Some(x) if x.is_keystone => "keystone",
                Some(x) if x.is_notable => "notable",
                _ => "minor",
            };
            let name = match picks.get(&n) {
                Some(lifeline_model::attributes::Attr::Str) => "Attribute → Strength".to_owned(),
                Some(lifeline_model::attributes::Attr::Dex) => "Attribute → Dexterity".to_owned(),
                Some(lifeline_model::attributes::Attr::Int) => "Attribute → Intelligence".to_owned(),
                None => name_of(n),
            };
            (kind, name)
        })
        .filter(|(_, name)| !name.is_empty())
        .collect();
    let off_plan: Vec<String> = c
        .allocated
        .iter()
        .filter(|id| !all_planned.contains(*id) && !planned_ids.contains(id.as_str()))
        .filter_map(|id| tree.node_by_id(id).map(name_of))
        .filter(|n| !n.is_empty())
        .collect();

    // Skills planned for this stage, with supports the player can use by then.
    let skills: Vec<Value> = lifeline_model::usable_skills(&imported.build, current.stage_key, data.as_deref())
        .iter()
        .map(|s| {
            json!({
                "skill": crate::gamedata::gem_display(state, &s.id),
                "attr": crate::gamedata::gem_attribute(state, &s.id),
                "supports": s.support_skills.iter().map(|x| crate::gamedata::gem_display(state, &x.id)).collect::<Vec<_>>(),
                "support_attrs": s.support_skills.iter().map(|x| crate::gamedata::gem_attribute(state, &x.id)).collect::<Vec<_>>(),
            })
        })
        .collect();

    let total = planned.len();
    json!({
        "build": imported.name,
        "source": imported.source,
        "stage": current.stage,
        "character": {"name": c.name, "class": c.class, "level": c.level, "act": c.act},
        "passives": {
            "planned": total,
            "allocated_of_plan": done,
            "percent": (done * 100).checked_div(total).unwrap_or(0),
            "missing": missing.iter().map(|(kind, name)| json!({"name": name, "kind": kind, "notable": *kind != "minor"})).collect::<Vec<_>>(),
            "off_plan": off_plan,
            "allocated_total": c.allocated.len(),
        },
        "skills": skills,
        "note": "Allocated passives come from the game log (only allocations Lifeline has seen). The game doesn't log gems, so the skills are the plan to socket.",
    })
}
