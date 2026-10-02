//! GGG's passive-tree export (`grindinggear/poe2-skilltree-export`,
//! `data.json`). Nodes are keyed by the integer id Path of Building stores;
//! each carries the string `id` the game logs and the Build Planner uses.

use std::collections::{HashMap, HashSet, VecDeque};

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
    #[serde(default)]
    pub stats: Vec<String>,
    #[serde(default, deserialize_with = "node_ids")]
    pub out: Vec<u32>,
    #[serde(default, rename = "in", deserialize_with = "node_ids")]
    pub inbound: Vec<u32>,
    #[serde(default, rename = "isJewelSocket")]
    pub is_jewel_socket: bool,
    /// Only granted by blight anointments; never on a path.
    #[serde(default, rename = "isBlighted")]
    pub is_blighted: bool,
    /// One of several "choose one" ascendancy nodes under `choice_parent`.
    #[serde(default, rename = "isMultipleChoiceOption")]
    pub is_choice_option: bool,
    #[serde(default, rename = "multipleChoiceParent", deserialize_with = "node_id_opt")]
    pub choice_parent: Option<u32>,
    /// Nodes only some ascendancies can take (`{"ascendancy": "Druid1"}`).
    #[serde(default, rename = "unlockConstraint")]
    pub unlock_constraint: Option<UnlockConstraint>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UnlockConstraint {
    #[serde(default)]
    pub ascendancy: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClassInfo {
    #[serde(default, deserialize_with = "null_as_empty")]
    pub name: String,
    #[serde(default)]
    pub ascendancies: Vec<AscendancyInfo>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AscendancyInfo {
    /// GGG id the Build Planner stores, e.g. `Huntress1`.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub id: String,
    /// Empty for ascendancies not in the game yet.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub name: String,
}

fn as_node_id(v: &Value) -> Option<u32> {
    match v {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        _ => None,
    }
}

fn node_ids<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<u32>, D::Error> {
    Ok(Vec::<Value>::deserialize(d)?.iter().filter_map(as_node_id).collect())
}

fn node_id_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    Ok(Option::<Value>::deserialize(d)?.as_ref().and_then(as_node_id))
}

fn null_as_empty<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

#[derive(Deserialize)]
struct RawExport {
    nodes: HashMap<String, TreeNode>,
    #[serde(default)]
    classes: Vec<ClassInfo>,
}

#[derive(Debug, Clone, Default)]
pub struct PassiveTree {
    nodes: HashMap<u32, TreeNode>,
    /// Undirected links (`out` ∪ `in`).
    links: HashMap<u32, Vec<u32>>,
    classes: Vec<ClassInfo>,
}

/// Why a target passive couldn't be added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Miss {
    /// Reachable, but needs this many points and fewer are left.
    TooFar { cost: u32, left: u32 },
    Unreachable,
}

