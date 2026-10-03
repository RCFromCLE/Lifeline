//! Turning a build's titled specs into playthrough stages (PLAN.md §5).
//!
//! Two signals are combined: what the guide author called the spec, and how
//! many passive points it spends (PoB's own level estimate). Explicit act
//! labels win; everything else falls back to the estimated level.

use std::sync::OnceLock;

use regex::Regex;

/// Where in the playthrough a stage sits. Campaign layout of patch 0.5:
/// Acts 1–4, three Interludes, then endgame from area level 65. 1.0 adds
/// Acts 5–6 and removes the Interludes, so this must become data-driven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    Act(u8),
    Interludes,
    Endgame,
}

/// What a spec/set title says about its stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageHint {
    Act(u8),
    /// Pre-0.3 trees (`0_1`, `0_2`) used "Act 4–6" for Cruel Acts 1–3.
    Cruel(u8),
    Interludes,
    EarlyMaps,
    Endgame,
    /// "Level 75", "lvl 90".
    Level(u32),
    /// "Leveling 3" — an ordinal within a leveling sequence.
    LevelingStep(u32),
    /// "Leveling" / "Campaign" without a number.
    Leveling,
}

struct TitlePatterns {
    interlude: Regex,
    act: Regex,
    early_maps: Regex,
    level: Regex,
    leveling_step: Regex,
    endgame: Regex,
    leveling: Regex,
}

fn patterns() -> &'static TitlePatterns {
    static P: OnceLock<TitlePatterns> = OnceLock::new();
    P.get_or_init(|| {
        let re = |s: &str| Regex::new(s).expect("static regex");
        TitlePatterns {
            interlude: re(r"(?i)\binterludes?\b"),
            act: re(r"(?i)\bact\s*(\d{1,2})\b"),
            early_maps: re(r"(?i)\b(?:early|start(?:ing)?)\s*maps?\b"),
            level: re(r"(?i)\b(?:level|lvl)\s*(\d{1,3})\b"),
            leveling_step: re(r"(?i)\blevell?ing\s*(\d{1,2})\b"),
            endgame: re(r"(?i)\b(?:end\s*-?\s*game|maps?|mapping|pinnacles?|bossing|main\s*tree|final)\b"),
            leveling: re(r"(?i)\b(?:levell?ing|campaign)\b"),
        }
    })
}

/// Reads a stage hint from a title. `tree_version` disambiguates eras: in
/// `0_1`/`0_2` trees "Act 4–6" meant Cruel; in `0_3`–`0_5` guides (Maxroll)
/// use "Act 5" for the Interludes.
pub fn classify_title(title: &str, tree_version: &str) -> Option<StageHint> {
    let p = patterns();
    if p.interlude.is_match(title) {
        return Some(StageHint::Interludes);
    }
    if let Some(c) = p.act.captures(title) {
        let n: u8 = c[1].parse().ok()?;
        return Some(match tree_version {
            "0_1" | "0_2" if (4..=6).contains(&n) => StageHint::Cruel(n - 3),
            "0_3" | "0_4" | "0_5" if n == 5 => StageHint::Interludes,
            _ => StageHint::Act(n),
        });
    }
    if p.early_maps.is_match(title) {
        return Some(StageHint::EarlyMaps);
    }
    if let Some(c) = p.level.captures(title) {
        return c[1].parse().ok().map(StageHint::Level);
    }
    if let Some(c) = p.leveling_step.captures(title) {
        return c[1].parse().ok().map(StageHint::LevelingStep);
    }
    if p.endgame.is_match(title) {
        return Some(StageHint::Endgame);
    }
    if p.leveling.is_match(title) {
        return Some(StageHint::Leveling);
    }
    None
}

