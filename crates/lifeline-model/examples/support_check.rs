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
    for path in &args[1..] {
        let name = std::path::Path::new(path).file_name().unwrap().to_string_lossy().into_owned();
        let (label, _) = lifeline_model::split_file_name(&name);
        let stage = match classify_title(&label, "0_5") {
            Some(StageHint::Act(n)) => Stage::Act(n),
            Some(StageHint::Interludes) => Stage::Interludes,
            _ => Stage::Endgame,
        };
        let mut build = PlannerBuild::from_json(&std::fs::read_to_string(path).unwrap()).unwrap();
        lifeline_model::supports::complete(&mut build.skills, &data, stage);
        println!("== {label} ({} sockets, cut level ≤ {})", lifeline_model::supports::sockets(stage), lifeline_model::supports::max_crafting_level(stage));
        for s in &build.skills {
            let n = |id: &str| data.gem_name(id).unwrap_or(id).to_owned();
            let sups: Vec<String> = s.support_skills.iter().map(|x| n(&x.id)).collect();
            println!("  {:<18} {}", n(&s.id), sups.join(", "));
        }
        if write {
            let dir = std::path::Path::new(path).parent().unwrap();
            let saved = lifeline_gamefiles::build_planner::write(dir, &build).unwrap();
            println!("  saved {}", saved.display());
        }
    }
}
