//! Game data from RePoE's PoE2 export (`repoe-fork.github.io/poe2`, datamined
//! from the game files; content © GGG). Gems, skill descriptions, item bases,
//! mods, uniques and areas, searchable by name. Downloading and caching is
//! the caller's job; this module parses the JSON files.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;
use serde_json::Value;

pub const REPOE_BASE: &str = "https://repoe-fork.github.io/poe2";
/// Files needed, fetched as `<REPOE_BASE>/<name>.min.json`.
pub const REPOE_FILES: &[&str] = &["skill_gems", "skills", "base_items", "mods", "uniques", "world_areas"];

/// `[Attack|Attacks]` → `Attacks`, `[Curse]` → `Curse`.
pub fn plain(text: &str) -> String {
    static LINKS: OnceLock<Regex> = OnceLock::new();
    LINKS
        .get_or_init(|| Regex::new(r"\[(?:[^\]|]*\|)?([^\]]*)\]").expect("static regex"))
        .replace_all(text, "$1")
        .into_owned()
}

#[derive(Debug, Clone, Serialize)]
pub struct Gem {
    pub id: String,
    pub name: String,
    /// `active`, `spirit` or `support`.
    pub kind: String,
    pub tags: Vec<String>,
    /// Main attribute: str / dex / int (or mixed).
    pub attribute: String,
    pub description: Option<String>,
    pub skill_types: Vec<String>,
    pub is_lineage: bool,
    pub recommended_supports: Vec<String>,
    /// Supports: skill-type rules (reverse-Polish with AND/OR/NOT) from the
    /// game data; a support works on a skill when `allowed` matches its types
    /// and `excluded` doesn't.
    pub support_allowed: Vec<String>,
    pub support_excluded: Vec<String>,
    /// Supports: what they do to the supported skill.
    pub support_effects: Vec<String>,
    /// Actives: cast time and mana cost at gem levels 1 / 10 / 20.
    pub cast_time_ms: Option<u64>,
    pub mana_cost: Vec<(u32, u64)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Base {
    pub id: String,
    pub name: String,
    pub item_class: String,
    pub drop_level: u64,
    pub requirements: Value,
    pub properties: Value,
    pub implicits: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemMod {
    pub id: String,
    pub affix: String,
    pub text: String,
    pub generation_type: String,
    pub required_level: u64,
    /// Item tags it can roll on (spawn weight > 0).
    pub rolls_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Area {
    pub id: String,
    pub name: String,
    pub act: u64,
    pub area_level: u64,
    pub is_town: bool,
    pub has_waypoint: bool,
    pub bosses: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Unique {
    pub name: String,
    pub item_class: String,
}

/// Evaluates a skill-type rule in reverse-Polish form (`["Persistent", "Buff",
/// "AND"]`); leftover values are OR'ed, as the game does.
pub fn type_rule(rule: &[String], types: &[String]) -> bool {
    let mut stack: Vec<bool> = Vec::new();
    for token in rule {
        match token.as_str() {
            "AND" | "OR" => {
                let (b, a) = (stack.pop().unwrap_or(false), stack.pop().unwrap_or(false));
                stack.push(if token == "AND" { a && b } else { a || b });
            }
            "NOT" => {
                let a = stack.pop().unwrap_or(false);
                stack.push(!a);
            }
            t => stack.push(types.iter().any(|x| x == t)),
        }
    }
    stack.into_iter().any(|v| v)
}

#[derive(Debug, Default)]
pub struct GameData {
    pub version: String,
    pub gems: Vec<Gem>,
    pub bases: Vec<Base>,
    pub mods: Vec<ItemMod>,
    pub uniques: Vec<Unique>,
    pub areas: Vec<Area>,
    by_gem_id: HashMap<String, usize>,
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str().map(str::to_owned))
        .collect()
}

/// Name match score: exact 3, prefix 2, every word contained 1, else 0.
pub fn name_score(name: &str, query: &str) -> u8 {
    let (n, q) = (name.to_lowercase(), query.to_lowercase());
    if n == q {
        3
    } else if n.starts_with(&q) {
        2
    } else if q.split_whitespace().all(|w| n.contains(w)) {
        1
    } else {
        0
    }
}

fn best<'a, T>(items: &'a [T], query: &str, name: impl Fn(&T) -> &str, limit: usize) -> Vec<&'a T> {
    let mut scored: Vec<(u8, &T)> = items
        .iter()
        .map(|i| (name_score(name(i), query), i))
        .filter(|(s, _)| *s > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(name(a.1).len().cmp(&name(b.1).len())));
    scored.into_iter().take(limit).map(|(_, i)| i).collect()
}

impl GameData {
    /// Parses the RePoE files (`files` maps file name → JSON text).
    pub fn parse(version: &str, files: &HashMap<String, String>) -> Result<Self, String> {
        let load = |name: &str| -> Result<Value, String> {
            let text = files.get(name).ok_or(format!("{name}.json missing"))?;
            serde_json::from_str(text).map_err(|e| format!("{name}.json: {e}"))
        };
        let skills = load("skills")?;
        let support_rules = |granted: &[String]| -> (Vec<String>, Vec<String>, Vec<String>) {
            for id in granted {
                let s = &skills[id];
                if s["support_gem"].is_object() {
                    let effects = s["stat_sets"][0]["static"]["stat_text"]
                        .as_object()
                        .map(|m| m.values().filter_map(|v| v.as_str().map(plain)).collect())
                        .unwrap_or_default();
                    return (
                        strings(&s["support_gem"]["allowed_types"]),
                        strings(&s["support_gem"]["excluded_types"]),
                        effects,
                    );
                }
            }
            (Vec::new(), Vec::new(), Vec::new())
        };
        let cost_info = |granted: &[String]| -> (Option<u64>, Vec<(u32, u64)>) {
            for id in granted {
                let s = &skills[id];
                if s["active_skill"].is_object() {
                    let costs = [1u32, 10, 20]
                        .iter()
                        .filter_map(|l| s["per_level"][l.to_string()]["costs"]["Mana"].as_u64().map(|c| (*l, c)))
                        .collect();
                    return (s["cast_time"].as_u64(), costs);
                }
            }
            (None, Vec::new())
        };
        // Description from the first active skill; types from all of them
        // (crossbow ammo and other gems grant several skills, and supports
        // apply to any of them).
        let skill_info = |granted: &[String]| -> (Option<String>, Vec<String>) {
            let mut desc = None;
            let mut types: Vec<String> = Vec::new();
            for id in granted {
                let active = &skills[id]["active_skill"];
                if active.is_object() {
                    if desc.is_none() {
                        desc = active["description"].as_str().map(plain);
                    }
                    for t in strings(&active["types"]) {
                        if !types.contains(&t) {
                            types.push(t);
                        }
                    }
                }
            }
            (desc, types)
        };

        let mut gems = Vec::new();
        let raw_gems = load("skill_gems")?;
        let names: HashMap<String, String> = raw_gems
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(id, g)| Some((id.clone(), g["base_item"]["display_name"].as_str()?.to_owned())))
            .collect();
        for (id, g) in raw_gems.as_object().into_iter().flatten() {
            let Some(name) = g["base_item"]["display_name"].as_str() else {
                continue;
            };
            let weights = &g["requirement_weights"];
            let attribute = [("str", "strength"), ("dex", "dexterity"), ("int", "intelligence")]
                .iter()
                .filter(|(_, k)| weights[*k].as_u64().unwrap_or(0) > 0)
                .map(|(s, _)| *s)
                .collect::<Vec<_>>()
                .join("/");
            let kind = g["gem_type"].as_str().unwrap_or("").to_owned();
            let granted = strings(&g["grants_skills"]);
            let (desc, types) = skill_info(&granted);
            let (support_allowed, support_excluded, support_effects) = support_rules(&granted);
            let (cast_time_ms, mana_cost) = cost_info(&granted);
            let description = if kind == "support" {
                g["support_text"].as_str().map(plain)
            } else {
                desc
            };
            gems.push(Gem {
                id: id.clone(),
                name: name.to_owned(),
                kind,
                tags: strings(&g["tags"]),
                attribute,
                description,
                skill_types: types,
                is_lineage: g["is_lineage"].as_bool().unwrap_or(false),
                recommended_supports: strings(&g["recommended_supports"])
                    .iter()
                    .filter_map(|s| names.get(s).cloned())
                    .collect(),
                support_allowed,
                support_excluded,
                support_effects,
                cast_time_ms,
                mana_cost,
            });
        }

