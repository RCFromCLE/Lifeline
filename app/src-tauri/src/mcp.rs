//! The app's MCP server: the game-data tools Claude's two jobs can call
//! (creating a build, suggesting skills). Streamable-HTTP transport answering
//! with plain JSON, on 127.0.0.1 with a per-launch bearer token. Each job gets
//! its own path (`/mcp/<scope>`) so the server knows which one is calling.

use std::sync::Arc;

use lifeline_ai::tools;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::state::AppState;

/// Starts the server (`lifeline_ai::mcp_server`); returns (port, token).
pub fn start(app: AppHandle) -> Result<(u16, String), String> {
    let server = lifeline_ai::mcp_server::serve(
        "lifeline",
        tool_definitions(),
        Arc::new(move |scope, name, args| {
            let result = call(&app, scope, name, args);
            let preview = match &result {
                Ok(v) => v.to_string().chars().take(160).collect::<String>(),
                Err(e) => format!("ERROR {e}"),
            };
            crate::debug_log(&app.state::<AppState>(), &format!("tool {name} (scope {scope}) → {preview}"));
            result
        }),
    )?;
    Ok((server.port, server.token))
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": tools::CHARACTER_STATE,
            "description": "The player's character from the game log: name, class, level, zone, area level and act.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": tools::BUILD_PLAN,
            "description": "The build being followed: its stages (Act 1–4, Interludes, Endgame) and the skills with supports planned for the current stage.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": tools::LOOKUP_GEM,
            "description": "Look up skill, spirit or support gems by name in the current game data: description, tags, attribute, recommended supports. Support gem names can differ from their ids (e.g. 'Rapid Attacks I').",
            "inputSchema": {"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]}
        },
        {
            "name": tools::LOOKUP_SUPPORTS_FOR,
            "description": "Shortlist of support gems relevant to a skill (game-recommended first, then supports whose text matches the skill's types). Judge fit yourself.",
            "inputSchema": {"type": "object", "properties": {"skill": {"type": "string"}, "limit": {"type": "integer"}}, "required": ["skill"]}
        },
        {
            "name": tools::LOOKUP_BASE,
            "description": "Item bases by name, or every base of a class ('boots', 'spear', 'body armour') with drop level, requirements, defences/damage and implicits.",
            "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}, "limit": {"type": "integer"}}, "required": ["query"]}
        },
        {
            "name": tools::LOOKUP_UNIQUE,
            "description": "Unique item by name: item class from game data plus its mods from the PoE2 wiki.",
            "inputSchema": {"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]}
        },
        {
            "name": tools::LOOKUP_MOD,
            "description": "Explicit item modifiers (prefix/suffix tiers with value ranges and required item level) whose text contains the words, optionally only those that can roll on an item class (slot: 'boots', 'ring', 'body armour', 'spear'…).",
            "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}, "slot": {"type": "string"}, "limit": {"type": "integer"}}, "required": ["text"]}
        },
        {
            "name": tools::LOOKUP_PASSIVE,
            "description": "Passive tree nodes (incl. notables, keystones, ascendancy) whose name or stats contain the words, from GGG's tree export. Returns the node ids the game and Build Planner use.",
            "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}, "limit": {"type": "integer"}}, "required": ["query"]}
        },
        {
            "name": tools::AREA_INFO,
            "description": "Area by name or id (e.g. 'Clearfell', 'G1_town'): act, area (monster) level, town, waypoint, bosses.",
            "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]}
        },
        {
            "name": tools::SEARCH_GAME_DATA,
            "description": "Search gems, item bases, uniques, areas and passives at once by name.",
            "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]}
        },
        {
            "name": tools::DESIGN_BUILD,
            "description": "Save the build you designed. The app computes the passive paths from your ordered target passives within each stage's points (cumulative: later stages keep earlier picks), checks every gem and support against game data and the game's support rules, and returns a report per stage: points used/budget/unspent, ascendancy points, reached and missed targets, problems. Budgets at stage end: Act 1 lvl 13/16 pts/0 asc, Act 2 lvl 29/36/2, Act 3 lvl 45/56/4, Act 4 lvl 52/67/4, Interludes lvl 65/86/4, Endgame lvl 90/113/8. Use exact names (lookup_passive, lookup_gem, lookup_supports_for). Fix what the report lists and call again until it's clean and most points are spent. Calling again replaces the design.",
            "inputSchema": {"type": "object", "properties": {
                "name": {"type": "string", "description": "short build name, e.g. 'Storm Spear Amazon'"},
                "class": {"type": "string", "description": "the character's class, e.g. 'Huntress'"},
                "ascendancy": {"type": "string", "description": "e.g. 'Amazon'"},
                "summary": {"type": "string", "description": "one sentence"},
                "endgame_level": {"type": "integer", "minimum": 65, "maximum": 100},
                "stages": {"type": "array", "description": "Act 1, Act 2, Act 3, Act 4, Interludes, Endgame", "items": {"type": "object", "properties": {
                    "stage": {"type": "string"},
                    "passives": {"type": "array", "items": {"type": "string"}, "description": "passive names to reach this stage, in order (notables, keystones, jewel sockets, or small passives)"},
                    "ascendancy": {"type": "array", "items": {"type": "string"}, "description": "ascendancy passive names to take this stage, in order"},
                    "skills": {"type": "array", "items": {"type": "object", "properties": {
                        "gem": {"type": "string"},
                        "supports": {"type": "array", "items": {"type": "string"}}
                    }, "required": ["gem"]}, "description": "omit to keep the previous stage's skills"},
                    "gear": {"type": "array", "items": {"type": "object", "properties": {
                        "slot": {"type": "string", "description": "Weapon, Offhand, Helmet, Body Armour, Gloves, Boots, Amulet, Ring, Belt"},
                        "unique": {"type": "string"},
                        "base": {"type": "string"},
                        "stats": {"type": "array", "items": {"type": "string"}}
                    }, "required": ["slot"]}, "description": "omit to keep the previous stage's gear goals"},
                    "notes": {"type": "string"}
                }, "required": ["stage"]}}
            }, "required": ["name", "class", "stages"]}
        }
    ])
}

