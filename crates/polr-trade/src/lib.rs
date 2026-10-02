//! PoE2 market access (PLAN.md §7).
//!
//! - trade2 search/fetch: GGG's undocumented trade endpoints (grey area the
//!   owner accepted, §2.1), paced by the `X-Rate-Limit-*` headers they return.
//! - stat lookup from the trade site's own `/api/trade2/data/stats`.
//! - poe.ninja economy prices (currency, gems, runes, uniques…).
//!
//! Travelling to a seller is not done here: it needs the player's logged-in
//! session and runs inside the app's trade window.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};

const BASE: &str = "https://www.pathofexile.com";
const USER_AGENT: &str = "PathOfLeastResistance/0.1 (personal PoE2 companion)";
const NINJA_TTL: Duration = Duration::from_secs(15 * 60);

const EXCHANGE_TYPES: &[&str] = &[
    "Currency",
    "Fragments",
    "Runes",
    "Essences",
    "SoulCores",
    "UncutGems",
    "LineageSupportGems",
    "Idols",
    "Abyss",
    "Ritual",
    "Expedition",
    "Delirium",
    "Breach",
];
const UNIQUE_TYPES: &[&str] = &[
    "UniqueWeapons",
    "UniqueArmours",
    "UniqueAccessories",
    "UniqueFlasks",
    "UniqueCharms",
    "UniqueJewels",
];