        let mut bases = Vec::new();
        for (id, b) in load("base_items")?.as_object().into_iter().flatten() {
            if b["release_state"].as_str() != Some("released") {
                continue;
            }
            let Some(name) = b["name"].as_str() else { continue };
            bases.push(Base {
                id: id.clone(),
                name: name.to_owned(),
                item_class: b["item_class"].as_str().unwrap_or("").to_owned(),
                drop_level: b["drop_level"].as_u64().unwrap_or(0),
                requirements: b["requirements"].clone(),
                properties: b["properties"].clone(),
                implicits: strings(&b["implicits"]),
                tags: strings(&b["tags"]),
            });
        }

        let mut mods = Vec::new();
        for (id, m) in load("mods")?.as_object().into_iter().flatten() {
            let generation_type = m["generation_type"].as_str().unwrap_or("");
            if !matches!(m["domain"].as_str(), Some("item" | "flask" | "misc" | "abyss_jewel"))
                || !matches!(generation_type, "prefix" | "suffix")
            {
                continue;
            }
            let Some(text) = m["text"].as_str() else { continue };
            mods.push(ItemMod {
                id: id.clone(),
                affix: m["name"].as_str().unwrap_or("").to_owned(),
                text: plain(text),
                generation_type: generation_type.to_owned(),
                required_level: m["required_level"].as_u64().unwrap_or(0),
                rolls_on: m["spawn_weights"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|w| w["weight"].as_u64().unwrap_or(0) > 0)
                    .filter_map(|w| w["tag"].as_str().map(str::to_owned))
                    .collect(),
            });
        }

