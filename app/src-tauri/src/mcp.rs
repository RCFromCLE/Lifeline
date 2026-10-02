//! The app's MCP server: the tools Claude (the companion and its specialist
//! agents) can call. Streamable-HTTP transport answering with plain JSON, on
//! 127.0.0.1 with a per-launch bearer token. Each conversation gets its own
//! path (`/mcp/<conversation id>`) so proposed actions land in the right chat.

use std::sync::Arc;

use polr_ai::tools;
use polr_gamefiles::build_planner::gem_short_name;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::state::{AppState, PendingAction};

/// Starts the server (`polr_ai::mcp_server`); returns (port, token).
pub fn start(app: AppHandle) -> Result<(u16, String), String> {
    let server = polr_ai::mcp_server::serve(
        "polr",
        tool_definitions(),
        Arc::new(move |conv, name, args| call(&app, conv, name, args)),
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
            "name": tools::PROPOSE_ACTION,
            "description": "Offer the player an action as a button in the chat; it only happens if they press it. kind 'travel' = travel to the seller's hideout for a listing (needs listing_id and search_id from trade_search); kind 'open_search' = open the search on the trade site (needs search_id).",
            "inputSchema": {"type": "object", "properties": {
                "kind": {"type": "string", "enum": ["travel", "open_search"]},
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
                return Ok(json!("No build imported yet."));
            };
            let c = state.character.lock().unwrap().clone();
            let stage = crate::ai::current_stage(c.act, c.area_level);
            let skills: Vec<Value> = polr_model::skills_for_stage(&i.build, stage)
                .iter()
                .map(|s| {
                    json!({"skill": gem_short_name(&s.id),
                           "supports": s.support_skills.iter().map(|x| gem_short_name(&x.id)).collect::<Vec<_>>()})
                })
                .collect();
            Ok(json!({
                "name": i.name,
                "class": i.build.class_name, "ascendancy": i.build.ascend_class_name,
                "current_stage": polr_model::stage_label(stage),
                "stages": i.stages.iter().filter(|s| s.chosen).map(|s| json!({"stage": s.stage, "spec": s.title, "level": s.estimated_level, "passives": s.main_points})).collect::<Vec<_>>(),
                "current_stage_skills": skills
            }))
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
                json!({"conv": conv, "type": "tool", "text": "searching the trade market…"}),
            );
            let outcome = state.market.search(&league, &body, max)?;
            {
                let mut map = state.listings.lock().unwrap();
                for l in &outcome.listings {
                    map.insert(l.id.clone(), outcome.query_id.clone());
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
                _ => return Err("kind must be 'travel' or 'open_search'".into()),
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
    Some(polr_ai::mcp_config_json(
        &format!("http://127.0.0.1:{port}/mcp/{conv}"),
        &token,
    ))
}
