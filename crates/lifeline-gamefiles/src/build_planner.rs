//! The in-game Build Planner (PoE2 0.5) loads JSON `.build` files from
//! `Documents/My Games/Path of Exile 2/BuildPlanner/` and logs
//! `[BuildPlanner] Successfully loaded build 'file:...'` for each one.
//!
//! Schema: GGG's "Build Object (Version 1 - Experimental)" at
//! <https://www.pathofexile.com/developer/docs/game>, cross-checked against
//! real Mobalytics exports (PLAN.md §3.1). The game only imports these files —
//! "Editing or creating builds within Path Of Exile 2 is currently not
//! supported" — so tools are the intended authors. Unknown keys are kept in
//! `extra` so a read → write round trip never drops data.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::Error;

pub const FILE_EXTENSION: &str = "build";

/// Not a documented limit: real exports cut names at 40 characters and the
/// file name mirrors the name, so we keep file names to the same length.
pub const MAX_NAME_CHARS: usize = 40;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlannerBuild {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Ascendancy id, e.g. `Huntress2`; ascendancy passives share it as a
    /// prefix (`AscendancyHuntress2Notable3`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ascendancy: Option<String>,
    #[serde(default)]
    pub inventory_slots: Vec<InventorySlot>,
    pub name: String,
    /// Read from bare id strings or objects; always written as objects.
    #[serde(default, deserialize_with = "ids_or_objects")]
    pub passives: Vec<PassiveRef>,
    #[serde(default, deserialize_with = "ids_or_objects")]
    pub skills: Vec<SkillRef>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventorySlot {
    /// Popup hover text: base type, target mods, notes. Supports [`markup`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_text: Option<String>,
    /// `Inventories` table id. Seen in real files: `Weapon1`, `Weapon2`,
    /// `Offhand1`, `Offhand2`, `Helm1`, `BodyArmour1`, `Gloves1`, `Boots1`,
    /// `Amulet1`, `Belt1`, `Ring1`, `Ring2`, `Flask1`, `Charm1`.
    pub inventory_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level_interval: Option<LevelInterval>,
    /// Position inside multi-cell inventories (flasks, charms). Defaults to 0.
    #[serde(default)]
    pub slot_x: u32,
    #[serde(default)]
    pub slot_y: u32,
    /// `Words` table entry, e.g. `Kalandra's Touch`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unique_name: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassiveRef {
    /// `PassiveSkills` table id — the same id the client logs on allocation
    /// (`Successfully allocated passive skill id: <id>, name: ...`) and the
    /// `id` in GGG's official tree export.
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level_interval: Option<LevelInterval>,
    /// 0–2 per the spec. Real exports use 1 and 2 for weapon-set points and
    /// omit the field for the shared tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weapon_set: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_text: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillRef {
    /// `BaseItemTypes` id. Both `Metadata/Items/Gems/...` and
    /// `Metadata/Items/Gem/...` occur in real exports. Meta gems are not
    /// supported by the game yet.
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level_interval: Option<LevelInterval>,
    /// Hover text in the gem crafting window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_text: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", deserialize_with = "ids_or_objects")]
    pub support_skills: Vec<SupportRef>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupportRef {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level_interval: Option<LevelInterval>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_text: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Character levels during which the planner shows an entry. The spec allows
/// `[min, max]` or a single uint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LevelInterval {
    Range([u32; 2]),
    /// Meaning not documented; treated as "from this level on" (unverified).
    From(u32),
}

impl LevelInterval {
    pub fn contains(&self, level: u32) -> bool {
        match *self {
            Self::Range([min, max]) => (min..=max).contains(&level),
            Self::From(min) => level >= min,
        }
    }
}

trait FromId {
    fn from_id(id: String) -> Self;
}

impl FromId for PassiveRef {
    fn from_id(id: String) -> Self {
        Self {
            id,
            level_interval: None,
            weapon_set: None,
            additional_text: None,
            extra: BTreeMap::new(),
        }
    }
}

impl FromId for SkillRef {
    fn from_id(id: String) -> Self {
        Self {
            id,
            level_interval: None,
            additional_text: None,
            support_skills: Vec::new(),
            extra: BTreeMap::new(),
        }
    }
}

impl FromId for SupportRef {
    fn from_id(id: String) -> Self {
        Self {
            id,
            level_interval: None,
            additional_text: None,
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum IdOr<T> {
    Id(String),
    Full(T),
}

fn ids_or_objects<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + FromId,
{
    let items = Vec::<IdOr<T>>::deserialize(deserializer)?;
    Ok(items
        .into_iter()
        .map(|item| match item {
            IdOr::Id(id) => T::from_id(id),
            IdOr::Full(full) => full,
        })
        .collect())
}

impl PlannerBuild {
    pub fn from_json(text: &str) -> Result<Self, Error> {
        Ok(serde_json::from_str(text)?)
    }

    /// Compact JSON, matching what the game and Mobalytics write.
    pub fn to_json(&self) -> Result<String, Error> {
        Ok(serde_json::to_string(self)?)
    }

    /// Skills shown at `level` (entries without an interval always show).
    pub fn skills_at_level(&self, level: u32) -> impl Iterator<Item = &SkillRef> {
        self.skills
            .iter()
            .filter(move |s| s.level_interval.map_or(true, |li| li.contains(level)))
    }

    pub fn slots_at_level(&self, level: u32) -> impl Iterator<Item = &InventorySlot> {
        self.inventory_slots
            .iter()
            .filter(move |s| s.level_interval.map_or(true, |li| li.contains(level)))
    }

    /// Unique passive ids on the shared tree (no weapon set). Real exports
    /// repeat some ids; what the repeats mean is not documented.
    pub fn shared_passive_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self
            .passives
            .iter()
            .filter(|p| p.weapon_set.is_none())
            .map(|p| p.id.as_str())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

/// Wraps text in one of the documented markup tags, e.g. `markup("red", "Cap
/// resistances first")`. Tags: `r b i u s m l`, colours `red orange yellow
/// green blue indigo violet black white grey bronze silver gold unique`, or
/// `rgb(r, g, b)`.
pub fn markup(tag: &str, text: &str) -> String {
    format!("<{tag}>{{{text}}}")
}

/// `Metadata/Items/Gems/SupportGemRageThree` → `RageThree`.
pub fn gem_short_name(metadata_id: &str) -> &str {
    let last = metadata_id.rsplit('/').next().unwrap_or(metadata_id);
    last.strip_prefix("SkillGem")
        .or_else(|| last.strip_prefix("SupportGem"))
        .unwrap_or(last)
}

/// Name truncated to [`MAX_NAME_CHARS`] with characters Windows forbids in file
/// names replaced, plus the `.build` extension.
pub fn file_name_for(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .take(MAX_NAME_CHARS)
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    format!("{}.{}", cleaned.trim_end_matches(['.', ' ']), FILE_EXTENSION)
}

/// One file from [`read_dir`] with its parse result.
pub type ScannedBuild = (PathBuf, Result<PlannerBuild, Error>);

/// Reads every `.build` file in `dir`. A file that fails to parse is reported
/// alongside the others instead of aborting the whole scan.
pub fn read_dir(dir: &Path) -> Result<Vec<ScannedBuild>, Error> {
    let entries = fs::read_dir(dir).map_err(|source| Error::io(dir, source))?;
    let mut out = Vec::new();
    for entry in entries {
        let path = entry.map_err(|source| Error::io(dir, source))?.path();
        if path.extension().and_then(|e| e.to_str()) != Some(FILE_EXTENSION) {
            continue;
        }
        let parsed = fs::read_to_string(&path)
            .map_err(|source| Error::io(&path, source))
            .and_then(|text| PlannerBuild::from_json(&text));
        out.push((path, parsed));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Writes `build` into `dir` via a temp file + rename so the game never sees a
/// half-written file. Returns the final path.
/// The build as the game itself writes it: the name inside equals the file
/// name (40 characters at most), no description, no built-in weapon attacks
/// (`…PlayerDefault…` aren't gems), and every skill and support carries a
/// level interval. Files that differ are skipped by the in-game planner.
pub fn for_game(build: &PlannerBuild) -> PlannerBuild {
    let mut b = build.clone();
    let file = file_name_for(&build.name);
    b.name = file.trim_end_matches(&format!(".{FILE_EXTENSION}")).to_owned();
    b.description = None;
    b.skills.retain(|s| !s.id.contains("PlayerDefault"));
    for s in &mut b.skills {
        s.level_interval.get_or_insert(LevelInterval::Range([1, 100]));
        for sup in &mut s.support_skills {
            sup.level_interval.get_or_insert(LevelInterval::Range([1, 100]));
        }
    }
    b
}

pub fn write(dir: &Path, build: &PlannerBuild) -> Result<PathBuf, Error> {
    let build = for_game(build);
    let path = dir.join(file_name_for(&build.name));
    let tmp = path.with_extension("build.tmp");
    fs::write(&tmp, build.to_json()?).map_err(|source| Error::io(&tmp, source))?;
    fs::rename(&tmp, &path).map_err(|source| Error::io(&path, source))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("../tests/fixtures/sample.build");

    #[test]
    fn parses_real_export_shape() {
        let b = PlannerBuild::from_json(SAMPLE).unwrap();
        assert_eq!(b.ascendancy.as_deref(), Some("Huntress2"));
        assert_eq!(b.inventory_slots[0].inventory_id, "Weapon1");
        assert_eq!(
            b.inventory_slots[0].level_interval,
            Some(LevelInterval::Range([10, 100]))
        );
        assert_eq!(b.inventory_slots[3].unique_name.as_deref(), Some("Trenchtimbre"));
        assert_eq!(b.passives.iter().filter(|p| p.weapon_set == Some(2)).count(), 1);
        assert_eq!(b.skills[0].support_skills.len(), 2);
        assert!(b.skills[2].support_skills.is_empty());
    }

    #[test]
    fn round_trip_is_lossless() {
        let original: Value = serde_json::from_str(SAMPLE).unwrap();
        let b = PlannerBuild::from_json(SAMPLE).unwrap();
        let again: Value = serde_json::from_str(&b.to_json().unwrap()).unwrap();
        assert_eq!(original, again);
    }

    #[test]
    fn accepts_spec_shorthand_forms() {
        let b = PlannerBuild::from_json(
            r#"{"name":"x","description":"d","passives":["strength89",{"id":"dexterity19","weapon_set":1,"level_interval":12}],
                "skills":["Metadata/Items/Gems/SkillGemEarthquake",{"id":"Metadata/Items/Gems/SkillGemTwister","support_skills":["Metadata/Items/Gems/SupportGemRage"]}]}"#,
        )
        .unwrap();
        assert_eq!(b.passives[0].id, "strength89");
        assert_eq!(b.passives[1].level_interval, Some(LevelInterval::From(12)));
        assert_eq!(b.skills[0].id, "Metadata/Items/Gems/SkillGemEarthquake");
        assert_eq!(b.skills[1].support_skills[0].id, "Metadata/Items/Gems/SupportGemRage");
        assert!(b.to_json().unwrap().contains(r#"{"id":"strength89"}"#));
    }

    #[test]
    fn level_filtering() {
        let b = PlannerBuild::from_json(SAMPLE).unwrap();
        let at_5: Vec<_> = b.skills_at_level(5).map(|s| gem_short_name(&s.id)).collect();
        assert_eq!(at_5, ["WhirlingSlash", "Twister"]);
        assert_eq!(b.skills_at_level(6).count(), 3);
        assert!(LevelInterval::From(12).contains(40));
        assert!(!LevelInterval::From(12).contains(11));
    }

    #[test]
    fn gem_names_handle_both_prefixes() {
        assert_eq!(gem_short_name("Metadata/Items/Gems/SupportGemRageThree"), "RageThree");
        assert_eq!(gem_short_name("Metadata/Items/Gem/SkillGemTwister"), "Twister");
    }

    #[test]
    fn markup_format() {
        assert_eq!(markup("red", "Cap resists"), "<red>{Cap resists}");
    }

    #[test]
    fn file_names_are_truncated_and_sanitized() {
        assert_eq!(
            file_name_for("Act 1 - [0.5] CaptainLance9's Spirit Walker Beast Master"),
            "Act 1 - [0.5] CaptainLance9's Spirit Wal.build"
        );
        assert_eq!(file_name_for("a/b:c?"), "a_b_c_.build");
    }

    #[test]
    fn written_builds_match_the_games_own_format() {
        let mut b = PlannerBuild::from_json(SAMPLE).unwrap();
        b.name = "Act 2 - Silverfist Companion Spirit Walker (Lifeline)".into();
        b.description = Some("x".into());
        b.skills.insert(0, SkillRef { id: "Metadata/Items/Gem/SkillGemPlayerDefaultSpear".into(), level_interval: None, additional_text: None, support_skills: vec![], extra: Default::default() });
        let g = for_game(&b);
        assert!(g.name.chars().count() <= MAX_NAME_CHARS);
        assert_eq!(format!("{}.{FILE_EXTENSION}", g.name), file_name_for(&b.name), "name equals the file name");
        assert!(g.description.is_none());
        assert!(g.skills.iter().all(|s| !s.id.contains("PlayerDefault")));
        assert!(g.skills.iter().all(|s| s.level_interval.is_some() && s.support_skills.iter().all(|x| x.level_interval.is_some())));
    }
}