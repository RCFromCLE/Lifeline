//! Market search the player drives: the modifier list for the pickers
//! (item properties such as total Armour, then every trade-site stat), a
//! direct search by item kind with a primary and secondary modifier, a price
//! limit or "best possible" (any price), and the level the character can wear.

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::state::AppState;

/// Item properties the trade site filters as `equipment_filters`: (key, label).
const PROPERTIES: [(&str, &str); 11] = [
    ("ar", "Armour (total)"),
    ("ev", "Evasion (total)"),
    ("es", "Energy Shield (total)"),
    ("block", "Block chance"),
    ("spirit", "Spirit"),
    ("dps", "Weapon DPS (total)"),
    ("pdps", "Weapon physical DPS"),
    ("edps", "Weapon elemental DPS"),
    ("aps", "Attacks per second"),
    ("crit", "Critical hit chance"),
    ("rune_sockets", "Rune sockets"),
];

/// Every modifier a picker can offer: properties first, then the trade
/// site's stats (labelled with their kind when it isn't a plain explicit).
pub fn modifiers(state: &AppState) -> Result<Vec<Value>, String> {
    let mut out: Vec<Value> = PROPERTIES
        .iter()
        .map(|(key, label)| json!({"id": format!("prop.{key}"), "label": label}))
        .collect();
    for s in state.market.stats()? {
        let text = lifeline_trade::plain(&s.text);
        let label = match s.kind.as_str() {
            "explicit" | "" => text,
            kind => format!("{text} ({kind})"),
        };
        out.push(json!({"id": s.id, "label": label}));
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    /// Trade category, e.g. `armour.boots`; empty for any.
    #[serde(default)]
    pub category: String,
    /// Modifier ids from [`modifiers`] (`prop.ar` or a stat id).
    #[serde(default)]
    pub primary: Option<String>,
    #[serde(default)]
    pub secondary: Option<String>,
    /// Price limit in exalted orbs; ignored for `best`.
    #[serde(default)]
    pub max_price: Option<f64>,
    /// Money is no object: the strongest pieces for the focus modifiers.
    #[serde(default)]
    pub best: bool,
    /// Only listings you can travel to and buy now.
    #[serde(default = "yes")]
    pub instant_only: bool,
}

fn yes() -> bool {
    true
}

/// The trade site query for a request (character level caps requirements).
fn query(req: &SearchRequest, level: u32) -> Value {
    let mut filters = json!({
        "req_filters": {"filters": {"lvl": {"max": level.max(1)}}},
    });
    if !req.category.is_empty() {
        filters["type_filters"] = json!({"filters": {"category": {"option": req.category}}});
    }
    if !req.best {
        if let Some(max) = req.max_price.filter(|m| *m > 0.0) {
            filters["trade_filters"] = json!({"filters": {"price": {"max": max, "option": "exalted"}}});
        }
    }
    let focus: Vec<&str> = [req.primary.as_deref(), req.secondary.as_deref()]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect();
    let mut equipment = serde_json::Map::new();
    let mut weights = Vec::new();
    for (i, id) in focus.iter().enumerate() {
        match id.strip_prefix("prop.") {
            Some(key) => {
                equipment.insert(key.to_owned(), json!({"min": 1}));
            }
            None => weights.push(json!({"id": id, "value": {"weight": if i == 0 { 2 } else { 1 }}})),
        }
    }
    if !equipment.is_empty() {
        filters["equipment_filters"] = json!({"filters": equipment});
    }
    let mut q = json!({
        "status": {"option": if req.instant_only { "securable" } else { "available" }},
        "filters": filters,
    });
    if !weights.is_empty() {
        q["stats"] = json!([{"type": "weight", "filters": weights, "value": {"min": 1}}]);
    }
    // Rank by the primary modifier (a property column, or the weighted
    // stat group); without one, cheapest first.
    let sort = match focus.first().map(|id| id.strip_prefix("prop.")) {
        Some(Some(key)) => json!({ key: "desc" }),
        Some(None) => json!({"statgroup.0": "desc"}),
        None => json!({"price": "asc"}),
    };
    json!({"query": q, "sort": sort})
}

/// Runs a search and returns listing cards (the same shape as Rating's
/// market cards, unrated). Listings are remembered so Travel works.
pub fn search(app: &AppHandle, req: &SearchRequest) -> Result<Value, String> {
    let state = app.state::<AppState>();
    let league = state.settings.lock().unwrap().league.clone();
    let level = state.character.lock().unwrap().level;
    let body = query(req, level);
    let outcome = match state.market.search(&league, &body, 20) {
        Ok(o) => o,
        // A sort the site doesn't take: retry cheapest first.
        Err(e) if body["sort"]["price"].is_null() => {
            let mut cheaper = body.clone();
            cheaper["sort"] = json!({"price": "asc"});
            state.market.search(&league, &cheaper, 20).map_err(|e2| format!("{e2} (and with its own sort: {e})"))?
        }
        Err(e) => return Err(e),
    };
    {
        let mut map = state.listings.lock().unwrap();
        let mut data = state.listing_data.lock().unwrap();
        for l in &outcome.listings {
            map.insert(l.id.clone(), outcome.query_id.clone());
            data.insert(l.id.clone(), l.clone());
        }
    }
    let cards: Vec<Value> = outcome
        .listings
        .iter()
        .map(|l| {
            json!({
                "listing_id": l.id, "icon": l.icon, "name": l.name, "base": l.base, "price": l.price,
                "seller": l.seller, "instant_buyout": l.instant_buyout, "requires": l.requires,
                "item_level": l.item_level, "corrupted": l.corrupted, "mods": l.mods, "search_id": outcome.query_id,
            })
        })
        .collect();
    Ok(json!({"total": outcome.total, "url": outcome.url, "search_id": outcome.query_id, "level": level, "cards": cards}))
}

/// Asks Lifeline (in a new chat) to rate these listings for the build.
pub fn rate_in_chat(app: &AppHandle, search_id: &str, listing_ids: &[String]) -> Result<u64, String> {
    if listing_ids.is_empty() {
        return Err("Search first.".into());
    }
    let state = app.state::<AppState>();
    // The AI can't open a past search, so it gets every listing in full.
    let listings: Vec<String> = {
        let data = state.listing_data.lock().unwrap();
        listing_ids
            .iter()
            .filter_map(|id| data.get(id))
            .map(|l| {
                format!(
                    "- listing_id {}: {}{} | {} | item level {} | {} | {}\n  {}",
                    l.id,
                    l.name,
                    if l.name.is_empty() || l.name == l.base { String::new() } else { format!(" ({})", l.base) },
                    l.price.as_deref().unwrap_or("no price"),
                    l.item_level.map_or("?".into(), |v| v.to_string()),
                    l.requires.as_deref().map_or("no requirements".into(), |r| format!("requires {r}")),
                    if l.corrupted { "corrupted" } else { "not corrupted" },
                    l.mods.join("; "),
                )
            })
            .collect()
    };
    if listings.is_empty() {
        return Err("Those results are from an earlier session. Search again, then rate.".into());
    }
    let id = state.new_conversation("Market check", false);
    state.save_conversations();
    let prompt = format!(
        "Rate these {} trade listings for my build and current stage (search_id {search_id}). Compare each with what I \
         wear in that slot (equipped_items) or my plan, hardcore first, then call rate_listings once with all of them \
         (listing_id, delta_pct, verdict). Name the best pick and why in one sentence.\n\nListings:\n{}",
        listings.len(),
        listings.join("\n")
    );
    let display = format!("Rate these {} market results for my build.", listings.len());
    crate::ai::ask_as(app, Some(id), prompt, display, "Market check", crate::ai::Origin::Chat);
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(primary: &str, secondary: &str, best: bool) -> SearchRequest {
        SearchRequest {
            category: "armour.chest".into(),
            primary: Some(primary.into()),
            secondary: Some(secondary.into()),
            max_price: Some(10.0),
            best,
            instant_only: true,
        }
    }

    #[test]
    fn queries_rank_by_the_primary_modifier() {
        let q = query(&req("prop.ar", "prop.ev", false), 30);
        assert_eq!(q["sort"], json!({"ar": "desc"}));
        assert_eq!(q["query"]["filters"]["equipment_filters"]["filters"]["ev"], json!({"min": 1}));
        assert_eq!(q["query"]["filters"]["req_filters"]["filters"]["lvl"]["max"], 30);
        assert_eq!(q["query"]["filters"]["trade_filters"]["filters"]["price"]["max"], 10.0);

        let q = query(&req("pseudo.pseudo_total_life", "prop.ar", true), 30);
        assert_eq!(q["sort"], json!({"statgroup.0": "desc"}));
        assert_eq!(q["query"]["stats"][0]["filters"][0]["value"]["weight"], 2);
        assert!(q["query"]["filters"]["trade_filters"].is_null(), "best possible ignores price");
    }
}
