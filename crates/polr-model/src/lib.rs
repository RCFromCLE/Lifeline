//! From an imported build to playthrough stages and in-game Build Planner
//! files (PLAN.md §5).

use std::collections::BTreeMap;

use polr_data::PassiveTree;
use polr_gamefiles::build_planner::{InventorySlot, PassiveRef, PlannerBuild, SkillRef, SupportRef};
use polr_pob::{classify_title, estimate_level, resolve_stage, ItemSet, PobBuild, SkillSet, Stage, POE2_0_5_ACTS};
use serde::Serialize;

/// One tree spec of the imported build, placed in the playthrough.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpecStage {
    pub spec_index: usize,
    pub title: String,
    pub stage: String,
    #[serde(skip)]
    pub stage_key: Stage,
    pub main_points: u32,
    pub ascendancy_points: u32,
    pub estimated_level: u32,
    /// The spec chosen to represent its stage when writing planner files.
    pub chosen: bool,
}

pub fn stage_label(stage: Stage) -> String {
    match stage {
        Stage::Act(n) => format!("Act {n}"),
        Stage::Interludes => "Interludes".into(),
        Stage::Endgame => "Endgame".into(),
    }
}

/// Places every spec in a stage and marks one spec per stage as chosen: the
/// largest spec of each campaign stage, and for endgame the exported active
/// spec when it is an endgame spec, else the largest.
pub fn plan_stages(build: &PobBuild, tree: Option<&PassiveTree>) -> Vec<SpecStage> {
    let mut out: Vec<SpecStage> = build
        .specs
        .iter()
        .enumerate()
        .map(|(i, spec)| {
            let (main, asc) = match tree {
                Some(t) => (
                    spec.nodes.iter().filter(|&&n| t.is_main_point(n)).count() as u32,
                    spec.nodes.iter().filter(|&&n| t.is_ascendancy_point(n)).count() as u32,
                ),
                None => (spec.nodes.len() as u32, 0),
            };
            let ws = (spec.weapon_set1.len() as u32, spec.weapon_set2.len() as u32);
            let level = estimate_level(&POE2_0_5_ACTS, main, 0, ws);
            let stage = resolve_stage(classify_title(&spec.title, &spec.tree_version), level);
            SpecStage {
                spec_index: i,
                title: spec.title.clone(),
                stage: stage_label(stage),
                stage_key: stage,
                main_points: main,
                ascendancy_points: asc,
                estimated_level: level,
                chosen: false,
            }
        })
        .collect();

    let mut best: BTreeMap<Stage, usize> = BTreeMap::new();
    for (pos, s) in out.iter().enumerate() {
        let entry = best.entry(s.stage_key).or_insert(pos);
        if s.main_points >= out[*entry].main_points {
            *entry = pos;
        }
    }
    if let Some(active) = build.active_spec {
        if out.get(active).is_some_and(|s| s.stage_key == Stage::Endgame) {
            best.insert(Stage::Endgame, active);
        }
    }
    for pos in best.values() {
        out[*pos].chosen = true;
    }
    out
}

fn is_campaign(stage: Stage) -> bool {
    !matches!(stage, Stage::Endgame)
}

/// Picks the set for a stage: a set titled for leveling/acts for campaign
/// stages, one titled for endgame/maps otherwise, else the first set.
fn pick_set<T>(sets: &[T], title: impl Fn(&T) -> &str, stage: Stage) -> Option<&T> {
    let lower = |s: &T| title(s).to_lowercase();
    let campaign_like = |t: &str| t.contains("level") || t.contains("act") || t.contains("campaign");
    let endgame_like = |t: &str| t.contains("end") || t.contains("map") || t.contains("default") || t.is_empty();
    let wanted = sets.iter().find(|s| {
        let t = lower(s);
        if is_campaign(stage) {
            campaign_like(&t)
        } else {
            endgame_like(&t) && !campaign_like(&t)
        }
    });
    wanted.or_else(|| sets.first())
}

