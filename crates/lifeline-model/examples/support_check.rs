//! The supports each skill would get in each planner file; with `--write`
//! as the first argument, also saves them back (the game reloads the folder).
//! `cargo run --example support_check -- [--write] <repoe dir> <planner .build>…`
use std::collections::HashMap;

use lifeline_data::{GameData, REPOE_FILES};
use lifeline_gamefiles::build_planner::PlannerBuild;
use lifeline_pob::{classify_title, Stage, StageHint};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let write = args.first().is_some_and(|a| a == "--write");
    if write {
        args.remove(0);
    }
    let files: HashMap<String, String> = REPOE_FILES
        .iter()
        .filter_map(|n| Some((n.to_string(), std::fs::read_to_string(format!("{}/{n}.json", args[0])).ok()?)))
        .collect();
    let data = GameData::parse("local", &files).unwrap();
    // The build's weapon from whichever file names it.
    let fallback = args[1..].iter().find_map(|p| {
        let b = PlannerBuild::from_json(&std::fs::read_to_string(p).ok()?).ok()?;
        let t = b.inventory_slots.iter().find(|s| s.inventory_id == "Weapon1")?.additional_text.clone()?;
        lifeline_model::supports::weapon_type(&t, &data)
    });
    for path in &args[1..] {
        let name = std::path::Path::new(path).file_name().unwrap().to_string_lossy().into_owned();
        let (label, _) = lifeline_model::split_file_name(&name);
        let stage = match classify_title(&label, "0_5") {
            Some(StageHint::Act(n)) => Stage::Act(n),
            Some(StageHint::Interludes) => Stage::Interludes,
            _ => Stage::Endgame,
        };
        let mut build = PlannerBuild::from_json(&std::fs::read_to_string(path).unwrap()).unwrap();
        let weapon = build
            .inventory_slots
            .iter()
            .find(|s| s.inventory_id == "Weapon1")
            .and_then(|s| s.additional_text.as_deref())
            .and_then(|t| lifeline_model::supports::weapon_type(t, &data))
            .or(fallback);
        lifeline_model::supports::fit_skills(&mut build.skills, &data, stage, weapon);
        lifeline_model::supports::complete(&mut build.skills, &data, stage);
        println!("== {label} ({} sockets, cut level ≤ {})", lifeline_model::supports::sockets(stage), lifeline_model::supports::max_crafting_level(stage));
        for s in &build.skills {
            let n = |id: &str| data.gem_name(id).unwrap_or(id).to_owned();
            let sups: Vec<String> = s.support_skills.iter().map(|x| n(&x.id)).collect();
            let from = match s.level_interval {
                Some(lifeline_gamefiles::build_planner::LevelInterval::Range([a, b])) => format!("lv {a}-{b}"),
                _ => String::new(),
            };
            println!("  {:<18} {:<9} {}", n(&s.id), from, sups.join(", "));
        }
        if write {
            let dir = std::path::Path::new(path).parent().unwrap();
            let saved = lifeline_gamefiles::build_planner::write(dir, &build).unwrap();
            println!("  saved {}", saved.display());
        }
    }
}