        let uniques_raw = load("uniques")?;
        let unique_iter: Vec<&Value> = match &uniques_raw {
            Value::Array(a) => a.iter().collect(),
            Value::Object(o) => o.values().collect(),
            _ => Vec::new(),
        };
        let mut uniques: Vec<Unique> = unique_iter
            .into_iter()
            .filter(|u| !u["is_alternate_art"].as_bool().unwrap_or(false))
            .filter_map(|u| {
                Some(Unique {
                    name: u["name"].as_str()?.to_owned(),
                    item_class: u["item_class"].as_str().unwrap_or("").to_owned(),
                })
            })
            .collect();
        uniques.sort_by(|a, b| a.name.cmp(&b.name));
        uniques.dedup_by(|a, b| a.name == b.name);

        let mut areas = Vec::new();
        for (id, a) in load("world_areas")?.as_object().into_iter().flatten() {
            let Some(name) = a["name"].as_str() else { continue };
            areas.push(Area {
                id: id.clone(),
                name: name.to_owned(),
                act: a["act"].as_u64().unwrap_or(0),
                area_level: a["area_level"].as_u64().unwrap_or(0),
                is_town: a["is_town"].as_bool().unwrap_or(false),
                has_waypoint: a["has_waypoint"].as_bool().unwrap_or(false),
                bosses: strings(&a["bosses"]),
            });
        }

        let by_gem_id = gems.iter().enumerate().map(|(i, g)| (g.id.clone(), i)).collect();
        Ok(Self {
            version: version.to_owned(),
            gems,
            bases,
            mods,
            uniques,
            areas,
            by_gem_id,
        })
    }

    pub fn gem_by_id(&self, id: &str) -> Option<&Gem> {
        self.by_gem_id.get(id).map(|&i| &self.gems[i])
    }

