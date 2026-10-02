//! Summarise a Path of Building code or share link (pobb.in, pastebin,
//! poe.ninja, maxroll /pob/) into exact game-data names, for research.
//! pob_summary <repoe dir> <tree data.json> <code | link>

use std::collections::{BTreeMap, HashMap};

use lifeline_data::{GameData, PassiveTree, REPOE_FILES};
use lifeline_pob::{decode, parse_xml, resolve, BuildSource};

fn get(url: &str) -> String {
    ureq::get(url)
        .header("User-Agent", "Lifeline/0.2 (build research)")
        .call()
        .unwrap_or_else(|e| panic!("download failed: {e}"))
        .body_mut()
        .read_to_string()
        .expect("body")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::path::Path::new(&args[0]);
    let tree = PassiveTree::from_json(&std::fs::read_to_string(&args[1]).unwrap()).unwrap();
    let files: HashMap<String, String> = REPOE_FILES
        .iter()
        .map(|f| (f.to_string(), std::fs::read_to_string(dir.join(format!("{f}.json"))).unwrap()))
        .collect();
    let data = GameData::parse("4.5.5.2", &files).unwrap();
    let input = args[2..].join(" ");
    let code = match resolve(&input) {
        BuildSource::Code(c) => c,
        BuildSource::CodeUrl(url) => get(&url),
        _ => panic!("unsupported link; use the guide's Path of Building link"),
    };
    let build = parse_xml(&decode(&code).expect("not a PoB code")).expect("not PoB2 XML");
    println!("Class: {:?}  Ascendancy: {:?}  Level: {:?}", build.class_name, build.ascend_class_name, build.level);
    for set in &build.skill_sets {
        println!("Skill set '{}':", set.title);
        for g in set.groups.iter().filter(|g| g.enabled && !g.gems.is_empty()) {
            let names: Vec<String> = g
                .gems
                .iter()
                .filter(|x| x.enabled)
                .map(|x| x.gem_id.as_deref().and_then(|id| data.gem_name(id)).unwrap_or(&x.name).to_owned())
                .collect();
            println!("  {}", names.join(" + "));
        }
    }
    for spec in &build.specs {
        let asc = spec.ascendancy_internal_id.as_deref();
        let mut notables = Vec::new();
        let mut keystones = Vec::new();
        let mut asc_nodes = Vec::new();
        for &n in &spec.nodes {
            let Some(base) = tree.node(n) else { continue };
            let shown = tree.node_for(n, asc).unwrap_or(base);
            if base.ascendancy_id.is_some() {
                if base.is_notable {
                    asc_nodes.push(shown.name.clone());
                }
            } else if base.is_keystone {
                keystones.push(shown.name.clone());
            } else if base.is_notable {
                notables.push(shown.name.clone());
            }
        }
        println!(
            "Tree '{}' ({} nodes): keystones [{}]; ascendancy [{}]; notables [{}]",
            spec.title,
            spec.nodes.len(),
            keystones.join(", "),
            asc_nodes.join(", "),
            notables.join(", ")
        );
    }
    let mut uniques: BTreeMap<String, ()> = BTreeMap::new();
    for raw in build.items.values() {
        let mut lines = raw.lines().map(str::trim).filter(|l| !l.is_empty());
        if lines.any(|l| l == "Rarity: UNIQUE") {
            if let Some(name) = lines.next() {
                uniques.insert(name.to_owned(), ());
            }
        }
    }
    println!("Uniques: {}", uniques.keys().cloned().collect::<Vec<_>>().join(", "));
}