//! The parts of a PoB2 build XML the planner needs. Element and attribute
//! names follow PoB2's savers (`Build.lua`, `PassiveSpec.lua`,
//! `SkillsTab.lua`, `ItemsTab.lua`); order of sections is not relied on.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;
use roxmltree::{Document, Node};

use crate::Error;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PobBuild {
    pub level: Option<u32>,
    pub class_name: Option<String>,
    pub ascend_class_name: Option<String>,
    /// Tree specs in file order — guide authors use one per stage.
    pub specs: Vec<TreeSpec>,
    /// Index into `specs` that was active when the build was exported.
    pub active_spec: Option<usize>,
    pub skill_sets: Vec<SkillSet>,
    pub item_sets: Vec<ItemSet>,
    /// Raw item text keyed by PoB item id.
    pub items: BTreeMap<String, String>,
    pub config_set_titles: Vec<String>,
    /// Author notes with PoB colour codes left in place.
    pub notes: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TreeSpec {
    /// Colour codes stripped; empty when untitled.
    pub title: String,
    /// `0_1` … `0_5`; selects which tree data the node ids refer to.
    pub tree_version: String,
    /// GGG ascendancy id such as `Ranger1` (absent in older exports).
    pub ascendancy_internal_id: Option<String>,
    /// Integer node ids (`skill` in tree.json), incl. class/ascendancy starts.
    pub nodes: Vec<u32>,
    pub weapon_set1: Vec<u32>,
    pub weapon_set2: Vec<u32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkillSet {
    pub id: Option<String>,
    pub title: String,
    pub groups: Vec<SkillGroup>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkillGroup {
    /// Authors often use gem-less groups whose label is a section header,
    /// e.g. `--Act 1-2 Bows--`.
    pub label: String,
    pub enabled: bool,
    pub slot: Option<String>,
    pub gems: Vec<Gem>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Gem {
    pub name: String,
    /// GGG `BaseItemTypes` id, e.g. `Metadata/Items/Gems/SkillGemSpark` —
    /// the same id the in-game Build Planner uses.
    pub gem_id: Option<String>,
    pub skill_id: Option<String>,
    pub level: Option<u32>,
    pub quality: Option<u32>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemSet {
    pub id: Option<String>,
    pub title: String,
    pub slots: Vec<SlotItem>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SlotItem {
    /// PoB slot name, e.g. `Weapon 1`, `Body Armour`, `Ring 2`.
    pub slot: String,
    pub item_id: String,
}

pub fn parse_xml(xml: &str) -> Result<PobBuild, Error> {
    let doc = Document::parse(xml)?;
    let root = doc.root_element();
    if root.tag_name().name() != "PathOfBuilding2" {
        return Err(Error::WrongRoot(root.tag_name().name().to_owned()));
    }

    let mut build = PobBuild::default();
    for section in root.children().filter(Node::is_element) {
        match section.tag_name().name() {
            "Build" => {
                build.level = attr_u32(section, "level");
                build.class_name = attr(section, "className");
                build.ascend_class_name = attr(section, "ascendClassName").filter(|s| !s.is_empty() && s != "None");
            }
            "Tree" => {
                build.specs = section
                    .children()
                    .filter(|n| n.has_tag_name("Spec"))
                    .map(parse_spec)
                    .collect();
                let count = build.specs.len();
                build.active_spec = attr_u32(section, "activeSpec")
                    .and_then(|n| (n as usize).checked_sub(1))
                    .filter(|&i| i < count);
            }
            "Skills" => build.skill_sets = parse_skill_sets(section),
            "Items" => parse_items(section, &mut build),
            "Config" => {
                build.config_set_titles = section
                    .children()
                    .filter(|n| n.has_tag_name("ConfigSet"))
                    .map(title_of)
                    .collect();
            }
            "Notes" => build.notes = section.text().unwrap_or_default().trim().to_owned(),
            _ => {}
        }
    }
    Ok(build)
}

fn parse_spec(spec: Node) -> TreeSpec {
    let weapon_set = |tag: &str| {
        spec.children()
            .find(|c| c.has_tag_name(tag))
            .and_then(|c| c.attribute("nodes"))
            .map(parse_ids)
            .unwrap_or_default()
    };
    TreeSpec {
        title: title_of(spec),
        tree_version: attr(spec, "treeVersion").unwrap_or_default(),
        ascendancy_internal_id: attr(spec, "ascendancyInternalId").filter(|s| !s.is_empty() && s != "nil"),
        nodes: spec.attribute("nodes").map(parse_ids).unwrap_or_default(),
        weapon_set1: weapon_set("WeaponSet1"),
        weapon_set2: weapon_set("WeaponSet2"),
    }
}

fn parse_skill_sets(skills: Node) -> Vec<SkillSet> {
    let sets: Vec<SkillSet> = skills
        .children()
        .filter(|n| n.has_tag_name("SkillSet"))
        .map(|set| SkillSet {
            id: attr(set, "id"),
            title: title_of(set),
            groups: parse_groups(set),
        })
        .collect();
    if !sets.is_empty() {
        return sets;
    }
    // Older exports keep skill groups directly under <Skills>.
    let groups = parse_groups(skills);
    if groups.is_empty() {
        Vec::new()
    } else {
        vec![SkillSet {
            id: None,
            title: String::new(),
            groups,
        }]
    }
}

fn parse_groups(parent: Node) -> Vec<SkillGroup> {
    parent
        .children()
        .filter(|n| n.has_tag_name("Skill"))
        .map(|skill| SkillGroup {
            label: attr(skill, "label").unwrap_or_default(),
            enabled: attr_bool(skill, "enabled").unwrap_or(true),
            slot: attr(skill, "slot").filter(|s| !s.is_empty()),
            gems: skill
                .children()
                .filter(|g| g.has_tag_name("Gem"))
                .map(|gem| Gem {
                    name: attr(gem, "nameSpec").unwrap_or_default(),
                    gem_id: attr(gem, "gemId").filter(|s| !s.is_empty()),
                    skill_id: attr(gem, "skillId").filter(|s| !s.is_empty()),
                    level: attr_u32(gem, "level"),
                    quality: attr_u32(gem, "quality"),
                    enabled: attr_bool(gem, "enabled").unwrap_or(true),
                })
                .collect(),
        })
        .collect()
}

fn parse_items(items: Node, build: &mut PobBuild) {
    for node in items.children().filter(Node::is_element) {
        match node.tag_name().name() {
            "Item" => {
                if let Some(id) = node.attribute("id") {
                    let text: String = node
                        .children()
                        .filter(|c| c.is_text())
                        .filter_map(|c| c.text())
                        .collect();
                    build.items.insert(id.to_owned(), text.trim().to_owned());
                }
            }
            "ItemSet" => build.item_sets.push(ItemSet {
                id: attr(node, "id"),
                title: title_of(node),
                slots: node
                    .children()
                    .filter(|c| c.has_tag_name("Slot"))
                    .filter_map(|slot| {
                        let item_id = slot.attribute("itemId").filter(|id| *id != "0")?;
                        Some(SlotItem {
                            slot: slot.attribute("name")?.to_owned(),
                            item_id: item_id.to_owned(),
                        })
                    })
                    .collect(),
            }),
            _ => {}
        }
    }
}

/// Removes PoB colour codes (`^7`, `^xRRGGBB`).
pub fn strip_colour_codes(text: &str) -> String {
    static CODES: OnceLock<Regex> = OnceLock::new();
    CODES
        .get_or_init(|| Regex::new(r"\^x[0-9A-Fa-f]{6}|\^[0-9]").expect("static regex"))
        .replace_all(text, "")
        .into_owned()
}

fn title_of(node: Node) -> String {
    node.attribute("title")
        .map(|t| strip_colour_codes(t).trim().to_owned())
        .unwrap_or_default()
}

fn parse_ids(list: &str) -> Vec<u32> {
    list.split(',').filter_map(|id| id.trim().parse().ok()).collect()
}

fn attr(node: Node, name: &str) -> Option<String> {
    node.attribute(name).map(str::to_owned)
}

fn attr_u32(node: Node, name: &str) -> Option<u32> {
    node.attribute(name)?.trim().parse().ok()
}

fn attr_bool(node: Node, name: &str) -> Option<bool> {
    match node.attribute(name)? {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::decode;

    fn load(code: &str) -> PobBuild {
        parse_xml(&decode(code).unwrap()).unwrap()
    }

    #[test]
    fn act_titled_build_0_2() {
        let b = load(include_str!("../tests/fixtures/stormweaver_0_2.pob"));
        assert_eq!(
            (b.class_name.as_deref(), b.ascend_class_name.as_deref()),
            (Some("Sorceress"), Some("Stormweaver"))
        );
        let titles: Vec<_> = b.specs.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Act 1",
                "Act 2",
                "Act 3",
                "Act 4",
                "Act 5",
                "Act 6",
                "Early Maps",
                "Endgame",
                "Endgame w/ Diamonds"
            ]
        );
        assert_eq!(b.specs[0].tree_version, "0_2");
        assert_eq!(b.specs[0].nodes.len(), 25);
        assert_eq!(b.specs[0].weapon_set1, [14363, 61338, 4456, 4776]);
        assert_eq!(b.active_spec, Some(7));
        assert_eq!(b.skill_sets[0].title, "Leveling");
        let spark = &b.skill_sets[0].groups[0].gems[0];
        assert_eq!(
            (spark.name.as_str(), spark.gem_id.as_deref()),
            ("Spark", Some("Metadata/Items/Gems/SkillGemSpark"))
        );
        let set_titles: Vec<_> = b.item_sets.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(set_titles, ["", "Leveling", "Endgame"]);
        // 16 items; one is `<Item variant="195" id="13">`, so attribute order varies.
        assert_eq!(b.items.len(), 16);
        assert!(b.notes.starts_with("^xFFFF77All Example items"));
    }

    #[test]
    fn leveling_numbered_build_0_5() {
        let b = load(include_str!("../tests/fixtures/deadeye_0_5.pob"));
        assert_eq!(b.specs.len(), 14);
        assert_eq!(b.specs[0].title, "Leveling 1 - Bows");
        assert_eq!(b.specs[0].ascendancy_internal_id.as_deref(), Some("Ranger1"));
        assert_eq!(b.specs[0].nodes.len(), 19);
        assert_eq!(b.specs[13].title, "Main Tree 93");
        assert_eq!(b.active_spec, Some(13));
        assert_eq!(b.skill_sets[0].title, "Leveling Skills");
        let header = &b.skill_sets[0].groups[0];
        assert_eq!(header.label, " --Act 1-2 Bows--");
        assert!(header.gems.is_empty());
    }

    #[test]
    fn rejects_non_pob2_xml() {
        assert!(matches!(
            parse_xml("<PathOfBuilding></PathOfBuilding>"),
            Err(Error::WrongRoot(_))
        ));
    }

    #[test]
    fn colour_codes_are_stripped() {
        assert_eq!(strip_colour_codes("^xFF0000Act 1^7 tree"), "Act 1 tree");
    }
}
