//! In-game Build Planner files → a staged build. Each `.build` file is one
//! stage ("Act 2 - Name.build"); a set of them with the same build name is
//! one build. This lets Lifeline follow any build in the player's planner,
//! not just ones it created.

use lifeline_data::{GameData, PassiveTree};
use lifeline_gamefiles::build_planner::PlannerBuild;
use lifeline_pob::{Gem, ItemSet, PobBuild, SkillGroup, SkillSet, SlotItem, TreeSpec};

/// Planner inventory slot → PoB slot name (inverse of the writer's map).
fn pob_slot(inventory_id: &str, x: u32) -> Option<&'static str> {
    Some(match (inventory_id, x) {
        ("Weapon1", _) => "Weapon 1",
        ("Offhand1", _) => "Weapon 2",
        ("Weapon2", _) => "Weapon 1 Swap",
        ("Offhand2", _) => "Weapon 2 Swap",
        ("Helm1", _) => "Helmet",
        ("BodyArmour1", _) => "Body Armour",
        ("Gloves1", _) => "Gloves",
        ("Boots1", _) => "Boots",
        ("Amulet1", _) => "Amulet",
        ("Ring1", _) => "Ring 1",
        ("Ring2", _) => "Ring 2",
        ("Belt1", _) => "Belt",
        ("Flask1", 0) => "Flask 1",
        ("Flask1", _) => "Flask 2",
        ("Charm1", 0) => "Charm 1",
        ("Charm1", 1) => "Charm 2",
        ("Charm1", _) => "Charm 3",
        _ => return None,
    })
}

/// Planner hover text ("Base\n1. mod\n2. mod") → PoB-style item text.
fn item_text(unique: Option<&str>, text: Option<&str>) -> String {
    if let Some(u) = unique {
        return format!("Rarity: UNIQUE\n{u}\n{u}\nImplicits: 0");
    }
    let mut lines = text.unwrap_or_default().lines();
    let head = lines.next().unwrap_or("Item").trim().to_owned();
    let mods: Vec<String> = lines
        .map(|l| {
            let l = l.trim();
            l.split_once(". ")
                .filter(|(n, _)| n.chars().all(|c| c.is_ascii_digit()))
                .map_or(l, |(_, rest)| rest)
                .to_owned()
        })
        .filter(|l| !l.is_empty())
        .collect();
    format!("Rarity: RARE\n{head}\n{head}\nImplicits: 0\n{}", mods.join("\n"))
}

/// `stages` are (stage label from the file name, parsed file), in any order.
pub fn from_planner(stages: &[(String, PlannerBuild)], tree: &PassiveTree, data: Option<&GameData>) -> PobBuild {
    let mut build = PobBuild::default();
    let asc_id = stages.iter().find_map(|(_, b)| b.ascendancy.clone());
    if let Some((class, asc)) = asc_id.as_deref().and_then(|a| tree.class_of_ascendancy(a)) {
        build.class_name = Some(class.name.clone());
        build.ascend_class_name = Some(asc.name.clone());
    }
    let gem_name = |id: &str| {
        data.and_then(|d| d.gem_name(id).map(str::to_owned))
            .unwrap_or_else(|| lifeline_gamefiles::build_planner::gem_short_name(id).to_owned())
    };
    let mut item_no = 0u32;
    for (label, b) in stages {
        let mut spec = TreeSpec {
            title: label.clone(),
            tree_version: "0_5".into(),
            ascendancy_internal_id: b.ascendancy.clone(),
            ..Default::default()
        };
        for p in &b.passives {
            let Some(node) = tree.node_by_id(&p.id) else { continue };
            match p.weapon_set {
                Some(1) => spec.weapon_set1.push(node),
                Some(2) => spec.weapon_set2.push(node),
                _ => {}
            }
            spec.nodes.push(node);
        }
        build.specs.push(spec);

        let groups = b
            .skills
            .iter()
            .map(|s| {
                let mut gems = vec![Gem {
                    name: gem_name(&s.id),
                    gem_id: Some(s.id.clone()),
                    enabled: true,
                    ..Default::default()
                }];
                gems.extend(s.support_skills.iter().map(|x| Gem {
                    name: gem_name(&x.id),
                    gem_id: Some(x.id.clone()),
                    enabled: true,
                    ..Default::default()
                }));
                SkillGroup {
                    label: gems[0].name.clone(),
                    enabled: true,
                    slot: None,
                    gems,
                }
            })
            .collect();
        build.skill_sets.push(SkillSet {
            id: None,
            title: label.clone(),
            groups,
        });

        let mut slots = Vec::new();
        for s in &b.inventory_slots {
            let Some(slot) = pob_slot(&s.inventory_id, s.slot_x) else { continue };
            item_no += 1;
            let id = item_no.to_string();
            build
                .items
                .insert(id.clone(), item_text(s.unique_name.as_deref(), s.additional_text.as_deref()));
            slots.push(SlotItem {
                slot: slot.to_owned(),
                item_id: id,
            });
        }
        build.item_sets.push(ItemSet {
            id: None,
            title: label.clone(),
            slots,
        });
    }
    build.active_spec = build.specs.len().checked_sub(1);
    build
}

/// "Act 2 - Silverfist Companion Spirit Walk.build" → ("Act 2", "Silverfist Companion Spirit Walk").
pub fn split_file_name(file: &str) -> (String, String) {
    let base = file.strip_suffix(".build").unwrap_or(file);
    match base.split_once(" - ") {
        Some((stage, name)) => (stage.trim().to_owned(), name.trim().to_owned()),
        None => (String::new(), base.trim().to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_and_items_convert() {
        assert_eq!(
            split_file_name("Act 2 - Silverfist Companion Spirit Walk.build"),
            ("Act 2".into(), "Silverfist Companion Spirit Walk".into())
        );
        assert_eq!(split_file_name("Solo.build"), (String::new(), "Solo".into()));
        let t = item_text(None, Some("Winged Spear\n1. +21 to Accuracy Rating\n2. Adds 7 to 13 Fire Damage"));
        assert_eq!(t, "Rarity: RARE\nWinged Spear\nWinged Spear\nImplicits: 0\n+21 to Accuracy Rating\nAdds 7 to 13 Fire Damage");
        assert!(item_text(Some("Pariah's Embrace"), None).starts_with("Rarity: UNIQUE\nPariah's Embrace"));
        assert_eq!(pob_slot("Charm1", 2), Some("Charm 3"));
    }
}
