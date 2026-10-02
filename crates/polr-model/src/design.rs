//! A build designed in the app (the build architect, with the player's
//! answers) turned into a real, checked build:
//! - passive paths are computed from the chosen notables within each stage's
//!   points, so the tree is always connected and affordable;
//! - gems and supports are checked against game data and the game's own
//!   support rules.
//!
//! The result is a [`PobBuild`], so it goes through the same staging, HUD
//! progress, rating and Build Planner export as an imported build.

use std::collections::HashSet;

use polr_data::{AscendancyInfo, GameData, Miss, PassiveTree, TreeNode};
use polr_pob::{
    classify_title, Gem, ItemSet, PobBuild, SkillGroup, SkillSet, SlotItem, Stage, StageHint, TreeSpec, POE2_0_5_ACTS,
};
use serde::{Deserialize, Serialize};

use crate::stage_label;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BuildDesign {
    pub name: String,
    pub class: String,
    #[serde(default)]
    pub ascendancy: Option<String>,
    #[serde(default)]
    pub summary: String,
    /// Target level for the endgame stage (default 90).
    #[serde(default)]
    pub endgame_level: Option<u32>,
    pub stages: Vec<StageDesign>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StageDesign {
    /// "Act 1" … "Act 4", "Interludes", "Endgame".
    pub stage: String,
    /// Passive names to reach, in order (notables, keystones, jewel sockets,
    /// or small passives). Earlier stages carry over.
    #[serde(default)]
    pub passives: Vec<String>,
    /// Ascendancy passive names, in order.
    #[serde(default)]
    pub ascendancy: Vec<String>,
    #[serde(default)]
    pub skills: Vec<SkillDesign>,
    #[serde(default)]
    pub gear: Vec<GearDesign>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SkillDesign {
    pub gem: String,
    #[serde(default)]
    pub supports: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GearDesign {
    /// "Weapon", "Offhand", "Helmet", "Body Armour", "Gloves", "Boots",
    /// "Amulet", "Ring", "Belt".
    pub slot: String,
    #[serde(default)]
    pub unique: Option<String>,
    #[serde(default)]
    pub base: Option<String>,
    /// Stats to look for, e.g. "+60 to maximum Life".
    #[serde(default)]
    pub stats: Vec<String>,
}

/// What a stage can spend, by its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct StageBudget {
    pub level: u32,
    pub points: u32,
    pub ascendancy_points: u32,
}

/// Level and points at the end of `stage`. Main points: about one level
/// over the stage's last quest area, plus the quest points earned by then
/// (PoB2 `QuestRewards.lua`, `POE2_0_5_ACTS`). Ascendancy: 2 per ascension —
/// 1st in Act 2 (Sekhemas), 2nd in Act 3 (Trial of Chaos), 3rd/4th need
/// level 60+/75+ trials, i.e. endgame (PoB2 `ascMax = 8`; maxroll Trials of
/// Ascendancy, 0.5.3).
pub fn stage_budget(stage: Stage, endgame_level: u32) -> Option<StageBudget> {
    let (row, level, ascendancy_points) = match stage {
        Stage::Act(n @ 1..=4) => {
            let row = POE2_0_5_ACTS[n as usize];
            (row, row.area_level + 1, [0, 2, 4, 4][n as usize - 1])
        }
        Stage::Act(_) => return None,
        Stage::Interludes => (POE2_0_5_ACTS[5], POE2_0_5_ACTS[5].area_level + 1, 4),
        Stage::Endgame => (POE2_0_5_ACTS[6], endgame_level.clamp(65, 100), 8),
    };
    Some(StageBudget {
        level,
        points: level - 1 + row.quest_points,
        ascendancy_points,
    })
}

fn parse_stage(s: &str) -> Option<Stage> {
    match classify_title(s, "0_5")? {
        StageHint::Act(n) if (1..=4).contains(&n) => Some(Stage::Act(n)),
        StageHint::Interludes => Some(Stage::Interludes),
        StageHint::Endgame | StageHint::EarlyMaps => Some(Stage::Endgame),
        _ => None,
    }
}

/// Design slot → PoB slot name; the second "Ring" becomes "Ring 2".
fn pob_slot(slot: &str, rings: &mut u8) -> Option<&'static str> {
    let s = slot.trim().to_lowercase();
    Some(match s.as_str() {
        "weapon" | "weapon 1" | "main hand" | "mainhand" | "main-hand" => "Weapon 1",
        "offhand" | "off hand" | "off-hand" | "weapon 2" | "shield" | "buckler" | "quiver" | "focus" => "Weapon 2",
        "helmet" | "helm" => "Helmet",
        "body armour" | "body armor" | "body" | "chest" => "Body Armour",
        "gloves" => "Gloves",
        "boots" => "Boots",
        "amulet" => "Amulet",
        "ring" => {
            *rings += 1;
            if *rings == 1 {
                "Ring 1"
            } else {
                "Ring 2"
            }
        }
        "ring 1" => "Ring 1",
        "ring 2" => "Ring 2",
        "belt" => "Belt",
        _ => return None,
    })
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StageReport {
    pub stage: String,
    pub level: u32,
    pub points_used: u32,
    pub points_budget: u32,
    /// Main points still free at the end of this stage; spend them.
    pub points_unspent: u32,
    pub ascendancy_used: u32,
    pub ascendancy_budget: u32,
    pub reached: Vec<String>,
    /// Targets that didn't make it, with why.
    pub missed: Vec<String>,
    pub skills: Vec<String>,
    pub problems: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DesignReport {
    pub name: String,
    pub class: String,
    pub ascendancy: Option<String>,
    pub stages: Vec<StageReport>,
    /// Problems that stop the design as a whole.
    pub problems: Vec<String>,
}

impl DesignReport {
    /// Everything resolved and every target reached.
    pub fn is_clean(&self) -> bool {
        self.problems.is_empty() && self.stages.iter().all(|s| s.missed.is_empty() && s.problems.is_empty())
    }
}

pub struct Realized {
    pub build: PobBuild,
    pub report: DesignReport,
}

fn listing<'a>(items: impl Iterator<Item = &'a str>) -> String {
    items.collect::<Vec<_>>().join(", ")
}

fn close(near: &str) -> String {
    if near.is_empty() {
        String::new()
    } else {
        format!(" (close: {near})")
    }
}

/// Turns the design into a build. `Err` only when nothing sensible can be
/// built (unknown class/ascendancy, no valid stages); smaller problems are
/// in the report so the designer can fix and retry.
pub fn realize(design: &BuildDesign, tree: &PassiveTree, data: &GameData) -> Result<Realized, String> {
    let (_, class) = tree.class(&design.class).ok_or_else(|| {
        let names = listing(
            tree.classes()
                .iter()
                .filter(|c| c.ascendancies.iter().any(|a| !a.name.is_empty()))
                .map(|c| c.name.as_str()),
        );
        format!("Unknown class '{}'. Classes: {names}.", design.class)
    })?;
    let start = tree.class_start(&class.name).ok_or("The tree data has no start node for that class.")?;
    let ascendancy: Option<&AscendancyInfo> = match design.ascendancy.as_deref().map(str::trim) {
        Some(a) if !a.is_empty() => Some(tree.ascendancy(&class.name, a).ok_or_else(|| {
            let names = listing(class.ascendancies.iter().filter(|x| !x.name.is_empty()).map(|x| x.name.as_str()));
            format!("'{a}' isn't a {} ascendancy. Options: {names}.", class.name)
        })?),
        _ => None,
    };
    let asc_id = ascendancy.map(|a| a.id.clone());

    let mut stages: Vec<(Stage, &StageDesign)> = Vec::new();
    let mut problems = Vec::new();
    for sd in &design.stages {
        match parse_stage(&sd.stage) {
            Some(s) if stages.iter().any(|(x, _)| *x == s) => problems.push(format!("'{}' is listed twice.", sd.stage)),
            Some(s) => stages.push((s, sd)),
            None => problems.push(format!(
                "Unknown stage '{}'. Use Act 1, Act 2, Act 3, Act 4, Interludes, Endgame.",
                sd.stage
            )),
        }
    }
    if stages.is_empty() {
        return Err(format!("No valid stages. {}", problems.join(" ")));
    }
    stages.sort_by_key(|(s, _)| *s);

    let main_usable = |_: u32, n: &TreeNode| {
        !n.id.is_empty()
            && n.ascendancy_id.is_none()
            && n.class_start_index.is_none()
            && !n.is_blighted
            && n.unlock_constraint
                .as_ref()
                .and_then(|u| u.ascendancy.as_deref())
                .map_or(true, |a| asc_id.as_deref() == Some(a))
    };
    let asc_usable = |_: u32, n: &TreeNode| !n.id.is_empty() && n.ascendancy_id.is_some() && n.ascendancy_id == asc_id;
    let main_cost = |n: u32| tree.is_main_point(n);
    let asc_cost = |n: u32| tree.is_ascendancy_point(n);

    let endgame_level = design.endgame_level.unwrap_or(90);
    let mut main_alloc = vec![start];
    let mut asc_alloc: Vec<u32> = asc_id.as_deref().and_then(|a| tree.ascendancy_start(a)).into_iter().collect();
    let (mut main_spent, mut asc_spent) = (0u32, 0u32);

    let mut build = PobBuild {
        class_name: Some(class.name.clone()),
        ascend_class_name: ascendancy.map(|a| a.name.clone()),
        level: Some(endgame_level.clamp(65, 100)),
        notes: design.summary.clone(),
        ..Default::default()
    };
    let mut reports = Vec::new();
    let mut item_no = 0u32;

    for (stage, sd) in &stages {
        let budget = stage_budget(*stage, endgame_level).expect("parsed stages have budgets");
        let label = stage_label(*stage);
        let mut r = StageReport {
            stage: label.clone(),
            level: budget.level,
            points_budget: budget.points,
            ascendancy_budget: budget.ascendancy_points,
            ..Default::default()
        };

        // Main tree: for names shared by several nodes, take the cheapest.
        for name in &sd.passives {
            let candidates: Vec<u32> = tree
                .nodes_named(name, None)
                .into_iter()
                .filter(|&id| tree.node(id).is_some_and(|n| main_usable(id, n)))
                .collect();
            if candidates.is_empty() {
                r.missed.push(format!("{name} (no such passive for this class — check lookup_passive)"));
                continue;
            }
            let set: HashSet<u32> = main_alloc.iter().copied().collect();
            let best = candidates
                .iter()
                .take(24)
                .filter_map(|&c| {
                    let path = tree.path_from(&set, c, &main_usable)?;
                    Some((path.iter().filter(|&&n| main_cost(n)).count(), c))
                })
                .min()
                .map(|(_, c)| c);
            let Some(target) = best else {
                r.missed.push(format!("{name} (not reachable from the {} start)", class.name));
                continue;
            };
            match tree.allocate(&mut main_alloc, target, budget.points - main_spent, &main_usable, &main_cost) {
                Ok(spent) => {
                    main_spent += spent;
                    r.reached.push(name.clone());
                }
                Err(Miss::TooFar { cost, left }) => r.missed.push(format!("{name} (needs {cost} points, {left} left)")),
                Err(Miss::Unreachable) => r.missed.push(format!("{name} (not reachable)")),
            }
        }

        for name in &sd.ascendancy {
            let Some(asc) = asc_id.as_deref() else {
                r.problems.push(format!("{name}: pick an ascendancy first"));
                continue;
            };
            let Some(&target) = tree.nodes_named(name, Some(asc)).first() else {
                r.missed.push(format!("{name} (not a passive of this ascendancy)"));
                continue;
            };
            match tree.allocate(&mut asc_alloc, target, budget.ascendancy_points - asc_spent, &asc_usable, &asc_cost) {
                Ok(spent) => {
                    asc_spent += spent;
                    r.reached.push(name.clone());
                }
                Err(Miss::TooFar { cost, left }) => r.missed.push(format!(
                    "{name} (needs {cost} ascendancy points, {left} left at {label})"
                )),
                Err(Miss::Unreachable) => r.missed.push(format!("{name} (not reachable, or another choice taken)")),
            }
        }
        r.points_used = main_spent;
        r.points_unspent = budget.points - main_spent;
        r.ascendancy_used = asc_spent;

        // Skills: real gems only, supports only where the game allows them.
        let mut groups = Vec::new();
        for sk in &sd.skills {
            let Some(gem) = data.gem_named(&sk.gem) else {
                let near = listing(data.find_gems(&sk.gem, 3).iter().map(|g| g.name.as_str()));
                r.problems.push(format!("Unknown gem '{}'{}", sk.gem, close(&near)));
                continue;
            };
            if gem.kind == "support" {
                r.problems.push(format!("{} is a support gem; list it under a skill", gem.name));
                continue;
            }
            let mut gems = vec![Gem {
                name: gem.name.clone(),
                gem_id: Some(gem.id.clone()),
                enabled: true,
                ..Default::default()
            }];
            let mut used = Vec::new();
            for s in &sk.supports {
                match data.support_named(s).or_else(|| data.gem_named(s)) {
                    None => {
                        let near = listing(data.find_gems(s, 3).iter().filter(|g| g.kind == "support").map(|g| g.name.as_str()));
                        r.problems.push(format!("Unknown support '{s}' on {}{}", gem.name, close(&near)))
                    }
                    Some(sg) if sg.kind != "support" => r.problems.push(format!("{} isn't a support", sg.name)),
                    Some(sg) if !GameData::may_support(sg, gem) => {
                        r.problems.push(format!("{} can't support {} (game rules) — dropped", sg.name, gem.name))
                    }
                    Some(sg) => {
                        used.push(sg.name.clone());
                        gems.push(Gem {
                            name: sg.name.clone(),
                            gem_id: Some(sg.id.clone()),
                            enabled: true,
                            ..Default::default()
                        });
                    }
                }
            }
            r.skills.push(if used.is_empty() {
                gem.name.clone()
            } else {
                format!("{} + {}", gem.name, used.join(", "))
            });
            groups.push(SkillGroup {
                label: gem.name.clone(),
                enabled: true,
                slot: None,
                gems,
            });
        }

        // Gear goals become planner hints (hover text in game).
        let mut slots = Vec::new();
        let mut rings = 0u8;
        for g in &sd.gear {
            let Some(slot) = pob_slot(&g.slot, &mut rings) else {
                r.problems.push(format!("Unknown gear slot '{}'", g.slot));
                continue;
            };
            let base = g.base.clone().unwrap_or_else(|| slot.to_owned());
            let text = match &g.unique {
                Some(u) => {
                    if data.find_uniques(u, 1).first().map_or(true, |x| !x.name.eq_ignore_ascii_case(u.trim())) {
                        r.problems.push(format!("Unknown unique '{u}'"));
                        continue;
                    }
                    format!("Rarity: UNIQUE\n{}\n{base}\nImplicits: 0", u.trim())
                }
                None => format!("Rarity: RARE\n{slot} goal\n{base}\nImplicits: 0\n{}", g.stats.join("\n")),
            };
            item_no += 1;
            let id = item_no.to_string();
            build.items.insert(id.clone(), text);
            slots.push(SlotItem {
                slot: slot.to_owned(),
                item_id: id,
            });
        }

        // A stage without its own skills or gear keeps the previous setup.
        if sd.skills.is_empty() {
            if let Some(prev) = build.skill_sets.last() {
                groups = prev.groups.clone();
                r.skills = vec!["(same as before)".into()];
            }
        }
        if sd.gear.is_empty() {
            if let Some(prev) = build.item_sets.last() {
                slots = prev.slots.clone();
            }
        }

        let mut nodes = main_alloc.clone();
        nodes.extend(&asc_alloc);
        build.specs.push(TreeSpec {
            title: label.clone(),
            tree_version: "0_5".into(),
            ascendancy_internal_id: asc_id.clone(),
            nodes,
            ..Default::default()
        });
        build.skill_sets.push(SkillSet {
            id: None,
            title: label.clone(),
            groups,
        });
        build.item_sets.push(ItemSet {
            id: None,
            title: label,
            slots,
        });
        reports.push(r);
    }
    build.active_spec = Some(build.specs.len() - 1);

    Ok(Realized {
        build,
        report: DesignReport {
            name: design.name.clone(),
            class: class.name.clone(),
            ascendancy: ascendancy.map(|a| a.name.clone()),
            stages: reports,
            problems,
        },
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_follow_pob_quest_data() {
        let b = |s| stage_budget(s, 90).unwrap();
        assert_eq!(b(Stage::Act(1)), StageBudget { level: 13, points: 16, ascendancy_points: 0 });
        assert_eq!(b(Stage::Act(2)).ascendancy_points, 2);
        assert_eq!(b(Stage::Act(3)).points, 44 + 12);
        assert_eq!(b(Stage::Interludes), StageBudget { level: 65, points: 86, ascendancy_points: 4 });
        assert_eq!(b(Stage::Endgame), StageBudget { level: 90, points: 89 + 24, ascendancy_points: 8 });
        assert!(stage_budget(Stage::Act(5), 90).is_none());
    }

    #[test]
    fn stages_and_slots_parse() {
        assert_eq!(parse_stage("Act 3"), Some(Stage::Act(3)));
        assert_eq!(parse_stage("Interludes"), Some(Stage::Interludes));
        assert_eq!(parse_stage("Endgame"), Some(Stage::Endgame));
        assert_eq!(parse_stage("Act 7"), None);
        let mut rings = 0;
        assert_eq!(pob_slot("Ring", &mut rings), Some("Ring 1"));
        assert_eq!(pob_slot("ring", &mut rings), Some("Ring 2"));
        assert_eq!(pob_slot("Body Armor", &mut rings), Some("Body Armour"));
        assert_eq!(pob_slot("Buckler", &mut rings), Some("Weapon 2"));
        assert_eq!(pob_slot("Cape", &mut rings), None);
    }
}