#[derive(Debug, Clone, Serialize)]
pub struct StatEntry {
    pub id: String,
    pub text: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Listing {
    /// Listing id for fetch/travel.
    pub id: String,
    /// e.g. "3 exalted".
    pub price: Option<String>,
    pub seller: String,
    /// Instant Buyout (merchant tab) — travel to hideout works.
    pub instant_buyout: bool,
    pub name: String,
    pub base: String,
    pub item_level: Option<u64>,
    pub corrupted: bool,
    /// e.g. "Level 45, 78 Dex".
    pub requires: Option<String>,
    pub mods: Vec<String>,
    /// Item art URL (web.poecdn.com).
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchOutcome {
    pub query_id: String,
    pub total: u64,
    pub url: String,
    pub listings: Vec<Listing>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PriceHit {
    pub name: String,
    pub category: String,
    pub divine: f64,
    pub exalted: Option<f64>,
    pub detail: Option<String>,
}

pub struct Market {
    agent: ureq::Agent,
    next_allowed: Mutex<HashMap<&'static str, Instant>>,
    stats: Mutex<Option<Vec<StatEntry>>>,
    ninja: Mutex<HashMap<String, (Instant, Value)>>,
}

impl Default for Market {
    fn default() -> Self {
        Self::new()
    }
}

/// Percent-encodes a path segment (league names contain spaces).
pub fn encode_segment(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// `[Resistances|Cold Resistance]` → `Cold Resistance`.
pub fn plain(text: &str) -> String {
    static LINKS: OnceLock<Regex> = OnceLock::new();
    LINKS
        .get_or_init(|| Regex::new(r"\[(?:[^\]|]*\|)?([^\]]*)\]").expect("static regex"))
        .replace_all(text, "$1")
        .into_owned()
}

/// Seconds to wait before the next request under a policy, from GGG's
/// `X-Rate-Limit-{rule}` (`hits:period:penalty,…`) and `-State`
/// (`current:period:restricted,…`) headers.
pub fn backoff_from_headers(get: impl Fn(&str) -> Option<String>) -> u64 {
    let rules = get("X-Rate-Limit-Rules").unwrap_or_default();
    let mut wait = 0;
    for rule in rules.split(',').map(str::trim).filter(|r| !r.is_empty()) {
        let (Some(limits), Some(states)) = (
            get(&format!("X-Rate-Limit-{rule}")),
            get(&format!("X-Rate-Limit-{rule}-State")),
        ) else {
            continue;
        };
        for (limit, state) in limits.split(',').zip(states.split(',')) {
            let l: Vec<u64> = limit.split(':').filter_map(|x| x.trim().parse().ok()).collect();
            let s: Vec<u64> = state.split(':').filter_map(|x| x.trim().parse().ok()).collect();
            if l.len() < 2 || s.len() < 3 {
                continue;
            }
            if s[2] > 0 {
                wait = wait.max(s[2]);
            } else if s[0] + 1 >= l[0] {
                wait = wait.max(l[1]);
            }
        }
    }
    wait
}

impl Market {
    pub fn new() -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .into();
        Self {
            agent,
            next_allowed: Mutex::new(HashMap::new()),
            stats: Mutex::new(None),
            ninja: Mutex::new(HashMap::new()),
        }
    }

    fn pace(&self, policy: &'static str) {
        let until = self.next_allowed.lock().unwrap().get(policy).copied();
        if let Some(until) = until {
            let now = Instant::now();
            if until > now {
                std::thread::sleep(until - now);
            }
        }
    }

    fn request(&self, policy: &'static str, url: &str, body: Option<&Value>) -> Result<Value, String> {
        self.pace(policy);
        let response = match body {
            Some(b) => self
                .agent
                .post(url)
                .header("User-Agent", USER_AGENT)
                .header("Content-Type", "application/json")
                .send(b.to_string()),
            None => self.agent.get(url).header("User-Agent", USER_AGENT).call(),
        };
        let mut response = response.map_err(|e| format!("market request failed: {e}"))?;
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let wait = backoff_from_headers(header);
        let retry_after: Option<u64> = header("Retry-After").and_then(|v| v.parse().ok());
        if wait > 0 || retry_after.is_some() {
            let secs = wait.max(retry_after.unwrap_or(0));
            self.next_allowed
                .lock()
                .unwrap()
                .insert(policy, Instant::now() + Duration::from_secs(secs));
        }
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("market response unreadable: {e}"))?;
        if status == 429 {
            return Err(format!(
                "The trade site is rate limiting us; try again in {}s.",
                retry_after.unwrap_or(60)
            ));
        }
        let value: Value = serde_json::from_str(&text).map_err(|_| format!("market returned HTTP {status}"))?;
        if status >= 400 {
            let msg = value["error"]["message"].as_str().unwrap_or("request rejected");
            return Err(format!("trade site HTTP {status}: {msg}"));
        }
        Ok(value)
    }

    /// The trade site's stat list (fetched once per run).
    pub fn stats(&self) -> Result<Vec<StatEntry>, String> {
        if let Some(s) = self.stats.lock().unwrap().clone() {
            return Ok(s);
        }
        let v = self.request("data", &format!("{BASE}/api/trade2/data/stats"), None)?;
        let entries: Vec<StatEntry> = v["result"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|group| group["entries"].as_array().cloned().unwrap_or_default())
            .filter_map(|e| {
                Some(StatEntry {
                    id: e["id"].as_str()?.to_owned(),
                    text: e["text"].as_str()?.to_owned(),
                    kind: e["type"].as_str().unwrap_or("").to_owned(),
                })
            })
            .collect();
        *self.stats.lock().unwrap() = Some(entries.clone());
        Ok(entries)
    }

    /// Stat ids whose text best matches `query` ("cold res", "maximum life").
    pub fn find_stats(&self, query: &str, limit: usize) -> Result<Vec<StatEntry>, String> {
        Ok(rank_stats(&self.stats()?, query, limit))
    }

    /// Runs a trade2 search and fetches up to `max_listings` (≤ 20) results.
    /// `query` is the trade site's own JSON (`{"query": {...}, "sort": {...}}`).
    pub fn search(&self, league: &str, query: &Value, max_listings: usize) -> Result<SearchOutcome, String> {
        let mut body = query.clone();
        if body.get("sort").is_none() {
            body["sort"] = json!({"price": "asc"});
        }
        let league_seg = encode_segment(league);
        let found = self.request(
            "search",
            &format!("{BASE}/api/trade2/search/poe2/{league_seg}"),
            Some(&body),
        )?;
        let query_id = found["id"].as_str().ok_or("search returned no id")?.to_owned();
        let total = found["total"].as_u64().unwrap_or(0);
        let ids: Vec<String> = found["result"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .take(max_listings.min(20))
            .collect();
        let mut listings = Vec::new();
        for chunk in ids.chunks(10) {
            let url = format!(
                "{BASE}/api/trade2/fetch/{}?query={query_id}&realm=poe2",
                chunk.join(",")
            );
            let fetched = self.request("fetch", &url, None)?;
            listings.extend(
                fetched["result"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(parse_listing),
            );
        }
        Ok(SearchOutcome {
            url: format!("{BASE}/trade2/search/poe2/{league_seg}/{query_id}"),
            query_id,
            total,
            listings,
        })
    }

    fn ninja(&self, league: &str, kind: &str, unique: bool) -> Result<Value, String> {
        let key = format!("{league}|{kind}");
        if let Some((at, v)) = self.ninja.lock().unwrap().get(&key) {
            if at.elapsed() < NINJA_TTL {
                return Ok(v.clone());
            }
        }
        let league = encode_segment(league);
        let url = if unique {
            format!("https://poe.ninja/poe2/api/economy/stash/current/item/overview?league={league}&type={kind}")
        } else {
            format!("https://poe.ninja/poe2/api/economy/exchange/current/overview?league={league}&type={kind}")
        };
        let v = self.request("ninja", &url, None)?;
        self.ninja.lock().unwrap().insert(key, (Instant::now(), v.clone()));
        Ok(v)
    }

    /// poe.ninja prices for items whose name contains `name` (case-insensitive).
    pub fn price(&self, league: &str, name: &str) -> Result<Vec<PriceHit>, String> {
        let needle = name.to_lowercase();
        let currency = self.ninja(league, "Currency", false)?;
        let exalted_per_divine = currency["core"]["rates"]["exalted"].as_f64();
        let to_ex = |d: f64| exalted_per_divine.map(|r| (d * r * 10.0).round() / 10.0);
        let mut hits = Vec::new();
        for kind in EXCHANGE_TYPES {
            let v = if *kind == "Currency" {
                currency.clone()
            } else {
                self.ninja(league, kind, false)?
            };
            let names: HashMap<&str, &str> = v["items"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|i| Some((i["id"].as_str()?, i["name"].as_str()?)))
                .collect();
            for line in v["lines"].as_array().into_iter().flatten() {
                let Some(id) = line["id"].as_str() else { continue };
                let item_name = names.get(id).copied().unwrap_or(id);
                if item_name.to_lowercase().contains(&needle) {
                    let divine = line["primaryValue"].as_f64().unwrap_or(0.0);
                    hits.push(PriceHit {
                        name: item_name.to_owned(),
                        category: kind.to_string(),
                        divine,
                        exalted: to_ex(divine),
                        detail: None,
                    });
                }
            }
            if !hits.is_empty() {
                return Ok(hits);
            }
        }
        for kind in UNIQUE_TYPES {
            let v = self.ninja(league, kind, true)?;
            for line in v["lines"].as_array().into_iter().flatten() {
                let item_name = line["name"].as_str().unwrap_or("");
                if item_name.to_lowercase().contains(&needle) {
                    let divine = line["primaryValue"].as_f64().unwrap_or(0.0);
                    hits.push(PriceHit {
                        name: item_name.to_owned(),
                        category: kind.to_string(),
                        divine,
                        exalted: to_ex(divine),
                        detail: Some(format!(
                            "{}{} listings",
                            line["baseType"].as_str().map(|b| format!("{b}, ")).unwrap_or_default(),
                            line["listingCount"].as_u64().unwrap_or(0)
                        )),
                    });
                }
            }
        }
        Ok(hits)
    }
}

fn rank_stats(stats: &[StatEntry], query: &str, limit: usize) -> Vec<StatEntry> {
    let words: Vec<String> = query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1)
        .map(|w| match w {
            "res" | "resist" => "resistance".to_string(),
            "ms" => "movement".to_string(),
            "hp" => "life".to_string(),
            w => w.to_string(),
        })
        .collect();
    let mut scored: Vec<(usize, &StatEntry)> = stats
        .iter()
        .filter_map(|s| {
            let text = s.text.to_lowercase();
            let score = words.iter().filter(|w| text.contains(w.as_str())).count();
            (score == words.len() && score > 0).then_some((score, s))
        })
        .collect();
    let kind_rank = |k: &str| match k {
        "pseudo" => 0,
        "explicit" => 1,
        "implicit" => 2,
        _ => 3,
    };
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then(kind_rank(&a.1.kind).cmp(&kind_rank(&b.1.kind)))
            .then(a.1.text.len().cmp(&b.1.text.len()))
    });
    scored.into_iter().take(limit).map(|(_, s)| s.clone()).collect()
}

/// `requirements: [{name: "Level", values: [["45", 0]]}, {name: "[Dexterity|Dex]", ...}]` → "Level 45, 78 Dex".
fn requirements(item: &Value) -> Option<String> {
    let parts: Vec<String> = item["requirements"]
        .as_array()?
        .iter()
        .filter_map(|r| {
            let name = plain(r["name"].as_str()?);
            let value = r["values"][0][0].as_str()?;
            Some(if name == "Level" {
                format!("Level {value}")
            } else {
                format!("{value} {name}")
            })
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

fn parse_listing(v: &Value) -> Option<Listing> {
    let listing = &v["listing"];
    let item = &v["item"];
    let price = listing["price"]["amount"]
        .as_f64()
        .map(|a| format!("{a} {}", listing["price"]["currency"].as_str().unwrap_or("?")));
    let mut mods = Vec::new();
    for key in [
        "implicitMods",
        "runeMods",
        "enchantMods",
        "explicitMods",
        "craftedMods",
        "desecratedMods",
    ] {
        for m in item[key].as_array().into_iter().flatten() {
            // trade2 returns objects ({description, mods:[{tier}]}); older shapes are strings.
            let text = m.as_str().or_else(|| m["description"].as_str());
            let Some(text) = text else { continue };
            let tier = m["mods"][0]["tier"]
                .as_str()
                .map(|t| format!(" ({t})"))
                .unwrap_or_default();
            mods.push(format!("{}{tier}", plain(text)));
        }
    }
    Some(Listing {
        id: v["id"].as_str()?.to_owned(),
        price,
        seller: listing["account"]["name"].as_str().unwrap_or("?").to_owned(),
        instant_buyout: !listing["fee"].is_null(),
        name: item["name"].as_str().unwrap_or("").to_owned(),
        base: item["typeLine"]
            .as_str()
            .or(item["baseType"].as_str())
            .unwrap_or("")
            .to_owned(),
        item_level: item["ilvl"].as_u64(),
        corrupted: item["corrupted"].as_bool().unwrap_or(false),
        requires: requirements(item),
        mods,
        icon: item["icon"].as_str().map(str::to_owned),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_backoff() {
        let headers = |name: &str| {
            match name {
                "X-Rate-Limit-Rules" => Some("Ip"),
                "X-Rate-Limit-Ip" => Some("5:10:60,15:60:300"),
                "X-Rate-Limit-Ip-State" => Some("4:10:0,3:60:0"),
                _ => None,
            }
            .map(str::to_owned)
        };
        assert_eq!(backoff_from_headers(headers), 10);
        let restricted = |name: &str| {
            match name {
                "X-Rate-Limit-Rules" => Some("Ip"),
                "X-Rate-Limit-Ip" => Some("5:10:60"),
                "X-Rate-Limit-Ip-State" => Some("6:10:60"),
                _ => None,
            }
            .map(str::to_owned)
        };
        assert_eq!(backoff_from_headers(restricted), 60);
        assert_eq!(backoff_from_headers(|_| None), 0);
    }

    #[test]
    fn stat_ranking_prefers_pseudo_totals() {
        let stats = vec![
            StatEntry {
                id: "explicit.stat_4220027924".into(),
                text: "#% to Cold Resistance".into(),
                kind: "explicit".into(),
            },
            StatEntry {
                id: "explicit.stat_3676141501".into(),
                text: "#% to Maximum Cold Resistance".into(),
                kind: "explicit".into(),
            },
            StatEntry {
                id: "pseudo.pseudo_total_cold_resistance".into(),
                text: "+#% total to Cold Resistance".into(),
                kind: "pseudo".into(),
            },
            StatEntry {
                id: "explicit.stat_3299347043".into(),
                text: "# to maximum Life".into(),
                kind: "explicit".into(),
            },
        ];
        let r = rank_stats(&stats, "cold res", 3);
        assert_eq!(r[0].id, "pseudo.pseudo_total_cold_resistance");
        assert_eq!(r.len(), 3);
        assert_eq!(rank_stats(&stats, "max life", 1)[0].id, "explicit.stat_3299347043");
    }

    #[test]
    fn listing_parsing_and_encoding() {
        let v = json!({"id": "abc", "listing": {"price": {"amount": 3, "currency": "exalted"}, "account": {"name": "Seller#1234"}, "fee": 120},
            "item": {"name": "Storm Stride", "typeLine": "Lattice Sandals", "ilvl": 48, "explicitMods": [
                {"description": "+32% to [Resistances|Cold Resistance]", "domain": "explicit", "mods": [{"name": "of the Tundra", "tier": "S3"}]},
                "25% increased Movement Speed"]}});
        let l = parse_listing(&v).unwrap();
        assert!(l.instant_buyout);
        assert_eq!(l.price.as_deref(), Some("3 exalted"));
        assert_eq!(l.mods, ["+32% to Cold Resistance (S3)", "25% increased Movement Speed"]);
        assert_eq!(encode_segment("HC Forbidden Rites"), "HC%20Forbidden%20Rites");
    }
}
