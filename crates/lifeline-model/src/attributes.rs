//! Which stat to take on each "+5 to any Attribute" passive, and the order
//! a stage's passives are taken in. Planner files and PoB codes don't say
//! which attribute those nodes should be, so Lifeline works it out from what
//! the stage needs: the colour of its skill and support gems, the gear goal
//! bases, and the class's own starting stats.

use std::collections::{BTreeMap, HashSet, VecDeque};

use lifeline_data::{GameData, PassiveTree};
use lifeline_pob::PobBuild;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Attr {
    Str,
    Dex,
    Int,
}

impl Attr {
    const ALL: [Attr; 3] = [Attr::Str, Attr::Dex, Attr::Int];

    fn index(self) -> usize {
        self as usize
    }

    pub fn from_short(s: &str) -> Option<Attr> {
        match s {
            "str" => Some(Attr::Str),
            "dex" => Some(Attr::Dex),
            "int" => Some(Attr::Int),
            _ => None,
        }
    }
}

/// How much the stage leans on each attribute, and what asks for it.
#[derive(Debug, Clone, Default)]
pub struct Needs {
    weight: [f64; 3],
    reasons: [Vec<String>; 3],
}

impl Needs {
    pub fn add(&mut self, attr: Attr, weight: f64, why: &str) {
        self.weight[attr.index()] += weight;
        let list = &mut self.reasons[attr.index()];
        if !why.is_empty() && !list.iter().any(|r| r == why) {
            list.push(why.to_owned());
        }
    }

    pub fn weight(&self, attr: Attr) -> f64 {
        self.weight[attr.index()]
    }
}

/// Gives each attribute node (in the order they're taken) to the attribute
/// with the highest weight ÷ (nodes it already has + 1), so the split follows
/// the needs: a pure Dexterity build gets all Dexterity; Dex 3 : Str 1 gets
/// roughly three Dex for every Str.
pub fn choose(order: &[u32], needs: &Needs) -> Vec<(u32, Attr)> {
    let mut taken = [0u32; 3];
    order
        .iter()
        .map(|&node| {
            let best = Attr::ALL
                .into_iter()
                .max_by(|a, b| {
                    let score = |x: Attr| needs.weight(x) / f64::from(taken[x.index()] + 1);
                    score(*a).total_cmp(&score(*b))
                })
                .unwrap_or(Attr::Dex);
            taken[best.index()] += 1;
            (node, best)
        })
        .collect()
}

/// The order the planned nodes are taken in: outward from the class start
/// through planned nodes (nearest first), then anything not connected that
/// way (other weapon set, or a plan from another tree version).
pub fn allocation_order(tree: &PassiveTree, start: Option<u32>, planned: &HashSet<u32>) -> Vec<u32> {
    let mut order = Vec::new();
    let mut seen = HashSet::new();
    let mut queue: VecDeque<u32> = start.into_iter().collect();
    seen.extend(start);
    while let Some(n) = queue.pop_front() {
        let mut next: Vec<u32> = tree
            .node(n)
            .map(|node| node.out.iter().chain(&node.inbound).copied().collect())
            .unwrap_or_default();
        next.sort_unstable();
        for m in next {
            if planned.contains(&m) && seen.insert(m) {
                order.push(m);
                queue.push_back(m);
            }
        }
    }
    let mut rest: Vec<u32> = planned.iter().copied().filter(|n| !seen.contains(n)).collect();
    rest.sort_unstable();
    order.extend(rest);
    order
}

/// Attribute a weapon or armour base leans on, from its requirements or tags.
fn base_attrs(base: &lifeline_data::game::Base) -> Vec<(Attr, f64)> {
    let req = |k: &str| base.requirements[k].as_f64().unwrap_or(0.0);
    let (s, d, i) = (req("strength"), req("dexterity"), req("intelligence"));
    if s + d + i > 0.0 {
        let total = s + d + i;
        return [(Attr::Str, s), (Attr::Dex, d), (Attr::Int, i)]
            .into_iter()
            .filter(|(_, v)| *v > 0.0)
            .map(|(a, v)| (a, v / total))
            .collect();
    }
    let has = |t: &str| base.tags.iter().any(|x| x == t);
    let mut out = Vec::new();
    for (tag, attrs) in [
        ("str_armour", &[Attr::Str][..]),
        ("dex_armour", &[Attr::Dex]),
        ("int_armour", &[Attr::Int]),
        ("str_dex_armour", &[Attr::Str, Attr::Dex]),
        ("str_int_armour", &[Attr::Str, Attr::Int]),
        ("dex_int_armour", &[Attr::Dex, Attr::Int]),
        ("bow", &[Attr::Dex]),
        ("spear", &[Attr::Dex]),
        ("quarterstaff", &[Attr::Dex]),
        ("claw", &[Attr::Dex]),
        ("dagger", &[Attr::Dex]),
        ("crossbow", &[Attr::Str, Attr::Dex]),
        ("mace", &[Attr::Str]),
        ("axe", &[Attr::Str]),
        ("sword", &[Attr::Str, Attr::Dex]),
        ("wand", &[Attr::Int]),
        ("staff", &[Attr::Int]),
        ("sceptre", &[Attr::Int]),
        ("focus", &[Attr::Int]),
    ] {
        if has(tag) {
            let share = 1.0 / attrs.len() as f64;
            out.extend(attrs.iter().map(|a| (*a, share)));
            break;
        }
    }
    out
}

