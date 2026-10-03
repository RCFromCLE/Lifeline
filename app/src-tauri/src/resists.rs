//! Resistances that matter where the character is: which elements the zone's
//! monsters and boss hit with (researched per zone, `data/area_threats.json`),
//! what each element needs to sit at the 75% cap after the area's penalty,
//! and what the recorded gear and permanent quest rewards give. The passive
//! tree isn't in the game log, so tree resistances aren't counted.

use serde_json::{json, Value};

use crate::state::AppState;

/// Default cap for fire, cold, lightning and chaos.
pub const CAP: i32 = 75;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Totals {
    pub fire: i32,
    pub cold: i32,
    pub lightning: i32,
    pub chaos: i32,
}

/// The first number on a line, ignoring advanced-copy ranges ("27(20-30)%").
fn first_number(line: &str) -> Option<i32> {
    let digits: String = line
        .trim_start_matches(|c: char| !c.is_ascii_digit() && c != '-')
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    digits.parse().ok()
}

/// Adds one item or buff line ("+30% to Cold Resistance", "+12% to all
/// Elemental Resistances", "+15% to Fire and Lightning Resistances").
fn add_line(t: &mut Totals, line: &str) {
    let l = line.to_ascii_lowercase();
    if !l.contains("resistance") || l.contains("maximum") || l.contains("penetrat") || !l.contains('%') {
        return;
    }
    let Some(n) = first_number(line) else { return };
    let n = if l.trim_start().starts_with('-') || l.contains("reduced") { -n.abs() } else { n };
    if l.contains("all elemental resistances") {
        t.fire += n;
        t.cold += n;
        t.lightning += n;
        return;
    }
    if l.contains("fire") {
        t.fire += n;
    }
    if l.contains("cold") {
        t.cold += n;
    }
    if l.contains("lightning") {
        t.lightning += n;
    }
    if l.contains("chaos") {
        t.chaos += n;
    }
}

/// Totals from item texts and buff lines.
pub fn totals<'a>(texts: impl IntoIterator<Item = &'a str>) -> Totals {
    let mut t = Totals::default();
    for text in texts {
        for line in text.lines().filter(|l| !l.trim_start().starts_with('{')) {
            add_line(&mut t, line);
        }
    }
    t
}

/// Per campaign zone: boss, the resistances that matter most (first = most),
/// and why — researched from boss and zone guides for patch 0.5.
fn threats() -> &'static Value {
    static DATA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("../data/area_threats.json")).unwrap_or_default())
}

/// The zone's entry, or a note for places without one (maps, unknown zones).
fn zone_focus(zone_id: &str, area_level: u32) -> Value {
    if let Some(entry) = threats().get(zone_id) {
        let mut entry = entry.clone();
        // Researched with no elemental threat: physical hits are the danger.
        // A low-confidence empty entry just means no guide covers the zone.
        if entry["resists"].as_array().map_or(true, Vec::is_empty) && entry["confidence"] != "low" {
            entry["physical"] = json!(true);
        }
        return entry;
    }
    if area_level >= 65 && !zone_id.starts_with('G') && !zone_id.starts_with('P') {
        return json!({"resists": [], "why": "Maps: the elements depend on the map's own mods; read them before you go in."});
    }
    json!({"resists": [], "why": "No researched data for this zone yet."})
}

/// What the header and HUD show: the zone's important resistances, the
/// capped value needed after the penalty, and what recorded gear gives.
pub fn summary(state: &AppState) -> Value {
    let c = state.character.lock().unwrap().clone();
    let equipped = state.equipped.lock().unwrap().clone();
    let worn: Vec<&str> = equipped
        .iter()
        .filter(|(k, _)| crate::state::is_worn_class(k))
        .map(|(_, v)| v.as_str())
        .collect();
    let have = totals(worn.iter().copied().chain(c.buffs.iter().map(String::as_str)));
    let need = c.res_penalty.map(|p| CAP - p);
    let zone_id = if c.zone_area.is_empty() { c.area_id.clone() } else { c.zone_area.clone() };
    let zone_name = state
        .game
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|d| d.areas.iter().find(|a| a.id == zone_id).map(|a| a.name.clone()))
        .unwrap_or_else(|| c.zone.clone());
    json!({
        "zone": zone_name,
        "zone_id": zone_id,
        "focus": zone_focus(&zone_id, c.area_level),
        "penalty": c.res_penalty,
        "need": need,
        "fire": have.fire, "cold": have.cold, "lightning": have.lightning, "chaos": have.chaos,
        "gear_slots": worn.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_data_is_bundled_and_well_formed() {
        let all = threats().as_object().expect("area_threats.json is an object");
        assert!(all.len() >= 90, "every campaign and interlude zone");
        for (id, e) in all {
            for r in e["resists"].as_array().unwrap() {
                assert!(["fire", "cold", "lightning", "chaos"].contains(&r.as_str().unwrap()), "{id}: {r}");
            }
        }
        assert_eq!(zone_focus("G1_15", 15)["resists"][0], "cold", "Count Geonor");
        assert_eq!(zone_focus("G3_3", 34)["physical"], true, "Mighty Silverfist is physical");
        assert!(zone_focus("MapSomething", 70)["why"].as_str().unwrap().starts_with("Maps"));
    }

    #[test]
    fn adds_up_resistance_lines() {
        let boots = "Item Class: Boots\nRarity: Rare\nGale Stride\nLeather Shoes\n--------\n+30% to Cold Resistance\n+12% to all Elemental Resistances\n+15% to Fire and Lightning Resistances\n+5% to maximum Fire Resistance\n{ Suffix }\n+8(6-10)% to Chaos Resistance";
        let t = totals([boots, "+10% to Cold Resistance"]);
        assert_eq!(t, Totals { fire: 27, cold: 52, lightning: 27, chaos: 8 });
        assert_eq!(first_number("+8(6-10)% to Chaos Resistance"), Some(8));
        let minus = totals(["-10% to Fire Resistance"]);
        assert_eq!(minus.fire, -10);
    }
}