    /// Display name for a gem metadata id (`SupportGemMartialTempo` → `Rapid Attacks I`).
    pub fn gem_name(&self, id: &str) -> Option<&str> {
        self.gem_by_id(id).map(|g| g.name.as_str())
    }

    pub fn find_gems(&self, query: &str, limit: usize) -> Vec<&Gem> {
        best(&self.gems, query, |g| &g.name, limit)
    }

    /// The gem whose display name is exactly `name` (case-insensitive).
    pub fn gem_named(&self, name: &str) -> Option<&Gem> {
        let name = name.trim();
        self.gems.iter().find(|g| g.name.eq_ignore_ascii_case(name))
    }

    /// Whether `support` can support `skill`, by the game's own type rules.
    pub fn is_compatible(support: &Gem, skill: &Gem) -> bool {
        // No allowed list = a generic support (Efficiency, Lifetap): anything
        // not excluded.
        support.kind == "support"
            && (support.support_allowed.is_empty() || type_rule(&support.support_allowed, &skill.skill_types))
            && !(!support.support_excluded.is_empty() && type_rule(&support.support_excluded, &skill.skill_types))
    }

    /// Whether `skill` makes minions or companions. Supports on such gems
    /// support the minions' own skills, whose types the data doesn't list,
    /// so the type rules can't rule a support out.
    pub fn is_minion_skill(skill: &Gem) -> bool {
        skill.skill_types.iter().any(|t| t == "Minion" || t == "Companion")
    }

    /// Meta gems (Blasphemy, Cast on …) pass supports to the skills socketed
    /// in them, so those can't be ruled out either.
    pub fn is_meta_skill(skill: &Gem) -> bool {
        skill.skill_types.iter().any(|t| t == "Meta")
    }

    /// `is_compatible`, or not ruled out because `skill` is a minion skill.
    pub fn may_support(support: &Gem, skill: &Gem) -> bool {
        Self::is_compatible(support, skill)
            || (support.kind == "support" && (Self::is_minion_skill(skill) || Self::is_meta_skill(skill)))
    }

    /// Every support that works with `skill` (game rules; for minion skills,
    /// every support not ruled out), the game's recommended ones first, then
    /// lineage, then by name.
    pub fn supports_for(&self, skill: &Gem, limit: usize) -> Vec<&Gem> {
        let mut found: Vec<&Gem> = self.gems.iter().filter(|g| Self::may_support(g, skill)).collect();
        found.sort_by_key(|g| {
            (
                !skill.recommended_supports.contains(&g.name),
                !g.is_lineage,
                g.name.clone(),
            )
        });
        found.into_iter().take(limit).collect()
    }
    fn is_equipment_class(class: &str) -> bool {
        !class.is_empty()
            && ![
                "Gem",
                "Hideout",
                "Microtransaction",
                "Quest",
                "Map",
                "Currency",
                "Fragment",
            ]
            .iter()
            .any(|x| class.contains(x))
    }

    fn class_matches(class: &str, query: &str) -> bool {
        let (c, q) = (class.to_lowercase(), query.to_lowercase());
        let q = q.trim();
        c == q || c == q.trim_end_matches('s') || c.replace(' ', "") == q.replace(' ', "")
    }