/// Campaign area-level bands for 0.5 (poe2wiki Act pages / RePoE
/// world_areas): Act 1 1–15, Act 2 16–31 (town 32), Act 3 33–45, Act 4
/// 46–53, Interludes 54–64, endgame 65+. Character level tracks area level
/// closely during the campaign, so this maps an estimated level to a stage.
/// Character level at the end of each stage at a safe hardcore pace:
/// Act 1 16–18, Act 2 30–32, Act 3 46–48 (owner, from play, 2026-10-03),
/// Act 4 56–58 and Interludes 63–65 (speedrun guides for 0.5.5 end the acts
/// at 14 / 29 / 42 / 52 / 59; careful hardcore play runs 3–5 levels over),
/// maps from 65. `POE2_0_5_ACTS` holds monster area levels where each act
/// begins, which is not what a character reaches.
pub fn stage_end_levels(stage: Stage) -> (u32, u32) {
    match stage {
        Stage::Act(1) => (16, 18),
        Stage::Act(2) => (30, 32),
        Stage::Act(3) => (46, 48),
        Stage::Act(_) => (56, 58),
        Stage::Interludes => (63, 65),
        Stage::Endgame => (65, 100),
    }
}

/// The levels a character plays a stage at: from about where the stage
/// before ends (middle of its range) to the top of this stage's end range.
/// Act 1 1–18, Act 2 17–32, Act 3 31–48, Act 4 47–58, Interludes 57–65,
/// maps from 65.
pub fn stage_level_span(stage: Stage) -> (u32, u32) {
    let middle = |s: Stage| {
        let (low, high) = stage_end_levels(s);
        (low + high) / 2
    };
    let start = match stage {
        Stage::Act(1) => 1,
        Stage::Act(n) if n <= 4 => middle(Stage::Act(n - 1)),
        Stage::Act(_) | Stage::Interludes => middle(Stage::Act(4)),
        Stage::Endgame => 65,
    };
    (start, stage_end_levels(stage).1)
}

pub fn stage_for_level(level: u32) -> Stage {
    match level {
        0..=15 => Stage::Act(1),
        16..=32 => Stage::Act(2),
        33..=45 => Stage::Act(3),
        46..=53 => Stage::Act(4),
        54..=64 => Stage::Interludes,
        _ => Stage::Endgame,
    }
}

/// Final stage for a spec: explicit act/interlude/endgame labels win, then a
/// "Level N" label, then the estimated level from points spent.
pub fn resolve_stage(hint: Option<StageHint>, estimated_level: u32) -> Stage {
    match hint {
        Some(StageHint::Act(n)) => Stage::Act(n),
        Some(StageHint::Interludes) => Stage::Interludes,
        Some(StageHint::EarlyMaps | StageHint::Endgame) => Stage::Endgame,
        Some(StageHint::Level(level)) => stage_for_level(level),
        Some(StageHint::Cruel(_) | StageHint::LevelingStep(_) | StageHint::Leveling) | None => {
            stage_for_level(estimated_level)
        }
    }
}

/// One row of PoB's acts table: the highest quest area level reached in that
/// act and the cumulative passive points granted by quests by then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActProgress {
    pub area_level: u32,
    pub quest_points: u32,
}

/// PoB2 `src/Data/QuestRewards.lua` at commit 2450f2f (0.5 data): start,
/// Act 1, Act 2, Act 3, Act 4, Interludes, Epilogue.
pub const POE2_0_5_ACTS: [ActProgress; 7] = [
    ActProgress {
        area_level: 1,
        quest_points: 0,
    },
    ActProgress {
        area_level: 12,
        quest_points: 4,
    },
    ActProgress {
        area_level: 28,
        quest_points: 8,
    },
    ActProgress {
        area_level: 44,
        quest_points: 12,
    },
    ActProgress {
        area_level: 51,
        quest_points: 16,
    },
    ActProgress {
        area_level: 64,
        quest_points: 22,
    },
    ActProgress {
        area_level: 62,
        quest_points: 24,
    },
];