impl PassiveTree {
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        let raw: RawExport = serde_json::from_str(text)?;
        let nodes: HashMap<u32, TreeNode> = raw
            .nodes
            .into_iter()
            .filter_map(|(key, node)| key.parse().ok().map(|k| (k, node)))
            .collect();
        let mut links: HashMap<u32, Vec<u32>> = HashMap::new();
        for (&id, node) in &nodes {
            for &other in node.out.iter().chain(&node.inbound) {
                if other != id && nodes.contains_key(&other) {
                    links.entry(id).or_default().push(other);
                    links.entry(other).or_default().push(id);
                }
            }
        }
        for list in links.values_mut() {
            list.sort_unstable();
            list.dedup();
        }
        Ok(Self {
            nodes,
            links,
            classes: raw.classes,
        })
    }

    pub fn classes(&self) -> &[ClassInfo] {
        &self.classes
    }

    /// A class by name, case-insensitive (`Huntress`).
    pub fn class(&self, name: &str) -> Option<(usize, &ClassInfo)> {
        self.classes
            .iter()
            .enumerate()
            .find(|(_, c)| c.name.eq_ignore_ascii_case(name.trim()))
    }

    /// The class's start node (classes share starts in pairs).
    pub fn class_start(&self, class: &str) -> Option<u32> {
        let (index, _) = self.class(class)?;
        self.nodes
            .iter()
            .find(|(_, n)| {
                n.class_start_index
                    .as_ref()
                    .and_then(Value::as_array)
                    .is_some_and(|a| a.iter().any(|v| v.as_u64() == Some(index as u64)))
            })
            .map(|(&id, _)| id)
    }

    /// A released ascendancy of `class` by name or id (`Amazon`, `Huntress1`).
    pub fn ascendancy(&self, class: &str, name: &str) -> Option<&AscendancyInfo> {
        let name = name.trim();
        self.class(class)?
            .1
            .ascendancies
            .iter()
            .filter(|a| !a.name.is_empty())
            .find(|a| a.name.eq_ignore_ascii_case(name) || a.id.eq_ignore_ascii_case(name))
    }

    pub fn ascendancy_start(&self, ascendancy_id: &str) -> Option<u32> {
        self.nodes
            .iter()
            .find(|(_, n)| n.is_ascendancy_start && n.ascendancy_id.as_deref() == Some(ascendancy_id))
            .map(|(&id, _)| id)
    }

    /// Nodes named exactly `name` (case-insensitive) in the main tree, or in
    /// one ascendancy when `ascendancy` is set. Notables and keystones first.
    pub fn nodes_named(&self, name: &str, ascendancy: Option<&str>) -> Vec<u32> {
        let name = name.trim();
        let mut found: Vec<(u32, &TreeNode)> = self
            .nodes
            .iter()
            .filter(|(_, n)| !n.id.is_empty() && n.name.eq_ignore_ascii_case(name))
            .filter(|(_, n)| n.ascendancy_id.as_deref() == ascendancy)
            .map(|(&id, n)| (id, n))
            .collect();
        found.sort_by_key(|(id, n)| (!n.is_keystone, !n.is_notable, *id));
        found.into_iter().map(|(id, _)| id).collect()
    }

    /// Shortest path (fewest new nodes) from any node in `allocated` to
    /// `target`, through nodes `usable` allows. Returns the new nodes,
    /// ending with `target`.
    pub fn path_from(&self, allocated: &HashSet<u32>, target: u32, usable: &dyn Fn(u32, &TreeNode) -> bool) -> Option<Vec<u32>> {
        if allocated.contains(&target) {
            return Some(Vec::new());
        }
        let mut prev: HashMap<u32, u32> = HashMap::new();
        let mut queue: VecDeque<u32> = allocated.iter().copied().collect();
        let mut seen: HashSet<u32> = allocated.clone();
        while let Some(at) = queue.pop_front() {
            for &next in self.links.get(&at).map(Vec::as_slice).unwrap_or_default() {
                if seen.contains(&next) || !self.nodes.get(&next).is_some_and(|n| usable(next, n)) {
                    continue;
                }
                seen.insert(next);
                prev.insert(next, at);
                if next == target {
                    let mut path = vec![target];
                    let mut cur = target;
                    while let Some(&p) = prev.get(&cur) {
                        if allocated.contains(&p) {
                            break;
                        }
                        path.push(p);
                        cur = p;
                    }
                    path.reverse();
                    return Some(path);
                }
                // Choice options are dead ends: never walk through one.
                if !self.nodes[&next].is_choice_option {
                    queue.push_back(next);
                }
            }
        }
        None
    }

    /// Allocates `target` (and the path to it) if it fits in `left` points,
    /// counting points with `cost`. Returns the points spent.
    pub fn allocate(
        &self,
        allocated: &mut Vec<u32>,
        target: u32,
        left: u32,
        usable: &dyn Fn(u32, &TreeNode) -> bool,
        cost: &dyn Fn(u32) -> bool,
    ) -> Result<u32, Miss> {
        let set: HashSet<u32> = allocated.iter().copied().collect();
        // Only one option per "choose one" parent.
        if let Some(parent) = self.node(target).and_then(|n| n.choice_parent) {
            if allocated.iter().any(|&a| a != target && self.node(a).is_some_and(|n| n.choice_parent == Some(parent))) {
                return Err(Miss::Unreachable);
            }
        }
        let path = self.path_from(&set, target, usable).ok_or(Miss::Unreachable)?;
        let spent = path.iter().filter(|&&n| cost(n)).count() as u32;
        if spent > left {
            return Err(Miss::TooFar { cost: spent, left });
        }
        allocated.extend(path);
        Ok(spent)
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

    /// Passives whose name or stat text contains every word of `query`;
    /// keystones and notables first.
    pub fn find_passives(&self, query: &str, limit: usize) -> Vec<&TreeNode> {
        let words: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_owned).collect();
        let mut found: Vec<&TreeNode> = self
            .nodes
            .values()
            .filter(|n| !n.id.is_empty() && !n.name.is_empty())
            .filter(|n| {
                let hay = format!("{} {}", n.name, n.stats.join(" ")).to_lowercase();
                words.iter().all(|w| hay.contains(w.as_str()))
            })
            .collect();
        found.sort_by_key(|n| (!n.is_keystone, !n.is_notable, n.ascendancy_id.is_some(), n.name.clone()));
        found.dedup_by(|a, b| a.name == b.name && a.stats == b.stats);
        found.into_iter().take(limit).collect()
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

    // Two classes' starts S (class 0) and T (class 1); S—a—b—N, T—c—b; the
    // ascendancy X starts at AS—x1—XN. Ids: S=1 a=2 b=3 N=4 T=5 c=6 AS=7 x1=8 XN=9.
    const PATHS: &str = r#"{"classes":[{"name":"Alpha","ascendancies":[{"id":"Alpha1","name":"Xer"},{"id":"Alpha2","name":""}]},{"name":"Beta"}],
      "nodes":{
        "1":{"id":"s","skill":1,"name":"ALPHA","classStartIndex":[0],"out":["2"]},
        "2":{"id":"a","skill":2,"name":"Small","out":["3"]},
        "3":{"id":"b","skill":3,"name":"Small","out":["4"],"in":["6"]},
        "4":{"id":"n","skill":4,"name":"Big Notable","isNotable":true},
        "5":{"id":"t","skill":5,"name":"BETA","classStartIndex":[1],"out":["6"]},
        "6":{"id":"c","skill":6,"name":"Small"},
        "7":{"id":"as","skill":7,"name":"Xer","ascendancyId":"Alpha1","isAscendancyStart":true,"out":["8"]},
        "8":{"id":"x1","skill":8,"name":"Asc Small","ascendancyId":"Alpha1","out":["9"]},
        "9":{"id":"xn","skill":9,"name":"Asc Notable","ascendancyId":"Alpha1","isNotable":true}
      }}"#;

    #[test]
    fn paths_respect_budgets_and_other_starts() {
        let t = PassiveTree::from_json(PATHS).unwrap();
        assert_eq!(t.class_start("alpha"), Some(1));
        assert_eq!(t.class_start("Beta"), Some(5));
        assert_eq!(t.ascendancy("Alpha", "xer").map(|a| a.id.as_str()), Some("Alpha1"));
        assert!(t.ascendancy("Alpha", "Alpha2").is_none(), "unreleased ascendancies are hidden");
        assert_eq!(t.nodes_named("big notable", None), vec![4]);

        let main = |_: u32, n: &TreeNode| n.ascendancy_id.is_none() && n.class_start_index.is_none();
        let cost = |n: u32| t.is_main_point(n);
        let mut alloc = vec![1];
        assert_eq!(t.allocate(&mut alloc, 4, 2, &main, &cost), Err(Miss::TooFar { cost: 3, left: 2 }));
        assert_eq!(t.allocate(&mut alloc, 4, 3, &main, &cost), Ok(3));
        assert_eq!(alloc, vec![1, 2, 3, 4]);
        // c hangs off b; T (another class's start) is never walked through.
        assert_eq!(t.allocate(&mut alloc, 6, 5, &main, &cost), Ok(1));
        let mut from_t = vec![5];
        assert_eq!(t.allocate(&mut from_t, 4, 9, &main, &cost), Ok(3), "T—c—b—N");

        let start = t.ascendancy_start("Alpha1").unwrap();
        let asc = |_: u32, n: &TreeNode| n.ascendancy_id.as_deref() == Some("Alpha1");
        let mut alloc = vec![start];
        assert_eq!(t.allocate(&mut alloc, 9, 2, &asc, &|n| t.is_ascendancy_point(n)), Ok(2));
        assert_eq!(t.allocate(&mut vec![1], 9, 9, &main, &cost), Err(Miss::Unreachable));
    }
}