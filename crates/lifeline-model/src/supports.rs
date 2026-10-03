//! Skills and support gems a stage can actually use. Skills: an attack
//! must suit the build's weapon (Whirling Assault is a Quarterstaff attack,
//! so a spear build can't use it), each skill shows in the in-game planner
//! from the level its gem is usually cut, and a low-level skill covers a
//! stage's first levels when nothing else can.
//!
//! Supports: The game allows each
//! support gem in only one skill, a skill gem has 2 support sockets until
//! Jeweller's Orbs add more (Lesser 3, Greater 4, Perfect 5), and a support
//! needs an Uncut Support Gem of high enough level to cut. This keeps the
//! planned supports (in order) where they fit those rules, then fills empty
//! sockets with the best remaining compatible supports.

use std::collections::HashSet;

use lifeline_data::game::Gem;
use lifeline_data::GameData;
use lifeline_gamefiles::build_planner::{LevelInterval, SkillRef, SupportRef};
use lifeline_pob::Stage;

/// Support sockets per skill gem by the end of a stage (Jeweller's Orbs:
/// Lesser in the early acts, Greater mid campaign, Perfect late).
pub fn sockets(stage: Stage) -> usize {
    match stage {
        Stage::Act(1) => 2,
        Stage::Act(2) | Stage::Act(3) => 3,
        Stage::Act(_) | Stage::Interludes => 4,
        Stage::Endgame => 5,
    }
}

/// Highest support crafting level cuttable by the end of a stage. Uncut
/// Support Gems come in three tiers: level 1 below area level 35 (Acts 1–2),
/// level 2 from Act 3, level 3 from about area level 54 (Interludes, maps).
/// The data's 1–5 scale maps onto them as 1–2, 3–4 and 5.
pub fn max_crafting_level(stage: Stage) -> u64 {
    match stage {
        Stage::Act(1) | Stage::Act(2) => 2,
        Stage::Act(_) => 4,
        Stage::Interludes | Stage::Endgame => 5,
    }
}

/// "Rapid Attacks II" → "Rapid Attacks": tiers of one support count as one.
fn family(name: &str) -> &str {
    for suffix in [" V", " IV", " III", " II", " I"] {
        if let Some(base) = name.strip_suffix(suffix) {
            return base;
        }
    }
    name
}

/// How well an unplanned support suits a skill: the game's own
/// recommendations first, then supports written for the skill's kind of
/// skill (not generic ones), then shared tags.
fn fit(support: &Gem, skill: &Gem) -> i32 {
    let mut score = 0;
    if skill.recommended_supports.contains(&support.name) {
        score += 100;
    }
    if !support.support_allowed.is_empty() && GameData::is_compatible(support, skill) {
        score += 20;
    }
    let text = support.support_effects.join(" ").to_lowercase();
    for tag in &skill.tags {
        if ["minion", "companion", "attack", "melee", "projectile", "spell", "area", "persistent", "duration"].contains(&tag.as_str())
            && (support.tags.contains(tag) || text.contains(tag.as_str()))
        {
            score += 5;
        }
    }
    // Higher tiers of a family are better when they can be cut.
    score + support.crafting_level as i32
}

/// Item class of a weapon → the gemcutting category of its attacks.
const WEAPON_TYPES: [(&str, &str); 17] = [
    ("Spears", "Spear"),
    ("Quarterstaves", "Quarterstaff"),
    ("Bows", "Bow"),
    ("Crossbows", "Crossbow"),
    ("One Hand Maces", "Mace"),
    ("Two Hand Maces", "Mace"),
    ("Sceptres", "Sceptre"),
    ("Wands", "Wand"),
    ("Staves", "Staff"),
    ("Talismans", "Talisman"),
    ("Flails", "Flail"),
    ("Daggers", "Dagger"),
    ("Claws", "Claw"),
    ("One Hand Swords", "Sword"),
    ("Two Hand Swords", "Sword"),
    ("One Hand Axes", "Axe"),
    ("Two Hand Axes", "Axe"),
];

/// Weapon category for an item class, in either spelling: the game's
/// copied items ("Spears", "Quarterstaves") or the base data ("Spear",
/// "Warstaff").
pub fn weapon_kind(class: &str) -> Option<&'static str> {
    if class == "Warstaff" || class == "Warstaves" {
        return Some("Quarterstaff");
    }
    WEAPON_TYPES
        .iter()
        .find(|(plural, _)| *plural == class || plural.strip_suffix('s') == Some(class) || plural.strip_suffix("es") == Some(class))
        .map(|(_, kind)| *kind)
}

