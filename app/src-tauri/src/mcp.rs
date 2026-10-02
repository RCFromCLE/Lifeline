//! The app's MCP server: the tools Claude (the companion and its specialist
//! agents) can call. Streamable-HTTP transport answering with plain JSON, on
//! 127.0.0.1 with a per-launch bearer token. Each conversation gets its own
//! path (`/mcp/<conversation id>`) so proposed actions land in the right chat.

use std::sync::Arc;

use lifeline_ai::tools;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::state::{AppState, PendingAction};

/// Starts the server (`lifeline_ai::mcp_server`); returns (port, token).
pub fn start(app: AppHandle) -> Result<(u16, String), String> {
    let server = lifeline_ai::mcp_server::serve(
        "lifeline",
        tool_definitions(),
        Arc::new(move |conv, name, args| {
            let result = call(&app, conv, name, args);
            let preview = match &result {
                Ok(v) => v.to_string().chars().take(160).collect::<String>(),
                Err(e) => format!("ERROR {e}"),
            };
            crate::debug_log(
                &app.state::<AppState>(),
                &format!("tool {name} (scope {conv}) → {preview}"),
            );
            result
        }),
    )?;
    Ok((server.port, server.token))
}
fn tool_definitions() -> Value {
    json!([
        {
            "name": tools::CHARACTER_STATE,
            "description": "The player's live state from the game log: character, class, level, zone, area level, act, resistance penalty there, deaths, permanent buffs, league.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": tools::BUILD_PLAN,
            "description": "The imported build: its stages (Act 1–4, Interludes, Endgame) and the skills with supports planned for the current stage.",
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
        },        {
            "name": tools::EQUIPPED_ITEMS,
            "description": "Items the player recorded as currently equipped (hotkey on the hovered item), keyed by item class, with full item text. Empty slots mean nothing was recorded — fall back to the build plan.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": tools::RATE_LISTINGS,
            "description": "Show trade listings to the player as cards with the item image, price, mods, a colour-coded badge with your delta_pct (how much better + or worse − the item is for the current build than the reference) and a Travel button. Use after trade_search.",
            "inputSchema": {"type": "object", "properties": {
                "search_id": {"type": "string"},
                "compared_to": {"type": "string", "description": "what the percentages compare against, e.g. 'your equipped Hunting Shoes'"},
                "ratings": {"type": "array", "items": {"type": "object", "properties": {
                    "listing_id": {"type": "string"},
                    "delta_pct": {"type": "number"},
                    "verdict": {"type": "string"}
                }, "required": ["listing_id", "delta_pct", "verdict"]}}
            }, "required": ["compared_to", "ratings"]}
        },        {
            "name": tools::TRADE_FIND_STAT,
            "description": "Find trade-site stat ids by text, e.g. 'cold res', 'maximum life', 'movement speed'. Pseudo ids (pseudo.pseudo_total_*) sum every source on the item.",
            "inputSchema": {"type": "object", "properties": {
                "query": {"type": "string"},
                "limit": {"type": "integer", "minimum": 1, "maximum": 15}
            }, "required": ["query"]}
        },
        {
            "name": tools::TRADE_SEARCH,
            "description": "Search the PoE2 trade market in the player's league and return listings (price, seller, mods with tiers). Pass the trade site's `query` object (stats, filters, optional type/name). Instant Buyout only unless instant_buyout_only is false. Each listing has a listing_id; the result has a search_id.",
            "inputSchema": {"type": "object", "properties": {
                "query": {"type": "object", "description": "trade2 query object, e.g. {\"stats\":[{\"type\":\"and\",\"filters\":[{\"id\":\"pseudo.pseudo_total_cold_resistance\",\"value\":{\"min\":20}}]}],\"filters\":{\"type_filters\":{\"filters\":{\"category\":{\"option\":\"armour.boots\"}}}}}"},
                "sort": {"type": "object", "description": "default {\"price\":\"asc\"}"},
                "instant_buyout_only": {"type": "boolean"},
                "max_listings": {"type": "integer", "minimum": 1, "maximum": 20}
            }, "required": ["query"]}
        },
        {
            "name": tools::PRICE,
            "description": "poe.ninja price in the player's league for currency, fragments, runes, essences, soul cores, uncut/lineage gems and uniques, matched by name. Values in divine and exalted.",
            "inputSchema": {"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]}
        },
        {
            "name": tools::DESIGN_BUILD,
            "description": "Make a designed build the player's build. The app computes the passive paths from your ordered target passives within each stage's points (cumulative: later stages keep earlier picks), checks every gem and support against game data and the game's support rules, and returns a report per stage: points used/budget/unspent, ascendancy points, reached and missed targets, problems. Budgets at stage end: Act 1 lvl 13/16 pts/0 asc, Act 2 lvl 29/36/2, Act 3 lvl 45/56/4, Act 4 lvl 52/67/4, Interludes lvl 65/86/4, Endgame lvl 90/113/8. Use exact names (lookup_passive, lookup_gem, lookup_supports_for). Fix what the report lists and call again until it's clean and most points are spent. Calling again replaces the build.",
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
        },        {
            "name": tools::PROPOSE_ACTION,
            "description": "Offer the player an action as a button in the chat; it only happens if they press it. kind 'travel' = travel to the seller's hideout for a listing (needs listing_id and search_id from trade_search); kind 'open_search' = open the search on the trade site (needs search_id); kind 'write_planner' = write the current build's stages into the game's Build Planner (after design_build or an import).",
            "inputSchema": {"type": "object", "properties": {
                "kind": {"type": "string", "enum": ["travel", "open_search", "write_planner"]},
                "summary": {"type": "string", "description": "one line shown on the button card, e.g. 'Hypnotic Tread — 37% cold, 86 life — 1 alch'"},
                "listing_id": {"type": "string"},
                "search_id": {"type": "string"}
            }, "required": ["kind", "summary"]}
        }
    ])
}

fn call(app: &AppHandle, conv: u64, name: &str, args: &Value) -> Result<Value, String> {
    let state = app.state::<AppState>();
    let league = state.settings.lock().unwrap().league.clone();
    match name {
        n if n == tools::CHARACTER_STATE => {
            let c = state.character.lock().unwrap().clone();
            let mut v = serde_json::to_value(&c).map_err(|e| e.to_string())?;
            v["league"] = json!(league);
            Ok(v)
        }
        n if n == tools::BUILD_PLAN => {
            let imported = state.imported.lock().unwrap();
            let Some(i) = imported.as_ref() else {
                return Ok(json!("No build yet. Point the player to the Builds tab: the Showcase guides them to a researched hardcore build and creates it (or Import for a Path of Building link)."));
            };
            let c = state.character.lock().unwrap().clone();
            let stage = crate::ai::current_stage(c.act, c.area_level);
            let skills: Vec<Value> = lifeline_model::skills_for_stage(&i.build, stage)
                .iter()
                .map(|s| {
                    json!({"skill": crate::gamedata::gem_display(&state, &s.id),
                           "supports": s.support_skills.iter().map(|x| crate::gamedata::gem_display(&state, &x.id)).collect::<Vec<_>>()})
                })
                .collect();
            Ok(json!({
                "name": i.name,
                "class": i.build.class_name, "ascendancy": i.build.ascend_class_name,
                "current_stage": lifeline_model::stage_label(stage),
                "stages": i.stages.iter().filter(|s| s.chosen).map(|s| json!({"stage": s.stage, "spec": s.title, "level": s.estimated_level, "passives": s.main_points})).collect::<Vec<_>>(),
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
            let supports: Vec<Value> = all.iter().take(limit).map(|s| json!({"name": s.name, "effects": s.support_effects, "text": s.description, "attribute": s.attribute, "lineage": s.is_lineage, "recommended_by_game": gem.recommended_supports.contains(&s.name)})).collect();
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
            let mods = d.find_mods(
                text,
                args["slot"].as_str(),
                args["limit"].as_u64().unwrap_or(15) as usize,
            );
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
                .map(|t| {
                    t.find_passives(q, 5)
                        .iter()
                        .map(|n| json!({"id": n.id, "name": n.name}))
                        .collect()
                })
                .unwrap_or_default();
            Ok(json!({
                "gems": d.find_gems(q, 5).iter().map(|g| json!({"name": g.name, "kind": g.kind})).collect::<Vec<_>>(),
                "bases": d.find_bases(q, 5).iter().map(|b| json!({"name": b.name, "class": b.item_class})).collect::<Vec<_>>(),
                "uniques": d.find_uniques(q, 5),
                "areas": d.find_areas(q, 5).iter().map(|a| json!({"id": a.id, "name": a.name, "act": a.act, "level": a.area_level})).collect::<Vec<_>>(),
                "passives": passives
            }))
        }
        n if n == tools::EQUIPPED_ITEMS => {
            let equipped = state.equipped.lock().unwrap().clone();
            if equipped.is_empty() {
                Ok(json!("Nothing recorded yet. The player records equipped items by hovering them in game and pressing the record-equipped hotkey."))
            } else {
                Ok(json!(equipped))
            }
        }
        n if n == tools::RATE_LISTINGS => {
            let compared_to = args["compared_to"].as_str().unwrap_or("your current item").to_owned();
            let data = state.listing_data.lock().unwrap().clone();
            let mut cards = Vec::new();
            for r in args["ratings"].as_array().into_iter().flatten() {
                let Some(id) = r["listing_id"].as_str() else { continue };
                let Some(l) = data.get(id) else { continue };
                cards.push(json!({
                    "listing_id": l.id, "icon": l.icon, "name": l.name, "base": l.base, "price": l.price,
                    "seller": l.seller, "instant_buyout": l.instant_buyout, "requires": l.requires,
                    "item_level": l.item_level, "corrupted": l.corrupted, "mods": l.mods,
                    "delta_pct": r["delta_pct"].as_f64().unwrap_or(0.0), "verdict": r["verdict"]
                }));
            }
            if cards.is_empty() {
                return Err("none of those listing_ids are from a recent trade_search".into());
            }
            let payload = json!({"compared_to": compared_to, "cards": cards});
            {
                let mut convs = state.conversations.lock().unwrap();
                if let Some(c) = convs.iter_mut().find(|c| c.id == conv) {
                    c.messages.push(crate::state::Message {
                        role: "market".into(),
                        label: "Market".into(),
                        text: payload.to_string(),
                        error: false,
                    });
                }
            }
            if conv >= crate::rating::MARKET_SCOPE_BASE {
                let index = (conv - crate::rating::MARKET_SCOPE_BASE) as usize;
                state.rating_cards.lock().unwrap().insert(index, payload.clone());
                state.save_rating();
            }
            let _ = app.emit("market", json!({"conv": conv, "market": payload}));
            Ok(json!(format!(
                "Shown {} listing cards with images, ±% badges and Travel buttons.",
                cards.len()
            )))
        }
        n if n == tools::TRADE_FIND_STAT => {
            let query = args["query"].as_str().ok_or("query is required")?;
            let limit = args["limit"].as_u64().unwrap_or(8) as usize;
            let found = state.market.find_stats(query, limit)?;
            Ok(json!(found))
        }
        n if n == tools::TRADE_SEARCH => {
            let mut query = args["query"].clone();
            if !query.is_object() {
                return Err("query must be the trade site's query object".into());
            }
            if query.get("status").is_none() {
                let ib = args["instant_buyout_only"].as_bool().unwrap_or(true);
                query["status"] = json!({"option": if ib { "securable" } else { "available" }});
            }
            let mut body = json!({"query": query});
            if args["sort"].is_object() {
                body["sort"] = args["sort"].clone();
            }
            let max = args["max_listings"].as_u64().unwrap_or(10) as usize;
            let _ = app.emit(
                "ai",
                json!({"conv": conv, "type": "tool", "text": "searching market…"}),
            );
            let outcome = state.market.search(&league, &body, max)?;
            {
                let mut map = state.listings.lock().unwrap();
                for l in &outcome.listings {
                    map.insert(l.id.clone(), outcome.query_id.clone());
                }
                let mut data = state.listing_data.lock().unwrap();
                for l in &outcome.listings {
                    data.insert(l.id.clone(), l.clone());
                }
            }
            Ok(json!({
                "league": league,
                "total": outcome.total,
                "search_id": outcome.query_id,
                "url": outcome.url,
                "listings": outcome.listings.iter().map(|l| json!({
                    "listing_id": l.id, "price": l.price, "seller": l.seller, "instant_buyout": l.instant_buyout,
                    "name": l.name, "base": l.base, "item_level": l.item_level, "requires": l.requires, "corrupted": l.corrupted, "mods": l.mods
                })).collect::<Vec<_>>()
            }))
        }
        n if n == tools::PRICE => {
            let name = args["name"].as_str().ok_or("name is required")?;
            let hits = state.market.price(&league, name)?;
            if hits.is_empty() {
                Ok(json!(format!("No poe.ninja price found for '{name}' in {league}. For rares, search comparable items with trade_search.")))
            } else {
                Ok(json!({"league": league, "source": "poe.ninja", "prices": hits}))
            }
        }
        n if n == tools::DESIGN_BUILD => {
            let design: lifeline_model::BuildDesign =
                serde_json::from_value(args.clone()).map_err(|e| format!("design doesn't match the schema: {e}"))?;
            // Wizard runs fill the library; chat runs make the build active.
            let report = if conv == crate::wizard::BUILD_SCOPE {
                let (_, report) = crate::builds::realize_design(&state, &design)?;
                crate::wizard::attempt(app, &report);
                *state.last_design.lock().unwrap() = Some((design.clone(), report.clone()));
                report
            } else {
                let (report, view) = crate::builds::create(&state, &design)?;
                let _ = app.emit("imported", &view);
                report
            };
            let unspent: u32 = report.stages.last().map_or(0, |s| s.points_unspent);
            let next = if report.is_clean() && unspent <= 10 {
                "Clean. Saved. Finish up as your instructions say."
            } else {
                "Saved, but fix what's listed (missed targets, problems, unspent points) and call design_build again."
            };
            Ok(json!({"report": report, "next": next}))
        }
        n if n == tools::PROPOSE_ACTION => {
            let kind = args["kind"].as_str().unwrap_or("");
            let summary = args["summary"].as_str().unwrap_or("").to_owned();
            let listing_id = args["listing_id"].as_str().map(str::to_owned);
            let mut search_id = args["search_id"].as_str().map(str::to_owned);
            match kind {
                "travel" => {
                    let lid = listing_id.as_deref().ok_or("travel needs listing_id")?;
                    if search_id.is_none() {
                        search_id = state.listings.lock().unwrap().get(lid).cloned();
                    }
                    if search_id.is_none() {
                        return Err("unknown listing — run trade_search first".into());
                    }
                }
                "open_search" if search_id.is_none() => return Err("open_search needs search_id".into()),
                "open_search" => {}
                "write_planner" if state.imported.lock().unwrap().is_none() => {
                    return Err("no build yet — call design_build (or have the player import one) first".into())
                }
                "write_planner" => {}
                _ => return Err("kind must be 'travel', 'open_search' or 'write_planner'".into()),
            }
            let action = {
                let mut actions = state.actions.lock().unwrap();
                let id = actions.iter().map(|a| a.id).max().unwrap_or(0) + 1;
                let a = PendingAction {
                    id,
                    conv,
                    kind: kind.to_owned(),
                    summary,
                    listing_id,
                    search_id,
                };
                actions.push(a.clone());
                a
            };
            let _ = app.emit("action", &action);
            Ok(json!("Shown to the player as a button in the chat. It only happens if they press it; don't claim it happened."))
        }
        _ => Err(format!("unknown tool {name}")),
    }
}

/// MCP config for one conversation's Claude turn.
pub fn config_for(state: &AppState, conv: u64) -> Option<String> {
    let (port, token) = state.mcp.lock().unwrap().clone()?;
    Some(lifeline_ai::mcp_config_json(
        &format!("http://127.0.0.1:{port}/mcp/{conv}"),
        &token,
    ))
}
