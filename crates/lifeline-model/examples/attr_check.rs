//! Prints a planner build's stage order and attribute picks.
//! `cargo run --example attr_check -- <tree.json> <repoe dir> <planner .build>…`
use std::collections::{HashMap, HashSet};

use lifeline_data::{GameData, PassiveTree, REPOE_FILES};
use lifeline_model::attributes;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let tree = PassiveTree::from_json(&std::fs::read_to_string(&args[0]).unwrap()).unwrap();
    let files: HashMap<String, String> = REPOE_FILES
        .iter()
        .filter_map(|n| Some((n.to_string(), std::fs::read_to_string(format!("{}/{n}.json", args[1])).ok()?)))
        .collect();
    let data = GameData::parse("local", &files).unwrap();
    let stages: Vec<_> = args[2..]
        .iter()
        .map(|p| {
            let name = std::path::Path::new(p).file_name().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(p).unwrap();
            let build = serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap();
            (lifeline_model::split_file_name(&name).0, build)
        })
        .collect();
    let build = lifeline_model::from_planner(&stages, &tree, Some(&data));
    let spec = &build.specs[0];
    let main: HashSet<u32> = spec.nodes.iter().copied().filter(|n| tree.node(*n).is_some_and(|x| x.ascendancy_id.is_none() && x.class_start_index.is_none())).collect();
    let start = build.class_name.as_deref().and_then(|c| tree.class_start(c));
    let order = attributes::allocation_order(&tree, start, &main);
    let stage = lifeline_pob::Stage::Act(1);
    let needs = attributes::stage_needs(&tree, &build, 0, stage, Some(&data));
    println!("class {:?} start {start:?} stage {stage:?}", build.class_name);
    println!("weights str {:.2} dex {:.2} int {:.2}", needs.weight(attributes::Attr::Str), needs.weight(attributes::Attr::Dex), needs.weight(attributes::Attr::Int));
    let plan = attributes::plan(&tree, &order, &needs);
    for n in &order {
        let node = tree.node(*n).unwrap();
        println!("{n:>6} {:<28} {:?}", node.name, plan.choices.get(n));
    }
}
