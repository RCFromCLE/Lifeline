//! Parse real RePoE files + tree export and run sample lookups.
//! cargo run -p polr-data --example data_check -- <repoe dir> <tree data.json>

use std::collections::HashMap;

use polr_data::{GameData, PassiveTree, REPOE_FILES};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::path::Path::new(&args[0]);
    let files: HashMap<String, String> = REPOE_FILES
        .iter()
        .map(|f| {
            (
                f.to_string(),
                std::fs::read_to_string(dir.join(format!("{f}.json"))).unwrap(),
            )
        })
        .collect();
    let t = std::time::Instant::now();
    let d = GameData::parse("4.5.5.2", &files).unwrap();
    println!(
        "parsed in {:?}: {} gems, {} bases, {} mods, {} uniques, {} areas",
        t.elapsed(),
        d.gems.len(),
        d.bases.len(),
        d.mods.len(),
        d.uniques.len(),
        d.areas.len()
    );
    let spark = d.find_gems("spark", 3);
    println!(
        "gems 'spark': {:?}",
        spark
            .iter()
            .map(|g| (&g.name, &g.kind, &g.attribute))
            .collect::<Vec<_>>()
    );
    println!("  desc: {:?}", spark[0].description);
    println!(
        "  supports shortlist: {:?}",
        d.supports_for(spark[0], 8).iter().map(|g| &g.name).collect::<Vec<_>>()
    );
    println!(
        "SupportGemMartialTempo → {:?}",
        d.gem_name("Metadata/Items/Gems/SupportGemMartialTempo")
    );
    println!(
        "bases 'spear': {:?}",
        d.find_bases("spear", 5)
            .iter()
            .map(|b| (&b.name, b.drop_level))
            .collect::<Vec<_>>()
    );
    println!(
        "mods 'cold resistance' on boots: {:?}",
        d.find_mods("cold resistance", Some("boots"), 4)
            .iter()
            .map(|m| (&m.text, m.required_level))
            .collect::<Vec<_>>()
    );
    println!(
        "areas 'clearfell': {:?}",
        d.find_areas("clearfell", 3)
            .iter()
            .map(|a| (&a.id, &a.name, a.area_level))
            .collect::<Vec<_>>()
    );
    println!(
        "uniques 'black': {:?}",
        d.find_uniques("black", 3)
            .iter()
            .map(|u| (&u.name, &u.item_class))
            .collect::<Vec<_>>()
    );
    let tree = PassiveTree::from_json(&std::fs::read_to_string(&args[1]).unwrap()).unwrap();
    println!(
        "passives 'cold resistance': {:?}",
        tree.find_passives("cold resistance", 4)
            .iter()
            .map(|n| (&n.id, &n.name, n.is_notable))
            .collect::<Vec<_>>()
    );
}