/// Base type of a PoB-style item text ("Rarity: RARE\nName\nBase\n…").
fn item_base(text: &str) -> Option<&str> {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let rarity = lines.next()?;
    let first = lines.next()?;
    if rarity.contains("UNIQUE") {
        return None;
    }
    if rarity.contains("NORMAL") || rarity.contains("MAGIC") {
        return Some(first);
    }
    lines.next()
}

#[derive(Debug, Clone, Serialize)]
pub struct AttributePlan {
    /// Attribute node → the stat to take there.
    pub choices: BTreeMap<u32, Attr>,
    /// Per attribute: how many nodes, and what asks for it.
    pub summary: Vec<AttrSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttrSummary {
    pub attr: Attr,
    pub nodes: usize,
    pub why: Vec<String>,
}

/// Needs of one stage: its skills and supports, its gear goals, and the
/// class's starting stats (a light tie-breaker).
pub fn stage_needs(
    tree: &PassiveTree,
    build: &PobBuild,
    spec_index: usize,
    stage: lifeline_pob::Stage,
    data: Option<&GameData>,
) -> Needs {
    let mut needs = Needs::default();
    if let Some((_, class)) = build.class_name.as_deref().and_then(|c| tree.class(c)) {
        needs.add(Attr::Str, f64::from(class.base_str) / 5.0, "");
        needs.add(Attr::Dex, f64::from(class.base_dex) / 5.0, "");
        needs.add(Attr::Int, f64::from(class.base_int) / 5.0, "");
    }
    let Some(data) = data else { return needs };
    for skill in crate::skills_for_stage(build, stage) {
        let gems = std::iter::once((&skill.id, 3.0)).chain(skill.support_skills.iter().map(|s| (&s.id, 1.0)));
        for (id, weight) in gems {
            let Some(gem) = data.gem_by_id(id) else { continue };
            let attrs: Vec<Attr> = gem.attribute.split('/').filter_map(Attr::from_short).collect();
            for a in &attrs {
                needs.add(*a, weight / attrs.len() as f64, &gem.name);
            }
        }
    }
    let title = build.specs.get(spec_index).map(|s| s.title.as_str()).unwrap_or_default();
    let set = build.item_sets.iter().find(|s| s.title == title).or(build.item_sets.first());
    for slot in set.map(|s| s.slots.as_slice()).unwrap_or_default() {
        if slot.slot.contains("Swap") || slot.slot.starts_with("Flask") || slot.slot.starts_with("Charm") {
            continue;
        }
        let Some(base_name) = build.items.get(&slot.item_id).and_then(|t| item_base(t)) else { continue };
        let Some(base) = data.find_bases(base_name, 1).into_iter().find(|b| b.name == base_name) else { continue };
        let weight = if slot.slot.starts_with("Weapon") || slot.slot == "Body Armour" { 3.0 } else { 2.0 };
        for (a, share) in base_attrs(base) {
            needs.add(a, weight * share, &base.name);
        }
    }
    needs
}

/// Picks for every attribute node in `order` (the stage's allocation order).
pub fn plan(tree: &PassiveTree, order: &[u32], needs: &Needs) -> AttributePlan {
    let attr_nodes: Vec<u32> = order
        .iter()
        .copied()
        .filter(|n| tree.node(*n).is_some_and(|x| x.is_generic_attribute))
        .collect();
    let choices: BTreeMap<u32, Attr> = choose(&attr_nodes, needs).into_iter().collect();
    let mut summary: Vec<AttrSummary> = Attr::ALL
        .into_iter()
        .map(|attr| AttrSummary {
            attr,
            nodes: choices.values().filter(|a| **a == attr).count(),
            why: needs.reasons[attr.index()].iter().take(4).cloned().collect(),
        })
        .filter(|s| s.nodes > 0)
        .collect();
    summary.sort_by_key(|s| std::cmp::Reverse(s.nodes));
    AttributePlan { choices, summary }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_follow_the_needs() {
        let mut needs = Needs::default();
        needs.add(Attr::Dex, 21.0, "Spear Stab");
        needs.add(Attr::Str, 1.4, "");
        needs.add(Attr::Int, 1.4, "");
        let picks = choose(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], &needs);
        assert!(picks.iter().all(|(_, a)| *a == Attr::Dex), "a pure Dex build stays Dex: {picks:?}");

        let mut mixed = Needs::default();
        mixed.add(Attr::Dex, 9.0, "Bow");
        mixed.add(Attr::Str, 3.0, "Plate");
        let picks = choose(&[1, 2, 3, 4, 5, 6, 7, 8], &mixed);
        let strength = picks.iter().filter(|(_, a)| *a == Attr::Str).count();
        assert_eq!(strength, 2, "3:1 needs give about one Str in four: {picks:?}");
        assert_eq!(picks[0].1, Attr::Dex, "the main need comes first");
    }

    #[test]
    fn item_bases_from_item_text() {
        assert_eq!(item_base("Rarity: RARE\nGale Stride\nQuilted Vest\nImplicits: 0"), Some("Quilted Vest"));
        assert_eq!(item_base("Rarity: NORMAL\nHardwood Spear"), Some("Hardwood Spear"));
        assert_eq!(item_base("Rarity: UNIQUE\nPariah's Embrace\nPariah's Embrace"), None);
    }
}
