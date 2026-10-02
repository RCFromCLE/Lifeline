//! Live check: the build-rater job returns a schema-valid rating.
//! cargo run -p polr-ai --example rating_live

use std::sync::Arc;

use polr_ai::mcp_server::serve;
use polr_ai::{agents, parse_rating, ClaudeCli, Job};
use serde_json::json;

fn main() {
    let tools = json!([
        {"name": "character_state", "description": "Player state.", "inputSchema": {"type": "object", "properties": {}}},
        {"name": "build_plan", "description": "Imported build plan.", "inputSchema": {"type": "object", "properties": {}}},
        {"name": "equipped_items", "description": "Recorded equipped items.", "inputSchema": {"type": "object", "properties": {}}}
    ]);
    let server = serve(
        "polr",
        tools,
        Arc::new(|_, tool, _| {
            println!("  [tool] {tool}");
            Ok(match tool {
                "character_state" => json!({"name": "TestHuntress", "class": "Huntress", "level": 14, "act": 2, "area_level": 16,
                    "zone": "Vastiri Outskirts", "res_penalty": -10, "deaths": 0, "buffs": ["+10% to Cold Resistance", "+20 to maximum Life"],
                    "league": "HC Forbidden Rites"}),
                "build_plan" => json!({"name": "Spirit Walker", "current_stage": "Act 2",
                    "current_stage_skills": [{"skill": "Twister", "supports": ["Retreat I"]}, {"skill": "Whirling Slash", "supports": ["Rage I", "Rapid Attacks I"]}]}),
                _ => json!({"Spear": "Item Class: Spears\nRarity: Magic\nHunting Spear of Ire\n--------\nPhysical Damage: 14-26\n--------\nRequires: Level 10\n--------\nAdds 3 to 6 Cold Damage",
                            "Boots": "Item Class: Boots\nRarity: Normal\nMail Sabatons\n--------\nArmour: 22"}),
            })
        }),
    )
    .expect("server");
    let dir = std::env::temp_dir().join("polr-rating-live");
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = dir.join("mcp.json");
    std::fs::write(&cfg, server.config_json(900000)).unwrap();
    let mut cli = ClaudeCli::new(dir.clone());
    cli.agents_file = Some(agents::write_agents_file(&dir).unwrap());
    cli.mcp_config = Some(cfg);
    cli.allowed_tools.extend(["character_state", "build_plan", "equipped_items"].map(polr_ai::mcp_tool_name));
    let job = cli.for_job(Job::Rating);
    match job.run_turn(&Job::Rating.prompt("{}"), None, |_| {}) {
        Ok(o) => match parse_rating(&o.result) {
            Some(r) => println!("\nRATING {} ({:.0}/100): {}\n{}\ncategories: {:?}\nrecs: {:?}", r.grade, r.score, r.summary, r.explanation,
                r.categories.iter().map(|c| (&c.name, &c.grade)).collect::<Vec<_>>(), r.recommendations.iter().map(|x| (&x.slot, &x.title)).collect::<Vec<_>>()),
            None => println!("no rating parsed; raw: {:?} structured={:?}", o.result.result, o.result.structured_output),
        },
        Err(e) => println!("ERROR {e}"),
    }
}
