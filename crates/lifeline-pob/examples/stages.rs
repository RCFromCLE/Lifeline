//! Splits a Path of Building build into playthrough stages.
//!
//! cargo run -p lifeline-pob --example stages -- crates/lifeline-pob/tests/fixtures/deadeye_0_5.pob
//! cargo run -p lifeline-pob --example stages -- <file containing a PoB code>
//!
//! Share links (pobb.in etc.) are resolved but not downloaded yet; download
//! the code from the printed URL and pass the file. Main-tree point counts
//! here include class/ascendancy start nodes until tree data lands (M2).

use lifeline_pob::{classify_title, decode, estimate_level, parse_xml, resolve, resolve_stage, BuildSource, POE2_0_5_ACTS};

fn main() {
    let Some(arg) = std::env::args().nth(1) else {
        eprintln!("usage: stages <file with a PoB code | code | link>");
        return;
    };
    let input = std::fs::read_to_string(&arg).unwrap_or(arg);
    let code = match resolve(&input) {
        BuildSource::Code(code) => code,
        BuildSource::CodeUrl(url) | BuildSource::MaxrollPlannerUrl(url) => {
            println!("That's a link. The app will download it from:\n  {url}\nFor now, save the code to a file and pass the file.");
            return;
        }
        BuildSource::Unsupported { site, advice } => {
            println!("{site}: {advice}");
            return;
        }
    };
    let build = match decode(&code)
        .map_err(|e| e.to_string())
        .and_then(|x| parse_xml(&x).map_err(|e| e.to_string()))
    {
        Ok(b) => b,
        Err(e) => {
            println!("Could not read build: {e}");
            return;
        }
    };

    println!(
        "{} / {} — level {} — {} tree specs",
        build.class_name.as_deref().unwrap_or("?"),
        build.ascend_class_name.as_deref().unwrap_or("?"),
        build.level.unwrap_or(0),
        build.specs.len()
    );
    println!(
        "\n{:<36} {:>6} {:>6}  {:<22} stage",
        "spec title", "nodes", "≈lvl", "title says"
    );
    for spec in &build.specs {
        let hint = classify_title(&spec.title, &spec.tree_version);
        let points = spec.nodes.len() as u32;
        let ws = (spec.weapon_set1.len() as u32, spec.weapon_set2.len() as u32);
        let level = estimate_level(&POE2_0_5_ACTS, points, 0, ws);
        let stage = resolve_stage(hint, level);
        println!(
            "{:<36} {:>6} {:>6}  {:<22} {stage:?}",
            spec.title,
            points,
            level,
            hint.map_or("-".to_string(), |h| format!("{h:?}"))
        );
    }

    println!("\nSkill sets:");
    for set in &build.skill_sets {
        let gems: usize = set.groups.iter().map(|g| g.gems.len()).sum();
        println!(
            "  {:<24} {} groups, {gems} gems",
            if set.title.is_empty() { "(default)" } else { &set.title },
            set.groups.len()
        );
        for g in set
            .groups
            .iter()
            .filter(|g| g.gems.is_empty() && !g.label.trim().is_empty())
        {
            println!("    section: {}", g.label.trim());
        }
    }
}