/// The weapon category named in an item or gear goal text ("1. Hardwood
/// Spear" → "Spear"), from the first line that is a weapon base.
pub fn weapon_type(text: &str, data: &GameData) -> Option<&'static str> {
    text.lines().find_map(|line| {
        let line = line.trim();
        let name = line
            .split_once(". ")
            .filter(|(n, _)| n.chars().all(|c| c.is_ascii_digit()))
            .map_or(line, |(_, r)| r);
        let base = data.find_bases(name, 1).into_iter().find(|b| b.name.eq_ignore_ascii_case(name))?;
        weapon_kind(&base.item_class)
    })
}

/// The weapon an attack gem needs (its gemcutting category), if any.
fn weapon_of(gem: &Gem) -> Option<&str> {
    if !gem.skill_types.iter().any(|t| t == "Attack") {
        return None;
    }
    gem.crafting_types
        .first()
        .map(String::as_str)
        .filter(|c| WEAPON_TYPES.iter().any(|(_, k)| k == c))
}

/// Character level a skill gem of this crafting level is usually cut by:
/// level 1 gems from the start, level 7 around character level 12 (a
/// hardcore Huntress had Tame Beast at 13 in Act 1), level 11 from late
/// Act 3 (~42), level 13–14 in Act 4 and later.
pub fn skill_available_from(crafting_level: u64) -> u32 {
    const TABLE: [(u64, u32); 10] =
        [(1, 1), (3, 5), (4, 7), (5, 9), (7, 12), (8, 16), (9, 24), (11, 42), (13, 50), (14, 57)];
    TABLE.iter().rev().find(|(cl, _)| crafting_level >= *cl).map_or(1, |(_, lvl)| *lvl)
}

/// The closest cuttable attack for `weapon` to `like`: most shared skill
/// types (how it plays), then the earliest gem — the weapon's staple skill,
/// with the most levels and supports by then. Names mean nothing here
/// (Rapid Assault is nothing like Whirling Assault).
fn similar_attack<'a>(data: &'a GameData, like: &Gem, weapon: &str, by_level: u32) -> Option<&'a Gem> {
    data.gems()
        .iter()
        .filter(|g| g.kind != "support" && weapon_of(g) == Some(weapon) && g.crafting_level > 0)
        .filter(|g| skill_available_from(g.crafting_level) <= by_level)
        // A main attack, not a movement, cooldown, buff, mark or charge-spending skill.
        .filter(|g| !g.skill_types.iter().any(|t| UTILITY_TYPES.contains(&t.as_str())))
        .max_by_key(|g| {
            let shared = g.skill_types.iter().filter(|t| like.skill_types.contains(t)).count();
            (shared, std::cmp::Reverse(g.crafting_level), std::cmp::Reverse(g.name.clone()))
        })
}

/// Skill types of utility skills a build doesn't swap a main attack for.
const UTILITY_TYPES: [&str; 11] = [
    "Movement", "Travel", "Cooldown", "Buff", "Persistent", "Mark", "Minion",
    "ConsumesCharges", "RequiresCharges", "HasUsageCondition", "OngoingSkill",
];

fn note(skill: &mut SkillRef, text: String) {
    skill.additional_text = Some(match skill.additional_text.take() {
        Some(old) if !old.is_empty() => format!("{old}\n{text}"),
        _ => text,
    });
}

/// Makes a stage's skills usable: attacks swapped to the build's weapon,
/// skills too advanced for the stage swapped for one that isn't, each skill
/// shown from the level its gem is usually cut, and a low-level skill for
/// the stage's first levels when nothing else covers them.
pub fn fit_skills(skills: &mut Vec<SkillRef>, data: &GameData, stage: Stage, weapon: Option<&str>) {
    let (start, end) = lifeline_pob::stage_level_span(stage);
    for skill in skills.iter_mut() {
        let Some(gem) = data.gem_by_id(&skill.id) else { continue };
        if gem.kind == "support" {
            continue;
        }
        let wrong_weapon = matches!((weapon_of(gem), weapon), (Some(w), Some(want)) if w != want);
        let too_late = gem.crafting_level > 0 && skill_available_from(gem.crafting_level) > end;
        if wrong_weapon || too_late {
            let want = weapon.or(weapon_of(gem));
            let swap = want.and_then(|w| similar_attack(data, gem, w, end)).filter(|g| g.id != gem.id);
            if let Some(swap) = swap {
                let why = if wrong_weapon {
                    format!(
                        "Lifeline: {} is a {} skill; this build uses a {}.",
                        gem.name,
                        weapon_of(gem).unwrap_or("different weapon"),
                        weapon.unwrap_or("different weapon")
                    )
                } else {
                    format!(
                        "Lifeline: {} usually can't be cut until about level {}.",
                        gem.name,
                        skill_available_from(gem.crafting_level)
                    )
                };
                skill.id.clone_from(&swap.id);
                note(skill, why);
            }
        }
        let cl = data.gem_by_id(&skill.id).map_or(0, |g| g.crafting_level);
        let from = skill_available_from(cl).max(start);
        if from > start {
            skill.level_interval = Some(LevelInterval::Range([from, 100]));
        }
    }
    // Nothing usable at the stage's first levels: a low-level attack covers them.
    let first = skills
        .iter()
        .filter(|s| data.gem_by_id(&s.id).is_some_and(|g| g.kind != "support"))
        .map(|s| match s.level_interval {
            Some(LevelInterval::Range([from, _])) | Some(LevelInterval::From(from)) => from,
            None => start,
        })
        .min()
        .unwrap_or(start);
    if first > start {
        let like = skills.iter().find_map(|s| data.gem_by_id(&s.id).filter(|g| weapon_of(g).is_some()));
        let cover = match (weapon, like) {
            (Some(w), Some(like)) => similar_attack(data, like, w, start),
            (Some(w), None) => data
                .gems()
                .iter()
                .filter(|g| weapon_of(g) == Some(w) && g.crafting_level == 1)
                .min_by_key(|g| g.name.clone()),
            _ => None,
        };
        if let Some(cover) = cover.filter(|c| !skills.iter().any(|s| s.id == c.id)) {
            skills.insert(
                0,
                SkillRef {
                    id: cover.id.clone(),
                    level_interval: Some(LevelInterval::Range([start, first - 1])),
                    additional_text: Some(format!("Lifeline: use this until level {first}.")),
                    support_skills: Vec::new(),
                    extra: Default::default(),
                },
            );
        }
    }
}

