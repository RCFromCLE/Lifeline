//! How the character lines up with the build being followed, for the stage
//! that fits its level: passives allocated (from the game log) against the
//! plan, recorded gear against the stage's gear goals, and the planned
//! skills and supports (the game doesn't log gems, so those are listed for
//! the player and the AI to check).

use std::collections::HashSet;

use serde_json::{json, Value};

use crate::state::AppState;

/// Gear goal slot (PoB name) → the item classes a recorded item may have.
fn classes_for(slot: &str) -> &'static [&'static str] {
    match slot {
        "Helmet" => &["Helmets"],
        "Body Armour" => &["Body Armours"],
        "Gloves" => &["Gloves"],
        "Boots" => &["Boots"],
        "Amulet" => &["Amulets"],
        "Ring 1" | "Ring 2" => &["Rings"],
        "Belt" => &["Belts"],
        "Weapon 2" => &["Shields", "Bucklers", "Quivers", "Foci", "Sceptres", "Wands"],
        _ => &[],
    }
}

/// First line of item text after the header lines (the item's name or base).
fn item_title(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("--------")).collect();
    let start = lines.iter().position(|l| l.starts_with("Rarity:")).map_or(0, |i| i + 1);
    lines.get(start).copied().unwrap_or("item").to_owned()
}

pub fn alignment(state: &AppState) -> Value {
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
    let have: Vec<(u32, &String)> = planned.iter().filter(|(_, id)| c.allocated.contains(id)).map(|(n, id)| (*n, id)).collect();
    let mut missing: Vec<(bool, String)> = planned
        .iter()
        .filter(|(_, id)| !c.allocated.contains(id))
        .map(|(n, _)| (tree.node(*n).is_some_and(|x| x.is_notable || x.is_keystone), name_of(*n)))
        .filter(|(_, name)| !name.is_empty())
        .collect();
    missing.sort_by_key(|(notable, _)| !notable);
    let off_plan: Vec<String> = c
        .allocated
        .iter()
        .filter(|id| !all_planned.contains(*id) && !planned_ids.contains(id.as_str()))
        .filter_map(|id| tree.node_by_id(id).map(name_of))
        .filter(|n| !n.is_empty())
        .collect();

    // Skills planned for this stage (names), to compare with what's socketed.
    let skills: Vec<Value> = lifeline_model::skills_for_stage(&imported.build, current.stage_key)
        .iter()
        .map(|s| {
            json!({
                "skill": crate::gamedata::gem_display(state, &s.id),
                "supports": s.support_skills.iter().map(|x| crate::gamedata::gem_display(state, &x.id)).collect::<Vec<_>>(),
            })
        })
        .collect();

    // Gear goals vs recorded items.
    let equipped = state.equipped.lock().unwrap().clone();
    let item_set = imported
        .build
        .item_sets
        .iter()
        .find(|s| s.title == spec.title)
        .or(imported.build.item_sets.first());
    let gear: Vec<Value> = item_set
        .map(|set| {
            set.slots
                .iter()
                .filter(|s| !s.slot.contains("Swap") && !s.slot.starts_with("Flask") && !s.slot.starts_with("Charm"))
                .map(|s| {
                    let goal = imported.build.items.get(&s.item_id).cloned().unwrap_or_default();
                    let goal_lines: Vec<&str> = goal.lines().skip_while(|l| !l.starts_with("Implicits:")).skip(1).collect();
                    let unique = goal.starts_with("Rarity: UNIQUE").then(|| goal.lines().nth(1).unwrap_or_default().to_owned());
                    let worn = classes_for(&s.slot)
                        .iter()
                        .find_map(|cl| equipped.get(*cl))
                        .or_else(|| {
                            (s.slot == "Weapon 1")
                                .then(|| equipped.iter().find(|(k, _)| !["Helmets", "Body Armours", "Gloves", "Boots", "Amulets", "Rings", "Belts", "Shields", "Bucklers", "Quivers", "Foci"].contains(&k.as_str())).map(|(_, v)| v))
                                .flatten()
                        })
                        .map(|t| item_title(t));
                    json!({"slot": s.slot, "goal_unique": unique, "goal": goal_lines, "recorded": worn})
                })
                .collect()
        })
        .unwrap_or_default();

    let done = have.len();
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
            "missing": missing.iter().map(|(n, name)| json!({"name": name, "notable": n})).collect::<Vec<_>>(),
            "off_plan": off_plan,
            "allocated_total": c.allocated.len(),
        },
        "skills": skills,
        "gear": gear,
        "note": "Allocated passives come from the game log (only allocations Lifeline has seen). The game doesn't log gems; ask the player what's socketed.",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_titles_and_slot_classes() {
        assert_eq!(item_title("Item Class: Boots\nRarity: Rare\nGale Stride\nThreaded Shoes\n--------\n+20 life"), "Gale Stride");
        assert_eq!(classes_for("Ring 2"), &["Rings"]);
        assert!(classes_for("Weapon 1").is_empty());
    }
}
