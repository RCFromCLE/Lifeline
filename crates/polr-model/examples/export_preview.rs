//! Preview the Build Planner files a PoB build would produce, without writing
//! into the game folder.
//!
//! cargo run -p polr-model --example export_preview -- <tree data.json> <file with PoB code>

use polr_data::PassiveTree;
use polr_gamefiles::build_planner::{file_name_for, gem_short_name};
use polr_model::{plan_stages, to_planner_build};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [tree_path, code_path] = args.as_slice() else {
        eprintln!("usage: export_preview <tree data.json> <file with PoB code>");
        return;
    };
    let tree = PassiveTree::from_json(&std::fs::read_to_string(tree_path).expect("tree file")).expect("tree json");
    let code = std::fs::read_to_string(code_path).expect("code file");
    let build = polr_pob::parse_xml(&polr_pob::decode(&code).expect("decode")).expect("xml");

    for stage in plan_stages(&build, Some(&tree)).iter().filter(|s| s.chosen) {
        let pb = to_planner_build(&build, stage, &tree, "Deadeye (PoLR)", None);
        let ws = pb.passives.iter().filter(|p| p.weapon_set.is_some()).count();
        let asc = pb.passives.iter().filter(|p| p.id.starts_with("Ascendancy")).count();
        println!(
            "{:<40} spec \"{}\" ≈lvl {:>3} | passives {:>3} (asc {asc}, weapon-set {ws}) | skills {:>2} | slots {:>2}",
            file_name_for(&pb.name),
            stage.title,
            stage.estimated_level,
            pb.passives.len(),
            pb.skills.len(),
            pb.inventory_slots.len()
        );
        if let Some(s) = pb.skills.first() {
            let sup: Vec<&str> = s.support_skills.iter().map(|x| gem_short_name(&x.id)).collect();
            println!("    first skill: {} [{}]", gem_short_name(&s.id), sup.join(", "));
        }
        if let Some(slot) = pb.inventory_slots.first() {
            let text = slot
                .unique_name
                .clone()
                .or(slot.additional_text.clone())
                .unwrap_or_default();
            println!(
                "    {}: {}",
                slot.inventory_id,
                text.lines().take(3).collect::<Vec<_>>().join(" | ")
            );
        }
    }
}
