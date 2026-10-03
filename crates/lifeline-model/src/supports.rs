//! Support gems a stage's skills can actually use. The game allows each
//! support gem in only one skill, a skill gem has 2 support sockets until
//! Jeweller's Orbs add more (Lesser 3, Greater 4, Perfect 5), and a support
//! needs an Uncut Support Gem of high enough level to cut. This keeps the
//! planned supports (in order) where they fit those rules, then fills empty
//! sockets with the best remaining compatible supports.

use std::collections::HashSet;

use lifeline_data::game::Gem;
use lifeline_data::GameData;
use lifeline_gamefiles::build_planner::{SkillRef, SupportRef};
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
    fn families_and_stage_limits() {
        assert_eq!(family("Rapid Attacks II"), "Rapid Attacks");
        assert_eq!(family("Loyalty"), "Loyalty");
        assert_eq!(sockets(Stage::Act(1)), 2);
        assert_eq!(sockets(Stage::Endgame), 5);
        assert_eq!(max_crafting_level(Stage::Act(1)), 2);
        assert_eq!(max_crafting_level(Stage::Act(3)), 4);
    }
}