fn call(app: &AppHandle, scope: u64, name: &str, args: &Value) -> Result<Value, String> {
    let state = app.state::<AppState>();
    match name {
        n if n == tools::CHARACTER_STATE => {
            let c = state.character.lock().unwrap().clone();
            serde_json::to_value(&c).map_err(|e| e.to_string())
        }
        n if n == tools::BUILD_PLAN => {
            let data = crate::gamedata::load(&state).ok();
            let imported = state.imported.lock().unwrap();
            let Some(i) = imported.as_ref() else {
                return Ok(json!("The player isn't following a build yet."));
            };
            let c = state.character.lock().unwrap().clone();
            let stage = crate::ai::current_stage(c.act, c.area_level);
            let skills: Vec<Value> = lifeline_model::usable_skills(&i.build, stage, data.as_deref())
                .iter()
                .map(|s| {
                    json!({"skill": crate::gamedata::gem_display(&state, &s.id),
                           "supports": s.support_skills.iter().map(|x| crate::gamedata::gem_display(&state, &x.id)).collect::<Vec<_>>()})
                })
                .collect();
            Ok(json!({
                "name": crate::ai::tidy(&i.name, 60),
                "class": i.build.class_name, "ascendancy": i.build.ascend_class_name,
                "current_stage": lifeline_model::stage_label(stage),
                "stages": i.stages.iter().filter(|s| s.chosen).map(|s| json!({"stage": s.stage, "levels": format!("{}-{}", s.level_from, s.level_to), "passives": s.main_points})).collect::<Vec<_>>(),
                "current_stage_skills": skills
            }))
        }
        n if n == tools::LOOKUP_GEM => {
            let d = crate::gamedata::load(&state)?;
            let name = args["name"].as_str().ok_or("name is required")?;
            Ok(json!({"version": d.version, "gems": d.find_gems(name, 6)}))
        }
        n if n == tools::LOOKUP_SUPPORTS_FOR => {
            let d = crate::gamedata::load(&state)?;
            let skill = args["skill"].as_str().ok_or("skill is required")?;
            let gem = d
                .find_gems(skill, 1)
                .into_iter()
                .find(|g| g.kind != "support")
                .ok_or(format!("no skill gem matching '{skill}'"))?;
            let limit = args["limit"].as_u64().unwrap_or(15) as usize;
            let all = d.supports_for(gem, 1000);
            let supports: Vec<Value> = all.iter().take(limit).map(|s| json!({"name": s.name, "effects": s.support_effects, "text": s.description, "attribute": s.attribute, "lineage": s.is_lineage, "uncut_support_level_needed": s.crafting_level, "recommended_by_game": gem.recommended_supports.contains(&s.name)})).collect();
            Ok(
                json!({"skill": gem.name, "skill_types": gem.skill_types, "compatible_total": all.len(), "note": if lifeline_data::GameData::is_minion_skill(gem) { "Minion/companion skill: supports apply to the minions' own skills, which the data doesn't list, so this isn't filtered by type. Prefer game-recommended and minion supports." } else { "Only supports the game allows on this skill (its type rules); game-recommended first. Ask with a larger limit for more." }, "supports": supports}),
            )
        }
        n if n == tools::LOOKUP_BASE => {
            let d = crate::gamedata::load(&state)?;
            let q = args["query"].as_str().ok_or("query is required")?;
            Ok(json!({"version": d.version, "bases": d.find_bases(q, args["limit"].as_u64().unwrap_or(12) as usize)}))
        }
        n if n == tools::LOOKUP_UNIQUE => {
            let d = crate::gamedata::load(&state)?;
            let name = args["name"].as_str().ok_or("name is required")?;
            let found = d.find_uniques(name, 5);
            let wiki = found
                .first()
                .map(|u| crate::gamedata::wiki_unique(&u.name))
                .transpose()
                .unwrap_or_else(|e| Some(vec![json!({"wiki_error": e})]));
            Ok(json!({"matches": found, "wiki": wiki}))
        }
        n if n == tools::LOOKUP_MOD => {
            let d = crate::gamedata::load(&state)?;
            let text = args["text"].as_str().ok_or("text is required")?;
            let mods = d.find_mods(text, args["slot"].as_str(), args["limit"].as_u64().unwrap_or(15) as usize);
            Ok(json!({"version": d.version, "mods": mods}))
        }
        n if n == tools::LOOKUP_PASSIVE => {
            let tree = crate::builds::load_tree(&state)?;
            let q = args["query"].as_str().ok_or("query is required")?;
            let found: Vec<Value> = tree.find_passives(q, args["limit"].as_u64().unwrap_or(10) as usize).iter().map(|n| json!({"id": n.id, "name": n.name, "notable": n.is_notable, "keystone": n.is_keystone, "ascendancy": n.ascendancy_id, "stats": n.stats.iter().map(|s| lifeline_data::game::plain(s)).collect::<Vec<_>>()})).collect();
            Ok(json!({"source": "GGG passive tree export", "passives": found}))
        }
        n if n == tools::AREA_INFO => {
            let d = crate::gamedata::load(&state)?;
            let q = args["query"].as_str().ok_or("query is required")?;
            Ok(json!({"areas": d.find_areas(q, 5)}))
        }
        n if n == tools::SEARCH_GAME_DATA => {
            let d = crate::gamedata::load(&state)?;
            let q = args["query"].as_str().ok_or("query is required")?;
            let passives: Vec<Value> = crate::builds::load_tree(&state)
                .map(|t| t.find_passives(q, 5).iter().map(|n| json!({"id": n.id, "name": n.name})).collect())
                .unwrap_or_default();
            Ok(json!({
                "gems": d.find_gems(q, 5).iter().map(|g| json!({"name": g.name, "kind": g.kind})).collect::<Vec<_>>(),
                "bases": d.find_bases(q, 5).iter().map(|b| json!({"name": b.name, "class": b.item_class})).collect::<Vec<_>>(),
                "uniques": d.find_uniques(q, 5),
                "areas": d.find_areas(q, 5).iter().map(|a| json!({"id": a.id, "name": a.name, "act": a.act, "level": a.area_level})).collect::<Vec<_>>(),
                "passives": passives
            }))
        }
        n if n == tools::DESIGN_BUILD => {
            // Only the build the player asked for ("Create full build") can
            // be saved, and it goes to My builds: nothing here changes the
            // build being followed.
            if scope != crate::wizard::BUILD_SCOPE {
                return Err("design_build is only available while creating a build".into());
            }
            let design: lifeline_model::BuildDesign =
                serde_json::from_value(args.clone()).map_err(|e| format!("design doesn't match the schema: {e}"))?;
            let (_, report) = crate::builds::realize_design(&state, &design)?;
            crate::wizard::attempt(app, &report);
            *state.last_design.lock().unwrap() = Some((design, report.clone()));
            let unspent: u32 = report.stages.last().map_or(0, |s| s.points_unspent);
            let next = if report.is_clean() && unspent <= 10 {
                "Clean. Saved. Finish up as your instructions say."
            } else {
                "Saved, but fix what's listed (missed targets, problems, unspent points) and call design_build again."
            };
            Ok(json!({"report": report, "next": next}))
        }
        _ => Err(format!("unknown tool {name}")),
    }
}

/// MCP config for one job's Claude run.
pub fn config_for(state: &AppState, scope: u64) -> Option<String> {
    let (port, token) = state.mcp.lock().unwrap().clone()?;
    Some(lifeline_ai::mcp_config_json(&format!("http://127.0.0.1:{port}/mcp/{scope}"), &token))
}