    /// Bases by name, or every base of an item class ("boots", "spear") by
    /// drop level.
    pub fn find_bases(&self, query: &str, limit: usize) -> Vec<&Base> {
        let mut of_class: Vec<&Base> = self
            .bases
            .iter()
            .filter(|b| Self::is_equipment_class(&b.item_class) && Self::class_matches(&b.item_class, query))
            .collect();
        if !of_class.is_empty() {
            of_class.sort_by_key(|b| b.drop_level);
            return of_class.into_iter().take(limit).collect();
        }
        let equipment: Vec<&Base> = self
            .bases
            .iter()
            .filter(|b| Self::is_equipment_class(&b.item_class))
            .collect();
        let mut scored: Vec<(u8, &Base)> = equipment
            .into_iter()
            .map(|b| (name_score(&b.name, query), b))
            .filter(|(s, _)| *s > 0)
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.drop_level.cmp(&b.1.drop_level)));
        scored.into_iter().take(limit).map(|(_, b)| b).collect()
    }

    /// Tags of every base in an item class ("boots" → boots, armour, …).
    fn slot_tags<'a>(&'a self, slot: &'a str) -> Vec<&'a str> {
        let mut tags: Vec<&str> = self
            .bases
            .iter()
            .filter(|b| Self::class_matches(&b.item_class, slot))
            .flat_map(|b| b.tags.iter().map(String::as_str))
            .filter(|t| *t != "default")
            .collect();
        tags.sort_unstable();
        tags.dedup();
        if tags.is_empty() {
            tags.push(slot);
        }
        tags
    }
    /// Mods whose text contains every word of `query`, optionally only those
    /// that can roll on an item class (`slot`: e.g. `boots`, `ring`, `body armour`).
    pub fn find_mods(&self, query: &str, slot: Option<&str>, limit: usize) -> Vec<&ItemMod> {
        let tags = slot.map(|s| self.slot_tags(s));
        let words: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_owned).collect();
        let mut found: Vec<&ItemMod> = self
            .mods
            .iter()
            .filter(|m| {
                let t = m.text.to_lowercase();
                words.iter().all(|w| t.contains(w.as_str()))
            })
            .filter(|m| {
                tags.as_ref()
                    .map_or(true, |tags| m.rolls_on.iter().any(|r| tags.contains(&r.as_str())))
            })
            .collect();
        found.sort_by(|a, b| a.required_level.cmp(&b.required_level).then(a.text.cmp(&b.text)));
        found.into_iter().take(limit).collect()
    }

    pub fn find_uniques(&self, query: &str, limit: usize) -> Vec<&Unique> {
        best(&self.uniques, query, |u| &u.name, limit)
    }

    pub fn find_areas(&self, query: &str, limit: usize) -> Vec<&Area> {
        if let Some(a) = self.areas.iter().find(|a| a.id.eq_ignore_ascii_case(query)) {
            return vec![a];
        }
        best(&self.areas, query, |a| &a.name, limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files() -> HashMap<String, String> {
        let mut f = HashMap::new();
        f.insert("skill_gems".into(), r#"{
            "Metadata/Items/Gems/SkillGemSpark": {"base_item": {"display_name": "Spark", "id": "Metadata/Items/Gems/SkillGemSpark", "release_state": "released"},
                "gem_type": "active", "grants_skills": ["SparkPlayer"], "requirement_weights": {"dexterity": 0, "intelligence": 100, "strength": 0},
                "tags": ["spell", "projectile", "lightning"], "recommended_supports": ["Metadata/Items/Gems/SupportGemPierce"]},
            "Metadata/Items/Gems/SupportGemPierce": {"base_item": {"display_name": "Pierce I", "id": "Metadata/Items/Gems/SupportGemPierce", "release_state": "released"},
                "gem_type": "support", "grants_skills": ["SupportPiercePlayer"], "requirement_weights": {"dexterity": 100, "intelligence": 0, "strength": 0},
                "tags": ["support", "projectile"], "support_text": "Supports [Projectile|Projectile] skills, making them Pierce."},
            "Metadata/Items/Gems/SupportGemMartialTempo": {"base_item": {"display_name": "Rapid Attacks I", "id": "Metadata/Items/Gems/SupportGemMartialTempo", "release_state": "released"},
                "gem_type": "support", "grants_skills": ["SupportMartialTempoPlayer"], "requirement_weights": {"dexterity": 100, "intelligence": 0, "strength": 0},
                "tags": ["support"], "support_text": "Supports [Attack|Attacks], causing them to [Attack] faster."}
        }"#.into());
        f.insert("skills".into(), r#"{"SparkPlayer": {"active_skill": {"description": "Launch a spray of sparking [Projectile|Projectiles].", "types": ["Spell", "Projectile", "Lightning"]}, "cast_time": 700, "per_level": {"1": {"costs": {"Mana": 5}}, "20": {"costs": {"Mana": 56}}}},
            "SupportPiercePlayer": {"support_gem": {"allowed_types": ["Projectile"], "excluded_types": ["ProjectileNoCollision"]}, "stat_sets": [{"static": {"stat_text": {"a": "[Projectile|Projectiles] from Supported Skills [Pierce] an Enemy"}}}]},
            "SupportMartialTempoPlayer": {"support_gem": {"allowed_types": ["Attack"], "excluded_types": []}}}"#.into());
        f.insert("base_items".into(), r#"{"Metadata/Items/Armours/Boots/BootsDex1": {"name": "Lattice Sandals", "item_class": "Boots", "drop_level": 4, "release_state": "released", "implicits": [], "tags": ["boots", "armour", "default"], "properties": {"evasion": {"min": 22, "max": 22}}, "requirements": {"level": 4}}}"#.into());
        f.insert("mods".into(), r#"{"ColdResist1": {"domain": "item", "generation_type": "suffix", "name": "of the Seal", "text": "+(6-10)% to [Resistances|Cold Resistance]", "required_level": 1, "spawn_weights": [{"tag": "armour", "weight": 1}, {"tag": "default", "weight": 0}]}}"#.into());
        f.insert(
            "uniques".into(),
            r#"[{"name": "Bramblejack", "item_class": "Body Armour", "is_alternate_art": false}]"#.into(),
        );
        f.insert("world_areas".into(), r#"{"G1_1": {"name": "The Riverbank", "act": 1, "area_level": 1, "is_town": false, "has_waypoint": true, "bosses": []}}"#.into());
        f
    }

    #[test]
    fn type_rules_evaluate_like_the_game() {
        let t = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(type_rule(
            &t(&["Persistent", "Buff", "AND"]),
            &t(&["Buff", "Persistent", "Spell"])
        ));
        assert!(!type_rule(&t(&["Persistent", "Buff", "AND"]), &t(&["Buff"])));
        assert!(type_rule(
            &t(&["Bear", "Wolf", "OR", "Wyvern", "OR", "Shapeshift", "AND"]),
            &t(&["Wolf", "Shapeshift"])
        ));
        assert!(
            type_rule(&t(&["Attack", "Spell"]), &t(&["Spell"])),
            "leftovers are OR'ed"
        );
        assert!(!type_rule(&t(&["Attack", "NOT"]), &t(&["Attack"])));
    }

    #[test]
    fn parses_and_searches() {
        let d = GameData::parse("4.5.5.2", &files()).unwrap();
        let spark = d.find_gems("spark", 5)[0];
        assert_eq!(
            spark.description.as_deref(),
            Some("Launch a spray of sparking Projectiles.")
        );
        assert_eq!(spark.attribute, "int");
        assert_eq!(
            d.gem_name("Metadata/Items/Gems/SupportGemMartialTempo"),
            Some("Rapid Attacks I")
        );
        let supports = d.supports_for(spark, 5);
        assert_eq!(supports.len(), 1, "Rapid Attacks needs Attack");
        assert_eq!(supports[0].name, "Pierce I");
        assert_eq!(
            supports[0].support_effects,
            ["Projectiles from Supported Skills Pierce an Enemy"]
        );
        assert_eq!(
            (spark.cast_time_ms, spark.mana_cost.clone()),
            (Some(700), vec![(1, 5), (20, 56)])
        );
        assert_eq!(d.find_bases("boots", 5)[0].name, "Lattice Sandals");
        assert_eq!(
            d.find_mods("cold res", Some("boots"), 5)[0].text,
            "+(6-10)% to Cold Resistance"
        );
        assert!(d.find_mods("cold res", Some("ring"), 5).is_empty());
        assert_eq!(d.find_areas("G1_1", 3)[0].name, "The Riverbank");
        assert_eq!(d.find_uniques("bramble", 3)[0].item_class, "Body Armour");
    }
}
