//! The passive tree as a picture: the whole tree's layout (sent once), and
//! the followed build's plan for a stage on top of it — what's taken, what
//! to take next and in which order, what's off-plan, and which stat to pick
//! on every "+5 to any Attribute" node.

use std::collections::HashSet;

use lifeline_model::attributes::{self, Attr};
use serde_json::{json, Value};

use crate::state::AppState;

/// Node kinds the UI draws differently.
fn kind(n: &lifeline_data::TreeNode) -> u8 {
    if n.class_start_index.is_some() {
        4
    } else if n.is_keystone {
        2
    } else if n.is_notable {
        1
    } else if n.is_jewel_socket {
        3
    } else if n.is_generic_attribute {
        5
    } else {
        0
    }
}

/// Every main-tree node and connection: `nodes` = [skill, x, y, kind, name],
/// `edges` = [from, to] or [from, to, cx, cy] for arcs. Ascendancy nodes are
/// listed separately in the plan.
pub fn layout(state: &AppState) -> Result<Value, String> {
    let tree = crate::builds::load_tree(state)?;
    let on_main = |n: &lifeline_data::TreeNode| n.ascendancy_id.is_none() && n.x.is_some() && n.y.is_some() && !n.is_blighted;
    let mut nodes: Vec<Value> = Vec::new();
    let mut shown = HashSet::new();
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (skill, n) in tree.nodes() {
        if !on_main(n) {
            continue;
        }
        let (x, y) = (n.x.unwrap_or_default(), n.y.unwrap_or_default());
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
        shown.insert(skill);
        nodes.push(json!([skill, x.round(), y.round(), kind(n), n.name]));
    }
    let edges: Vec<Value> = tree
        .edges()
        .iter()
        .filter(|e| shown.contains(&e.from) && shown.contains(&e.to))
        .map(|e| match e.center {
            Some((cx, cy)) => json!([e.from, e.to, cx.round(), cy.round()]),
            None => json!([e.from, e.to]),
        })
        .collect();
    Ok(json!({"bounds": [min_x, min_y, max_x, max_y], "nodes": nodes, "edges": edges}))
}

fn attr_name(a: Attr) -> &'static str {
    match a {
        Attr::Str => "Strength",
        Attr::Dex => "Dexterity",
        Attr::Int => "Intelligence",
    }
}

/// The followed build's plan for `stage` (a label like "Act 2"; default:
/// the stage the character is in).
pub fn plan(state: &AppState, stage: Option<&str>) -> Result<Value, String> {
    let guard = state.imported.lock().unwrap();
    let imported = guard.as_ref().ok_or("No build is being followed yet.")?;
    let tree = crate::builds::load_tree(state)?;
    let data = crate::gamedata::load(state).ok();
    let c = state.character.lock().unwrap().clone();
    let mut chosen: Vec<_> = imported.stages.iter().filter(|s| s.chosen).collect();
    chosen.sort_by_key(|s| s.stage_key);
    let here = crate::ai::current_stage(c.act, c.area_level);
    let current = stage
        .and_then(|label| chosen.iter().find(|s| s.stage == label))
        .or_else(|| chosen.iter().find(|s| s.stage_key == here))
        .or_else(|| chosen.iter().filter(|s| s.stage_key <= here).max_by_key(|s| s.stage_key))
        .or(chosen.first())
        .copied()
        .ok_or("This build has no stages.")?;
    let spec = &imported.build.specs[current.spec_index];
    let asc = spec.ascendancy_internal_id.as_deref();

    let main = |n: &u32| tree.node(*n).is_some_and(|x| x.ascendancy_id.is_none() && x.class_start_index.is_none());
    let planned: HashSet<u32> = spec.nodes.iter().copied().filter(main).collect();
    // Planned in the stage before this one (drawn as "already done by now").
    let earlier: HashSet<u32> = chosen
        .iter()
        .filter(|s| s.stage_key < current.stage_key)
        .max_by_key(|s| s.stage_key)
        .map(|s| imported.build.specs[s.spec_index].nodes.iter().copied().filter(main).collect())
        .unwrap_or_default();
    let allocated: HashSet<u32> = c.allocated.iter().filter_map(|id| tree.node_by_id(id)).collect();

    let start = imported.build.class_name.as_deref().and_then(|cl| tree.class_start(cl));
    let order = attributes::allocation_order(&tree, start, &planned);
    let needs = attributes::stage_needs(&tree, &imported.build, current.spec_index, current.stage_key, data.as_deref());
    let attrs = attributes::plan(&tree, &order, &needs);

    let detail = |n: u32| {
        let node = tree.node_for(n, asc);
        let pick = attrs.choices.get(&n).copied();
        json!({
            "skill": n,
            "name": match pick { Some(a) => format!("Attribute → {}", attr_name(a)), None => node.map(|x| x.name.clone()).unwrap_or_default() },
            "stats": match pick { Some(a) => vec![format!("+5 to {}", attr_name(a))], None => node.map(|x| x.stats.clone()).unwrap_or_default() },
            "notable": node.is_some_and(|x| x.is_notable || x.is_keystone),
            "keystone": node.is_some_and(|x| x.is_keystone),
            "attr": pick,
        })
    };
    let next: Vec<Value> = order.iter().copied().filter(|n| !allocated.contains(n)).map(detail).collect();
    let off_plan: Vec<u32> = allocated.iter().copied().filter(|n| main(n) && !planned.contains(n)).collect();
    let ascendancy: Vec<Value> = spec
        .nodes
        .iter()
        .copied()
        .filter(|n| tree.is_ascendancy_point(*n))
        .filter_map(|n| tree.node_for(n, asc).map(|x| (n, x)))
        .map(|(n, x)| json!({"name": x.name, "stats": x.stats, "notable": x.is_notable, "taken": allocated.contains(&n)}))
        .collect();
    let all_details: serde_json::Map<String, Value> = planned
        .iter()
        .chain(&off_plan)
        .map(|&n| (n.to_string(), detail(n)))
        .collect();

    Ok(json!({
        "build": imported.name,
        "stage": current.stage,
        "stages": chosen.iter().map(|s| s.stage.clone()).collect::<Vec<_>>(),
        "start": start,
        "planned": planned,
        "earlier": earlier,
        "allocated": allocated.iter().filter(|n| main(n)).collect::<Vec<_>>(),
        "off_plan": off_plan,
        "next": next,
        "attributes": attrs.summary.iter().map(|s| json!({"attr": attr_name(s.attr), "short": s.attr, "nodes": s.nodes, "why": s.why})).collect::<Vec<_>>(),
        "ascendancy": ascendancy,
        "details": all_details,
        "allocated_seen": !c.allocated.is_empty(),
    }))
}
