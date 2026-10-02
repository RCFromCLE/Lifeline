//! Look up exact game-data names (for researching builds).
//! lookup <repoe dir> <tree data.json> gems <words>          skill/support gems by name
//! lookup <repoe dir> <tree data.json> tagged <tag>          skill gems with a tag (e.g. lightning, minion, spear)
//! lookup <repoe dir> <tree data.json> supports <skill>      supports the game allows on a skill
//! lookup <repoe dir> <tree data.json> passive <words>       passives whose name/stats contain the words
//! lookup <repoe dir> <tree data.json> near <Class>          notables/keystones near the class start (path cost)
//! lookup <repoe dir> <tree data.json> asc <Class> <Asc>     every passive of an ascendancy
//! lookup <repoe dir> <tree data.json> unique <words>        uniques by name

use std::collections::{HashMap, HashSet};

use polr_data::{GameData, PassiveTree, TreeNode, REPOE_FILES};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::path::Path::new(&args[0]);
    let tree = PassiveTree::from_json(&std::fs::read_to_string(&args[1]).unwrap()).unwrap();
    let files: HashMap<String, String> = REPOE_FILES
        .iter()
        .map(|f| (f.to_string(), std::fs::read_to_string(dir.join(format!("{f}.json"))).unwrap()))
        .collect();
    let data = GameData::parse("4.5.5.2", &files).unwrap();
    let rest = args[3..].join(" ");
    match args[2].as_str() {
        "gems" => {
            for g in data.find_gems(&rest, 25) {
                println!("{} [{}] {} — {}", g.name, g.kind, g.tags.join(","), g.description.clone().unwrap_or_default().replace('\n', " "));
            }
        }
        "tagged" => {
            let tag = rest.to_lowercase();
            for g in data.gems.iter().filter(|g| g.kind != "support" && g.tags.iter().any(|t| t.to_lowercase() == tag)) {
                println!("{} [{}] {}", g.name, g.kind, g.tags.join(","));
            }
        }
        "supports" => {
            let gem = data.gem_named(&rest).or_else(|| data.find_gems(&rest, 1).into_iter().find(|g| g.kind != "support")).expect("skill");
            println!("{} types: {}", gem.name, gem.skill_types.join(","));
            for s in data.supports_for(gem, 2000) {
                println!("  {} — {}", s.name, s.support_effects.join(" / ").replace('\n', " "));
            }
        }
        "passive" => {
            for n in tree.find_passives(&rest, 40) {
                let kind = if n.is_keystone { "keystone" } else if n.is_notable { "notable" } else { "small" };
                println!("{} [{kind}{}] {}", n.name, n.ascendancy_id.as_deref().map(|a| format!(", {a}")).unwrap_or_default(), n.stats.join(" / ").replace('\n', " "));
            }
        }
        "near" => {
            let start = tree.class_start(&args[3]).expect("class");
            let set: HashSet<u32> = [start].into();
            let usable = |_: u32, n: &TreeNode| !n.id.is_empty() && n.ascendancy_id.is_none() && n.class_start_index.is_none();
            let mut found = Vec::new();
            for id in 0..70000u32 {
                let Some(n) = tree.node(id) else { continue };
                if (n.is_notable || n.is_keystone) && n.ascendancy_id.is_none() {
                    if let Some(path) = tree.path_from(&set, id, &usable) {
                        found.push((path.len(), n.name.clone(), n.is_keystone, n.stats.join(" / ").replace('\n', " ")));
                    }
                }
            }
            found.sort();
            for (cost, name, ks, stats) in found.iter().take(150) {
                println!("{cost:>3} {}{name}: {stats}", if *ks { "KEYSTONE " } else { "" });
            }
        }
        "asc" => {
            let asc = tree.ascendancy(&args[3], &args[4..].join(" ")).expect("ascendancy");
            println!("{} ({})", asc.name, asc.id);
            for id in 0..70000u32 {
                if let Some(n) = tree.node(id).filter(|n| n.ascendancy_id.as_deref() == Some(&asc.id) && !n.name.is_empty()) {
                    let kind = if n.is_notable { "NOTABLE" } else if n.is_ascendancy_start { "start" } else { "small" };
                    println!("  [{kind}] {}: {}", n.name, n.stats.join(" / ").replace('\n', " "));
                }
            }
        }
        "unique" => {
            for u in data.find_uniques(&rest, 15) {
                println!("{} ({})", u.name, u.item_class);
            }
        }
        other => eprintln!("unknown mode {other}"),
    }
}