/// Port of PoB2 `buildMode:EstimatePlayerProgress` (src/Modules/Build.lua):
/// the character level at which `points_used` main-tree passives become
/// available. `points_used` excludes class/ascendancy start nodes and
/// ascendancy passives; `weapon_set_points` are the points used in each set.
pub fn estimate_level(acts: &[ActProgress], points_used: u32, extra_points: u32, weapon_set_points: (u32, u32)) -> u32 {
    let shared_weapon_points = i64::from(weapon_set_points.0.min(weapon_set_points.1));
    let mut level = 1;
    for (i, act) in acts.iter().enumerate() {
        level =
            (i64::from(points_used) + 1 - i64::from(act.quest_points) - i64::from(extra_points) - shared_weapon_points)
                .max(i64::from(act.area_level))
                .min(100);
        match acts.get(i + 1) {
            Some(next) if level > i64::from(next.area_level) => continue,
            _ => break,
        }
    }
    level as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_from_real_guides() {
        let c = |t| classify_title(t, "0_5");
        assert_eq!(c("Leveling 1 - Bows"), Some(StageHint::LevelingStep(1)));
        assert_eq!(c("Leveling 4 - Spears Swap"), Some(StageHint::LevelingStep(4)));
        assert_eq!(c("Starting Maps/Random Gear"), Some(StageHint::EarlyMaps));
        assert_eq!(c("Proper Uniques Maligaro+Vipers"), None);
        assert_eq!(c("Level 75 Endgame Setup"), Some(StageHint::Level(75)));
        assert_eq!(c("Main Tree 93"), Some(StageHint::Endgame));
        assert_eq!(c("Act 5 / End of Campaign"), Some(StageHint::Interludes));
        assert_eq!(c("Interludes"), Some(StageHint::Interludes));
    }

    #[test]
    fn pre_0_3_acts_four_to_six_were_cruel() {
        assert_eq!(classify_title("Act 3", "0_2"), Some(StageHint::Act(3)));
        assert_eq!(classify_title("Act 4", "0_2"), Some(StageHint::Cruel(1)));
        assert_eq!(classify_title("Act 6", "0_2"), Some(StageHint::Cruel(3)));
        assert_eq!(classify_title("Endgame w/ Diamonds", "0_2"), Some(StageHint::Endgame));
        assert_eq!(classify_title("Early Maps", "0_2"), Some(StageHint::EarlyMaps));
    }

    #[test]
    fn estimator_matches_pob_on_known_point_counts() {
        // Main-tree points of a real 0.5 Deadeye guide's specs and the levels
        // PoB derives for them.
        let est = |points| estimate_level(&POE2_0_5_ACTS, points, 0, (0, 0));
        assert_eq!(est(17), 14);
        assert_eq!(est(33), 28);
        assert_eq!(est(47), 40);
        assert_eq!(est(59), 48);
        assert_eq!(est(76), 61);
        assert_eq!(est(87), 64);
        assert_eq!(est(0), 1);
        assert_eq!(est(500), 100);
    }

    #[test]
    fn shared_weapon_set_points_reduce_the_level() {
        // 33 + 1 - 4 quest points - 4 shared weapon-set points = 26 (Act 2).
        assert_eq!(estimate_level(&POE2_0_5_ACTS, 33, 0, (4, 6)), 26);
        assert!(estimate_level(&POE2_0_5_ACTS, 50, 0, (4, 6)) < estimate_level(&POE2_0_5_ACTS, 50, 0, (0, 0)));
    }

    #[test]
    fn stage_spans_start_where_the_last_stage_ends() {
        assert_eq!(stage_level_span(Stage::Act(1)), (1, 18));
        assert_eq!(stage_level_span(Stage::Act(2)), (17, 32));
        assert_eq!(stage_level_span(Stage::Act(3)), (31, 48));
        assert_eq!(stage_level_span(Stage::Act(4)), (47, 58));
        assert_eq!(stage_level_span(Stage::Interludes), (57, 65));
        assert_eq!(stage_level_span(Stage::Endgame), (65, 100));
    }

    #[test]
    fn stage_resolution() {
        assert_eq!(resolve_stage(Some(StageHint::Act(2)), 50), Stage::Act(2));
        assert_eq!(resolve_stage(Some(StageHint::LevelingStep(5)), 48), Stage::Act(4));
        assert_eq!(resolve_stage(Some(StageHint::Level(75)), 70), Stage::Endgame);
        assert_eq!(resolve_stage(None, 61), Stage::Interludes);
        assert_eq!(resolve_stage(Some(StageHint::EarlyMaps), 64), Stage::Endgame);
    }
}