fn planner_skills(set: &SkillSet) -> Vec<SkillRef> {
    let mut skills = Vec::new();
    for group in set.groups.iter().filter(|g| g.enabled) {
        let gems: Vec<_> = group.gems.iter().filter(|g| g.enabled && g.gem_id.is_some()).collect();
        let is_support = |id: &str| id.rsplit('/').next().is_some_and(|n| n.starts_with("SupportGem"));
        let supports: Vec<SupportRef> = gems
            .iter()
            .filter_map(|g| g.gem_id.as_deref())
            .filter(|id| is_support(id))
            .map(|id| SupportRef {
                id: id.to_owned(),
                level_interval: None,
                additional_text: None,
                extra: Default::default(),
            })
            .collect();
        for active in gems
            .iter()
            .filter_map(|g| g.gem_id.as_deref())
            .filter(|id| !is_support(id))
        {
            if skills.iter().any(|s: &SkillRef| s.id == active) {
                continue;
            }
            skills.push(SkillRef {
                id: active.to_owned(),
                level_interval: None,
                additional_text: None,
                support_skills: supports.clone(),
                extra: Default::default(),
            });
        }
    }
    skills
}

/// Skills (with supports) the build uses at `stage`.
pub fn skills_for_stage(build: &PobBuild, stage: Stage) -> Vec<SkillRef> {
    pick_set(&build.skill_sets, |s| s.title.as_str(), stage)
        .map(planner_skills)
        .unwrap_or_default()
}

/// PoB slot name → (`inventory_id`, `slot_x`). PoB2's own exporter maps
/// "Weapon 2" to `Offhand1`; charms use `Charm1` as in real exports.
fn inventory_slot(pob_slot: &str) -> Option<(&'static str, u32)> {
    Some(match pob_slot {
        "Weapon 1" => ("Weapon1", 0),
        "Weapon 2" => ("Offhand1", 0),
        "Weapon 1 Swap" => ("Weapon2", 0),
        "Weapon 2 Swap" => ("Offhand2", 0),
        "Helmet" => ("Helm1", 0),
        "Body Armour" => ("BodyArmour1", 0),
        "Gloves" => ("Gloves1", 0),
        "Boots" => ("Boots1", 0),
        "Amulet" => ("Amulet1", 0),
        "Ring 1" => ("Ring1", 0),
        "Ring 2" => ("Ring2", 0),
        "Belt" => ("Belt1", 0),
        "Flask 1" => ("Flask1", 0),
        "Flask 2" => ("Flask1", 1),
        "Charm 1" => ("Charm1", 0),
        "Charm 2" => ("Charm1", 1),
        "Charm 3" => ("Charm1", 2),
        _ => return None,
    })
}

/// Item text → (unique name, planner hover text with base and mods).
fn item_hint(raw: &str) -> (Option<String>, String) {
    let lines: Vec<&str> = raw.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let rarity = lines
        .iter()
        .find_map(|l| l.strip_prefix("Rarity:"))
        .map(|r| r.trim().to_uppercase())
        .unwrap_or_default();
    let header: Vec<&str> = lines
        .iter()
        .skip_while(|l| !l.starts_with("Rarity:"))
        .skip(1)
        .take(2)
        .copied()
        .collect();
    let skip_prefixes = [
        "Rarity:",
        "Unique ID:",
        "Item Level:",
        "LevelReq:",
        "Level:",
        "Quality:",
        "Sockets:",
        "Implicits:",
        "Prefix:",
        "Suffix:",
        "Selected Variant:",
        "Variant:",
        "Rune:",
        "Crafted:",
        "Corrupted",
        "Requires",
    ];
    let mods: Vec<String> = lines
        .iter()
        .skip_while(|l| !l.starts_with("Implicits:"))
        .skip(1)
        .filter(|l| !skip_prefixes.iter().any(|p| l.starts_with(p)))
        .map(|l| {
            let mut s = *l;
            while let Some(rest) = s.strip_prefix('{').and_then(|r| r.split_once('}')).map(|(_, r)| r) {
                s = rest;
            }
            s.to_owned()
        })
        .filter(|l| !l.is_empty())
        .collect();
    let unique = (rarity == "UNIQUE")
        .then(|| header.first().map(|s| s.to_string()))
        .flatten();
    let mut text = header.join("\n");
    for (i, m) in mods.iter().enumerate() {
        text.push_str(&format!("\n{}. {m}", i + 1));
    }
    (unique, text)
}

