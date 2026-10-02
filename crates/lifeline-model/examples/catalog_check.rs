//! Check a build catalog file (an array of archetypes) against game data.
//! cargo run -p lifeline-model --example catalog_check -- <repoe dir> <tree data.json> <catalog.json>

use std::collections::{HashMap, HashSet};

use lifeline_data::{GameData, PassiveTree, REPOE_FILES};
use lifeline_model::Archetype;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::path::Path::new(&args[0]);
    let tree = PassiveTree::from_json(&std::fs::read_to_string(&args[1]).expect("tree file")).expect("tree json");
    let files: HashMap<String, String> = REPOE_FILES
        .iter()
        .map(|f| (f.to_string(), std::fs::read_to_string(dir.join(format!("{f}.json"))).expect("repoe file")))
        .collect();
    let data = GameData::parse("4.5.5.2", &files).expect("repoe");
    let text = std::fs::read_to_string(&args[2]).expect("catalog file");
    let list: Vec<Archetype> = match serde_json::from_str(&text) {
        Ok(l) => l,
        Err(e) => {
            println!("JSON doesn't match the schema: {e}");
            std::process::exit(2);
        }
    };
    let mut ids = HashSet::new();
    let mut bad = 0;
    for a in &list {
        let mut p = lifeline_model::catalog::check(a, &tree, &data);
        if !ids.insert(a.id.clone()) {
            p.push("duplicate id".into());
        }
        if p.is_empty() {
            println!("OK   {} ({} {})", a.id, a.class, a.ascendancy);
        } else {
            bad += 1;
            println!("FIX  {}:", a.id);
            for x in p {
                println!("       - {x}");
            }
        }
    }
    println!("{} archetypes, {} need fixes", list.len(), bad);
    if bad > 0 {
        std::process::exit(1);
    }
}