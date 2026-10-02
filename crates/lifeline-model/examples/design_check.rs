//! Realize a build design against real data, or list notables near a class.
//! cargo run -p lifeline-model --example design_check -- <repoe dir> <tree data.json> near <Class> [Ascendancy]
//! cargo run -p lifeline-model --example design_check -- <repoe dir> <tree data.json> design <design.json>

use std::collections::{HashMap, HashSet};

use lifeline_data::{GameData, PassiveTree, REPOE_FILES};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::path::Path::new(&args[0]);
    let tree = PassiveTree::from_json(&std::fs::read_to_string(&args[1]).unwrap()).unwrap();
    match args[2].as_str() {
        "near" => {
            let start = tree.class_start(&args[3]).expect("class");
            let set: HashSet<u32> = [start].into();
            let usable = |_: u32, n: &lifeline_data::TreeNode| {
                !n.id.is_empty() && n.ascendancy_id.is_none() && n.class_start_index.is_none()
            };
            let mut found: Vec<(usize, String, String)> = Vec::new();
            for id in 0..70000u32 {
                let Some(n) = tree.node(id) else { continue };
                if !(n.is_notable || n.is_keystone) || n.ascendancy_id.is_some() {
                    continue;
                }
                if let Some(path) = tree.path_from(&set, id, &usable) {
                    if path.len() <= 16 {
                        found.push((path.len(), n.name.clone(), n.stats.join(" / ")));
                    }
                }
            }
            found.sort();
            for (cost, name, stats) in found.iter().take(40) {
                println!("{cost:>2}  {name}: {stats}");
            }
            if let Some(asc) = args.get(4).and_then(|a| tree.ascendancy(&args[3], a)) {
                println!("--- {} ({})", asc.name, asc.id);
                for id in 0..70000u32 {
                    if let Some(n) = tree.node(id).filter(|n| n.ascendancy_id.as_deref() == Some(&asc.id) && n.is_notable) {
                        println!("    {}: {}", n.name, n.stats.join(" / "));
                    }
                }
            }
        }
        "design" => {
            let files: HashMap<String, String> = REPOE_FILES
                .iter()
                .map(|f| (f.to_string(), std::fs::read_to_string(dir.join(format!("{f}.json"))).unwrap()))
                .collect();
            let data = GameData::parse("4.5.5.2", &files).unwrap();
            let design: lifeline_model::BuildDesign =
                serde_json::from_str(&std::fs::read_to_string(&args[3]).unwrap()).unwrap();
            match lifeline_model::realize(&design, &tree, &data) {
                Ok(r) => {
                    println!("{}", serde_json::to_string_pretty(&r.report).unwrap());
                    let stages = lifeline_model::plan_stages(&r.build, Some(&tree));
                    for s in &stages {
                        let planner = lifeline_model::to_planner_build(&r.build, s, &tree, &design.name, None);
                        println!(
                            "{} → est. level {}, {} passives, {} skills, {} item hints, file '{}'",
                            s.stage,
                            s.estimated_level,
                            planner.passives.len(),
                            planner.skills.len(),
                            planner.inventory_slots.len(),
                            planner.name
                        );
                    }
                }
                Err(e) => println!("ERROR {e}"),
            }
        }
        other => eprintln!("unknown mode {other}"),
    }
}