fn planner_slots(set: &ItemSet, build: &PobBuild) -> Vec<InventorySlot> {
    set.slots
        .iter()
        .filter_map(|slot| {
            let (inventory_id, slot_x) = inventory_slot(&slot.slot)?;
            let raw = build.items.get(&slot.item_id)?;
            let (unique_name, text) = item_hint(raw);
            Some(InventorySlot {
                additional_text: if unique_name.is_some() { None } else { Some(text) },
                inventory_id: inventory_id.to_owned(),
                level_interval: None,
                slot_x,
                slot_y: 0,
                unique_name,
                extra: Default::default(),
            })
        })
        .collect()
}

/// Build Planner file for one chosen spec. `build_name` is appended to the
/// stage label, e.g. "Act 2 - Deadeye (PoLR)"; the file name is cut to 40
/// characters by the writer.
pub fn to_planner_build(
    build: &PobBuild,
    spec_stage: &SpecStage,
    tree: &PassiveTree,
    build_name: &str,
    source_link: Option<&str>,
) -> PlannerBuild {
    let spec = &build.specs[spec_stage.spec_index];
    let passives = spec
        .nodes
        .iter()
        .filter(|&&n| tree.is_plannable(n))
        .filter_map(|&n| {
            let id = tree.node(n)?.id.clone();
            let weapon_set = if spec.weapon_set1.contains(&n) {
                Some(1)
            } else if spec.weapon_set2.contains(&n) {
                Some(2)
            } else {
                None
            };
            Some(PassiveRef {
                id,
                level_interval: None,
                weapon_set,
                additional_text: None,
                extra: Default::default(),
            })
        })
        .collect();
    let skills = pick_set(&build.skill_sets, |s| s.title.as_str(), spec_stage.stage_key)
        .map(planner_skills)
        .unwrap_or_default();
    let inventory_slots = pick_set(&build.item_sets, |s| s.title.as_str(), spec_stage.stage_key)
        .map(|set| planner_slots(set, build))
        .unwrap_or_default();
    PlannerBuild {
        author: Some("PathOfLeastResistance".into()),
        link: source_link.map(str::to_owned),
        description: Some(format!(
            "{} — from \"{}\" (≈ level {}, {} passives). Generated by PathOfLeastResistance.",
            spec_stage.stage, spec_stage.title, spec_stage.estimated_level, spec_stage.main_points
        )),
        ascendancy: spec.ascendancy_internal_id.clone(),
        inventory_slots,
        name: format!("{} - {build_name}", spec_stage.stage),
        passives,
        skills,
        extra: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_hint_reads_pob_item_text() {
        let raw = "Rarity: RARE\nExample Wand\nAttuned Wand\nPrefix: {range:1}SpellDamageOnWeapon8_\nLevelReq: 64\nImplicits: 1\n{range:0.5}Grants Skill: Level (1-20) Mana Drain\n{crafted}+30 to maximum Mana";
        let (unique, text) = item_hint(raw);
        assert_eq!(unique, None);
        assert_eq!(
            text,
            "Example Wand\nAttuned Wand\n1. Grants Skill: Level (1-20) Mana Drain\n2. +30 to maximum Mana"
        );
        let (unique, _) = item_hint("Rarity: UNIQUE\nTrenchtimbre\nPlank Shield\nImplicits: 0");
        assert_eq!(unique.as_deref(), Some("Trenchtimbre"));
    }

    #[test]
    fn slots_map_to_inventory_ids() {
        assert_eq!(inventory_slot("Body Armour"), Some(("BodyArmour1", 0)));
        assert_eq!(inventory_slot("Charm 3"), Some(("Charm1", 2)));
        assert_eq!(inventory_slot("Jewel 61419"), None);
    }

    #[test]
    fn real_build_without_tree_data_still_stages() {
        let code = include_str!("../../polr-pob/tests/fixtures/stormweaver_0_2.pob");
        let build = polr_pob::parse_xml(&polr_pob::decode(code).unwrap()).unwrap();
        let stages = plan_stages(&build, None);
        assert_eq!(stages[0].stage, "Act 1");
        assert_eq!(stages[7].stage, "Endgame");
        assert!(stages[7].chosen, "the exported active spec represents endgame");
        assert_eq!(stages.iter().filter(|s| s.chosen && s.stage == "Act 1").count(), 1);
    }
}
