//! GGG's passive-tree export (`grindinggear/poe2-skilltree-export`,
//! `data.json`). Nodes are keyed by the integer id Path of Building stores;
//! each carries the string `id` the game logs and the Build Planner uses.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

pub const TREE_EXPORT_URL: &str =
    "https://raw.githubusercontent.com/grindinggear/poe2-skilltree-export/master/data.json";

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TreeNode {
    /// `PassiveSkills` id, e.g. `ranger_huntress_notable2`. 240 nodes in the
    /// 0.5.5 export have `"id": null`; those read as empty and are never
    /// written to planner files.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub id: String,
    #[serde(default)]
    pub skill: u32,
    #[serde(default)]
    pub name: String,
    /// Set on ascendancy nodes, e.g. `Ranger1`.
    #[serde(default, rename = "ascendancyId")]
    pub ascendancy_id: Option<String>,
    #[serde(default, rename = "isAscendancyStart")]
    pub is_ascendancy_start: bool,
    /// Present on the class start nodes.
    #[serde(default, rename = "classStartIndex")]
    pub class_start_index: Option<Value>,
    #[serde(default, rename = "isFree")]
    pub is_free: bool,
    #[serde(default, rename = "isNotable")]
    pub is_notable: bool,
    #[serde(default, rename = "isKeystone")]
    pub is_keystone: bool,
}

fn null_as_empty<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

#[derive(Deserialize)]
struct RawExport {
    nodes: HashMap<String, TreeNode>,
}

#[derive(Debug, Clone, Default)]
pub struct PassiveTree {
    nodes: HashMap<u32, TreeNode>,
}

impl PassiveTree {
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        let raw: RawExport = serde_json::from_str(text)?;
        let nodes = raw
            .nodes
            .into_iter()
            .filter_map(|(key, node)| key.parse().ok().map(|k| (k, node)))
            .collect();
        Ok(Self { nodes })
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn node(&self, skill: u32) -> Option<&TreeNode> {
        self.nodes.get(&skill)
    }

    /// Costs a regular passive point: not an ascendancy node, not a class
    /// start, not free. Unknown ids (another tree version) count as points.
    pub fn is_main_point(&self, skill: u32) -> bool {
        self.node(skill).map_or(true, |n| {
            n.ascendancy_id.is_none() && n.class_start_index.is_none() && !n.is_free
        })
    }

    /// Costs an ascendancy point.
    pub fn is_ascendancy_point(&self, skill: u32) -> bool {
        self.node(skill)
            .is_some_and(|n| n.ascendancy_id.is_some() && !n.is_ascendancy_start)
    }

    /// Belongs in a Build Planner passive list (class and ascendancy start
    /// nodes are implicit in game).
    pub fn is_plannable(&self, skill: u32) -> bool {
        self.node(skill)
            .is_some_and(|n| !n.id.is_empty() && n.class_start_index.is_none() && !n.is_ascendancy_start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"tree":"Default","nodes":{
        "4":{"id":"lightning14","skill":4,"name":"Shock Chance"},
        "16":{"id":"AscendancyRanger3Small6","skill":16,"name":"Life Flask Charges","ascendancyId":"Ranger3"},
        "74":{"id":"AscendancyMonk3Start","skill":74,"name":"Acolyte of Chayula","ascendancyId":"Monk3","isAscendancyStart":true},
        "100":{"id":"ranger_start","skill":100,"name":"Ranger","classStartIndex":2},
        "41311":{"id":null,"skill":41311,"name":""},
        "root":{"out":["4"]}
    }}"#;

    #[test]
    fn classifies_nodes_like_the_export() {
        let t = PassiveTree::from_json(SAMPLE).unwrap();
        assert_eq!(t.len(), 5);
        assert!(!t.is_plannable(41311), "null-id nodes are skipped");
        assert_eq!(t.node(4).unwrap().id, "lightning14");
        assert!(t.is_main_point(4) && !t.is_main_point(16) && !t.is_main_point(74) && !t.is_main_point(100));
        assert!(t.is_ascendancy_point(16) && !t.is_ascendancy_point(74));
        assert!(t.is_plannable(4) && t.is_plannable(16) && !t.is_plannable(74) && !t.is_plannable(100));
        assert!(t.is_main_point(99_999));
    }
}