/// Makes every skill's supports usable at `stage`: unique across skills,
/// cuttable by then, at most the stage's socket count, empty sockets filled.
pub fn complete(skills: &mut [SkillRef], data: &GameData, stage: Stage) {
    let max_level = max_crafting_level(stage);
    let slots = sockets(stage);
    let mut used: HashSet<String> = HashSet::new();
    let cuttable = |g: &Gem| g.kind == "support" && !g.is_lineage && (1..=max_level).contains(&g.crafting_level);
    for skill in skills.iter_mut() {
        let Some(gem) = data.gem_by_id(&skill.id) else { continue };
        if gem.kind == "support" {
            continue;
        }
        let mut kept: Vec<SupportRef> = Vec::new();
        for s in std::mem::take(&mut skill.support_skills) {
            let Some(support) = data.gem_by_id(&s.id) else { continue };
            // A planned support too advanced for the stage: its best cuttable tier.
            let pick = if cuttable(support) || support.is_lineage {
                Some(support)
            } else {
                data.find_gems(family(&support.name), 20)
                    .into_iter()
                    .filter(|g| family(&g.name) == family(&support.name) && cuttable(g))
                    .max_by_key(|g| g.crafting_level)
            };
            let Some(pick) = pick else { continue };
            if kept.len() < slots && GameData::may_support(pick, gem) && used.insert(family(&pick.name).to_owned()) {
                kept.push(SupportRef { id: pick.id.clone(), ..s });
            }
        }
        if kept.len() < slots {
            let mut extra: Vec<&Gem> = data
                .supports_for(gem, usize::MAX)
                .into_iter()
                .filter(|g| cuttable(g) && !used.contains(family(&g.name)))
                .collect();
            extra.sort_by_key(|g| (std::cmp::Reverse(fit(g, gem)), g.name.clone()));
            for g in extra {
                if kept.len() >= slots {
                    break;
                }
                if used.insert(family(&g.name).to_owned()) {
                    kept.push(SupportRef {
                        id: g.id.clone(),
                        level_interval: None,
                        additional_text: None,
                        extra: Default::default(),
                    });
                }
            }
        }
        skill.support_skills = kept;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_kinds_in_both_spellings() {
        assert_eq!(weapon_kind("Spear"), Some("Spear"));
        assert_eq!(weapon_kind("Spears"), Some("Spear"));
        assert_eq!(weapon_kind("Warstaff"), Some("Quarterstaff"));
        assert_eq!(weapon_kind("One Hand Mace"), Some("Mace"));
        assert_eq!(weapon_kind("Boots"), None);
    }

    #[test]
    fn skill_gem_availability() {
        assert_eq!(skill_available_from(1), 1);
        assert_eq!(skill_available_from(7), 12);
        assert_eq!(skill_available_from(11), 42);
        assert_eq!(skill_available_from(12), 42);
        assert_eq!(skill_available_from(0), 1);
    }

    #[test]
    fn families_and_stage_limits() {
        assert_eq!(family("Rapid Attacks II"), "Rapid Attacks");
        assert_eq!(family("Loyalty"), "Loyalty");
        assert_eq!(sockets(Stage::Act(1)), 2);
        assert_eq!(sockets(Stage::Endgame), 5);
        assert_eq!(max_crafting_level(Stage::Act(1)), 2);
        assert_eq!(max_crafting_level(Stage::Act(3)), 4);
    }
}
