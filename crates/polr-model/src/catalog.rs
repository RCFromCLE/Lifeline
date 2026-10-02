//! The researched build catalog: hardcore-viable endgame archetypes per
//! class and ascendancy, shipped with the app (`data/builds.json`). The
//! build wizard filters and ranks these; the build architect turns the
//! chosen one into a full staged build. Every name is checked against game
//! data by [`check`].

use polr_data::{GameData, PassiveTree, TreeNode};
use serde::{Deserialize, Serialize};

use crate::design::SkillDesign;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Ratings {
    /// 1 (low) – 5 (high) unless noted.
    pub damage: u8,
    pub tankiness: u8,
    pub clear_speed: u8,
    pub bossing: u8,
    /// 1 = works on almost no currency, 5 = needs expensive uniques.
    pub budget: u8,
    /// 1 = press one button, 5 = demanding.
    pub complexity: u8,
    /// How safe it is to take through endgame on one life.
    pub hardcore: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Archetype {
    /// kebab-case, unique: `huntress-amazon-lightning-spear`.
    pub id: String,
    pub class: String,
    pub ascendancy: String,
    pub name: String,
    /// Two sentences at most.
    pub summary: String,
    /// melee, ranged, caster, minion, totem, trap, …
    pub style: Vec<String>,
    /// physical, lightning, cold, fire, chaos, plus mechanics: crit, bleed,
    /// poison, ignite, shock, freeze, …
    pub damage: Vec<String>,
    /// life, energy shield, armour, evasion, block, deflection, …
    pub defences: Vec<String>,
    pub ratings: Ratings,
    pub main_skill: String,
    /// Endgame skill setups (the main skill first).
    pub skills: Vec<SkillDesign>,
    /// What to use while levelling, in order (Act 1 first).
    #[serde(default)]
    pub leveling: Vec<SkillDesign>,
    #[serde(default)]
    pub keystones: Vec<String>,
    /// Notables the tree is built around.
    #[serde(default)]
    pub key_passives: Vec<String>,
    /// Ascendancy passives in the order to take them.
    pub ascendancy_passives: Vec<String>,
    #[serde(default)]
    pub key_uniques: Vec<String>,
    /// Weapon type and gear priorities, briefly.
    #[serde(default)]
    pub gear: String,
    #[serde(default)]
    pub strengths: Vec<String>,
    #[serde(default)]
    pub weaknesses: Vec<String>,
    #[serde(default)]
    pub league_start: bool,
    pub sources: Vec<String>,
}

/// Problems with one archetype's names and numbers (empty = good).
pub fn check(a: &Archetype, tree: &PassiveTree, data: &GameData) -> Vec<String> {
    let mut p = Vec::new();
    if a.id.is_empty() || a.id.contains(' ') {
        p.push(format!("id '{}' must be kebab-case", a.id));
    }
    let asc_id = match tree.ascendancy(&a.class, &a.ascendancy) {
        Some(asc) => Some(asc.id.clone()),
        None => {
            p.push(format!("'{}' isn't a released {} ascendancy", a.ascendancy, a.class));
            None
        }
    };
    let r = &a.ratings;
    for (k, v) in [
        ("damage", r.damage),
        ("tankiness", r.tankiness),
        ("clear_speed", r.clear_speed),
        ("bossing", r.bossing),
        ("budget", r.budget),
        ("complexity", r.complexity),
        ("hardcore", r.hardcore),
    ] {
        if !(1..=5).contains(&v) {
            p.push(format!("rating {k} must be 1–5"));
        }
    }
    if a.sources.is_empty() {
        p.push("needs at least one source URL".into());
    }
    let check_skill = |s: &SkillDesign, p: &mut Vec<String>| {
        let Some(gem) = data.gem_named(&s.gem) else {
            let near: Vec<&str> = data.find_gems(&s.gem, 3).iter().map(|g| g.name.as_str()).collect();
            p.push(format!("unknown gem '{}' (close: {})", s.gem, near.join(", ")));
            return;
        };
        if gem.kind == "support" {
            p.push(format!("'{}' is a support, not a skill", gem.name));
            return;
        }
        for sup in &s.supports {
            match data.support_named(sup).or_else(|| data.gem_named(sup)) {
                None => {
                    let near: Vec<&str> = data
                        .find_gems(sup, 4)
                        .iter()
                        .filter(|g| g.kind == "support")
                        .map(|g| g.name.as_str())
                        .collect();
                    p.push(format!("unknown support '{sup}' (close: {})", near.join(", ")));
                }
                Some(sg) if !GameData::may_support(sg, gem) => {
                    p.push(format!("'{}' can't support '{}' (game rules)", sg.name, gem.name))
                }
                Some(_) => {}
            }
        }
    };
    if data.gem_named(&a.main_skill).is_none() {
        p.push(format!("main_skill '{}' isn't a gem name", a.main_skill));
    }
    if a.skills.is_empty() {
        p.push("needs skills".into());
    }
    for s in a.skills.iter().chain(&a.leveling) {
        check_skill(s, &mut p);
    }
    let main_ok = |n: &TreeNode| {
        n.ascendancy_id.is_none()
            && n.unlock_constraint
                .as_ref()
                .and_then(|u| u.ascendancy.as_deref())
                .map_or(true, |x| asc_id.as_deref() == Some(x))
    };
    for name in a.keystones.iter().chain(&a.key_passives) {
        let found = tree.nodes_named(name, None);
        if !found.iter().any(|&id| tree.node(id).is_some_and(main_ok)) {
            p.push(format!("passive '{name}' not found in the main tree (use the exact name)"));
        }
    }
    if let Some(asc) = asc_id.as_deref() {
        for name in &a.ascendancy_passives {
            if tree.nodes_named(name, Some(asc)).is_empty() {
                p.push(format!("'{name}' isn't a {} passive", a.ascendancy));
            }
        }
    }
    for u in &a.key_uniques {
        if !data.find_uniques(u, 1).first().is_some_and(|x| x.name.eq_ignore_ascii_case(u.trim())) {
            p.push(format!("unique '{u}' not found"));
        }
    }
    p
}

/// The catalog shipped with the app (researched; see `data/builds.json`).
pub fn bundled() -> Vec<Archetype> {
    serde_json::from_str(include_str!("../data/builds.json")).unwrap_or_default()
}

/// What the player picked in the build wizard.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub ascendancy: Option<String>,
    /// "damage", "balanced" (default) or "tanky".
    #[serde(default)]
    pub focus: Option<String>,
    /// Any of melee, ranged, caster, minion; empty = any.
    #[serde(default)]
    pub styles: Vec<String>,
    /// "ssf", "low" or "high".
    #[serde(default)]
    pub budget: Option<String>,
    /// "simple" or "any".
    #[serde(default)]
    pub complexity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ranked {
    pub archetype: Archetype,
    /// 0–100 fit for the preferences (hardcore safety always counts).
    pub score: u32,
    /// Short reasons it fits (or doesn't).
    pub reasons: Vec<String>,
}

fn eq(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// Filters the catalog to the class/ascendancy/styles and ranks the rest.
pub fn rank(list: &[Archetype], p: &Preferences) -> Vec<Ranked> {
    let any = |o: &Option<String>| o.as_deref().map_or(true, |s| s.is_empty() || eq(s, "any"));
    let mut out: Vec<Ranked> = list
        .iter()
        .filter(|a| any(&p.class) || eq(&a.class, p.class.as_deref().unwrap_or_default()))
        .filter(|a| any(&p.ascendancy) || eq(&a.ascendancy, p.ascendancy.as_deref().unwrap_or_default()))
        .filter(|a| p.styles.is_empty() || p.styles.iter().any(|s| a.style.iter().any(|x| eq(x, s))))
        .map(|a| {
            let r = &a.ratings;
            // Weights: damage, bossing, clear, tankiness, hardcore.
            let w: [u32; 5] = match p.focus.as_deref() {
                Some("damage") => [3, 2, 2, 1, 2],
                Some("tanky") => [1, 1, 1, 3, 4],
                _ => [2, 2, 2, 2, 3],
            };
            let vals = [r.damage, r.bossing, r.clear_speed, r.tankiness, r.hardcore].map(u32::from);
            let got: u32 = w.iter().zip(vals).map(|(w, v)| w * v).sum();
            let max: u32 = w.iter().map(|w| w * 5).sum();
            let mut score = (got * 100 / max) as i32;
            let mut reasons = Vec::new();
            match p.budget.as_deref() {
                Some("ssf") => {
                    score -= (i32::from(r.budget) - 1) * 6;
                    if r.budget <= 2 {
                        reasons.push("Works without trading".to_owned());
                    }
                }
                Some("low") => {
                    score -= (i32::from(r.budget) - 2).max(0) * 8;
                    if r.budget >= 4 {
                        reasons.push("Needs expensive gear".to_owned());
                    }
                }
                _ => {}
            }
            if p.complexity.as_deref() == Some("simple") {
                score -= (i32::from(r.complexity) - 2).max(0) * 8;
            }
            if a.league_start && matches!(p.budget.as_deref(), Some("ssf" | "low")) {
                score += 4;
            }
            if r.hardcore >= 4 {
                reasons.push(format!("Hardcore-safe ({}/5)", r.hardcore));
            } else if r.hardcore <= 2 {
                reasons.push(format!("Risky in hardcore ({}/5)", r.hardcore));
            }
            if r.tankiness >= 4 {
                reasons.push("Tanky".to_owned());
            }
            if r.damage >= 4 {
                reasons.push("Big damage".to_owned());
            }
            if r.complexity <= 2 {
                reasons.push("Easy to play".to_owned());
            }
            Ranked {
                archetype: a.clone(),
                score: score.clamp(0, 100) as u32,
                reasons,
            }
        })
        .collect();
    out.sort_by(|a, b| b.score.cmp(&a.score).then(b.archetype.ratings.hardcore.cmp(&a.archetype.ratings.hardcore)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arch(id: &str, style: &str, damage: u8, tank: u8, hc: u8, budget: u8) -> Archetype {
        Archetype {
            id: id.into(),
            class: "Huntress".into(),
            ascendancy: "Amazon".into(),
            style: vec![style.into()],
            ratings: Ratings {
                damage,
                tankiness: tank,
                clear_speed: 3,
                bossing: 3,
                budget,
                complexity: 2,
                hardcore: hc,
            },
            ..Default::default()
        }
    }

    #[test]
    fn ranking_follows_focus_style_and_budget() {
        let list = vec![arch("glass", "ranged", 5, 1, 2, 2), arch("wall", "melee", 2, 5, 5, 2), arch("rich", "ranged", 4, 4, 4, 5)];
        let tanky = rank(&list, &Preferences { focus: Some("tanky".into()), ..Default::default() });
        assert_eq!(tanky[0].archetype.id, "wall");
        let dmg = rank(&list, &Preferences { focus: Some("damage".into()), budget: Some("high".into()), ..Default::default() });
        assert_eq!(dmg[0].archetype.id, "rich");
        let cheap = rank(&list, &Preferences { styles: vec!["ranged".into()], budget: Some("ssf".into()), ..Default::default() });
        assert_eq!(cheap.len(), 2, "style filters out melee");
        assert!(cheap.iter().all(|r| r.archetype.style == ["ranged"]));
        let other = rank(&list, &Preferences { class: Some("Witch".into()), ..Default::default() });
        assert!(other.is_empty());
    }

    #[test]
    fn bundled_catalog_parses() {
        let text = include_str!("../data/builds.json");
        let parsed: Result<Vec<Archetype>, _> = serde_json::from_str(text);
        assert!(parsed.is_ok(), "data/builds.json doesn't match the schema: {:?}", parsed.err());
    }
}