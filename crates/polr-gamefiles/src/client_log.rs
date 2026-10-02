//! Events from the client log (`<install>/logs/Client.txt`).
//!
//! Every pattern below was checked against a real 0.5 log (PLAN.md §3.2).
//! Chat lines are deliberately not parsed here: they carry other players'
//! messages and are handled, if ever, behind an explicit opt-in.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::OnceLock;

use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEvent {
    /// `YYYY/MM/DD HH:MM:SS` in local time, exactly as the client writes it.
    pub timestamp: String,
    pub kind: EventKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    /// `Generating level 15 area "G1_town" with seed 1`. Logged before the
    /// matching [`EventKind::SceneEntered`]; `area_level` is the monster level.
    AreaGenerated {
        area_id: String,
        area_level: u32,
        seed: u64,
    },
    /// `[SCENE] Set Source [Clearfell Encampment]`, placeholders filtered out.
    SceneEntered {
        name: String,
    },
    /// `: Name (Class) is now level 12`. `class` is the ascendancy once chosen.
    LevelUp {
        character: String,
        class: String,
        level: u32,
    },
    /// `: Name has been slain.` Also logged for party members.
    Slain {
        character: String,
    },
    /// Ids match the in-game Build Planner's passive ids.
    PassiveAllocated {
        id: String,
        name: String,
    },
    PassiveUnallocated {
        id: String,
        name: String,
    },
    /// Quest rewards: `You have received 2 Passive Skill Points.` and
    /// `You have received 2 Weapon Set Passive Skill Points.`
    PassivePointsReceived {
        count: u32,
        weapon_set: bool,
    },
    /// Permanent campaign rewards: `: Name has received +10% to
    /// [Resistances|Cold Resistance].` — `lost` when a choice is swapped
    /// (`has lost ...`). `bonus` keeps the game's `[Tag|Text]` markup; see
    /// [`plain_text`].
    PermanentBonus {
        character: String,
        bonus: String,
        lost: bool,
    },
    /// `[BuildPlanner] Successfully loaded build 'file:C:/.../X.build'`
    BuildPlannerLoaded {
        path: String,
    },
    PlayerJoinedArea {
        character: String,
    },
    PlayerLeftArea {
        character: String,
    },
    TradeAccepted,
    AfkMode {
        on: bool,
    },
}

struct Patterns {
    line: Regex,
    area: Regex,
    scene: Regex,
    level_up: Regex,
    slain: Regex,
    allocated: Regex,
    unallocated: Regex,
    points: Regex,
    bonus: Regex,
    planner: Regex,
    joined: Regex,
    left: Regex,
    afk: Regex,
}

fn patterns() -> &'static Patterns {
    static PATTERNS: OnceLock<Patterns> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        let re = |s: &str| Regex::new(s).expect("static regex");
        Patterns {
            line: re(r"^(\d{4}/\d{2}/\d{2} \d{2}:\d{2}:\d{2}) \d+ \S+ \[\w+ Client \d+\] (.*)$"),
            area: re(r#"^Generating level (\d+) area "([^"]+)" with seed (\d+)$"#),
            scene: re(r"^\[SCENE\] Set Source \[(.+)\]$"),
            level_up: re(r"^: (\S+) \(([^)]+)\) is now level (\d+)$"),
            slain: re(r"^: (\S+) has been slain\.$"),
            allocated: re(r"^Successfully allocated passive skill id: ([^,]+), name: (.*)$"),
            unallocated: re(r"^Successfully unallocated passive skill id: ([^,]+), name: (.*)$"),
            points: re(r"^: You have received (\d+) (Weapon Set )?Passive Skill Points\.$"),
            bonus: re(r"^: (\S+) has (received|lost) (.+)\.$"),
            planner: re(r"^\[BuildPlanner\] Successfully loaded build '(.*)'$"),
            joined: re(r"^: (\S+) has joined the area\.$"),
            left: re(r"^: (\S+) has left the area\.$"),
            afk: re(r"^: AFK mode is now (ON|OFF)\."),
        }
    })
}

/// Parses one log line; `None` for lines that aren't a known event.
pub fn parse_line(line: &str) -> Option<LogEvent> {
    let p = patterns();
    let caps = p.line.captures(line.trim_end())?;
    let timestamp = caps[1].to_owned();
    let kind = parse_body(p, caps[2].trim_end())?;
    Some(LogEvent { timestamp, kind })
}

fn parse_body(p: &Patterns, body: &str) -> Option<EventKind> {
    if let Some(c) = p.area.captures(body) {
        return Some(EventKind::AreaGenerated {
            area_level: c[1].parse().ok()?,
            area_id: c[2].to_owned(),
            seed: c[3].parse().ok()?,
        });
    }
    if let Some(c) = p.scene.captures(body) {
        let name = &c[1];
        if name == "(unknown)" || name == "(null)" {
            return None;
        }
        return Some(EventKind::SceneEntered { name: name.to_owned() });
    }
    if let Some(c) = p.level_up.captures(body) {
        return Some(EventKind::LevelUp {
            character: c[1].to_owned(),
            class: c[2].to_owned(),
            level: c[3].parse().ok()?,
        });
    }
    if let Some(c) = p.slain.captures(body) {
        return Some(EventKind::Slain {
            character: c[1].to_owned(),
        });
    }
    if let Some(c) = p.allocated.captures(body) {
        return Some(EventKind::PassiveAllocated {
            id: c[1].to_owned(),
            name: c[2].to_owned(),
        });
    }
    if let Some(c) = p.unallocated.captures(body) {
        return Some(EventKind::PassiveUnallocated {
            id: c[1].to_owned(),
            name: c[2].to_owned(),
        });
    }
    if let Some(c) = p.points.captures(body) {
        return Some(EventKind::PassivePointsReceived {
            count: c[1].parse().ok()?,
            weapon_set: c.get(2).is_some(),
        });
    }
    if let Some(c) = p.bonus.captures(body) {
        return Some(EventKind::PermanentBonus {
            character: c[1].to_owned(),
            lost: &c[2] == "lost",
            bonus: c[3].to_owned(),
        });
    }
    if let Some(c) = p.planner.captures(body) {
        return Some(EventKind::BuildPlannerLoaded { path: c[1].to_owned() });
    }
    if let Some(c) = p.joined.captures(body) {
        return Some(EventKind::PlayerJoinedArea {
            character: c[1].to_owned(),
        });
    }
    if let Some(c) = p.left.captures(body) {
        return Some(EventKind::PlayerLeftArea {
            character: c[1].to_owned(),
        });
    }
    if body == ": Trade accepted." {
        return Some(EventKind::TradeAccepted);
    }
    if let Some(c) = p.afk.captures(body) {
        return Some(EventKind::AfkMode { on: &c[1] == "ON" });
    }
    None
}

/// Resolves the client's link markup: `[Resistances|Cold Resistance]` →
/// `Cold Resistance`, `[Charm]` → `Charm`.
pub fn plain_text(text: &str) -> String {
    static LINKS: OnceLock<Regex> = OnceLock::new();
    LINKS
        .get_or_init(|| Regex::new(r"\[(?:[^\]|]*\|)?([^\]]*)\]").expect("static regex"))
        .replace_all(text, "$1")
        .into_owned()
}

/// Campaign act for a campaign area id (`G1_4`, `G2_4_1`, `G3_town`).
/// `None` for hideouts, endgame maps and anything else; interlude ids are not
/// yet verified (PLAN.md §12).
pub fn campaign_act(area_id: &str) -> Option<u8> {
    let rest = area_id.strip_prefix('G')?;
    let (act, _) = rest.split_once('_')?;
    act.parse().ok()
}

/// Incrementally reads complete lines appended to the log. Polling-based so it
/// survives the client truncating or recreating the file.
pub struct LogTailer {
    path: PathBuf,
    offset: u64,
    partial: Vec<u8>,
}

impl LogTailer {
    /// Starts at the current end of the file: only new lines are returned.
    pub fn from_end(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let offset = std::fs::metadata(&path)?.len();
        Ok(Self {
            path,
            offset,
            partial: Vec::new(),
        })
    }

    /// Starts at the beginning, e.g. to backfill a character's history.
    pub fn from_start(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            offset: 0,
            partial: Vec::new(),
        }
    }

    pub fn poll(&mut self) -> io::Result<Vec<String>> {
        let mut file = File::open(&self.path)?;
        let len = file.metadata()?.len();
        if len < self.offset {
            self.offset = 0;
            self.partial.clear();
        }
        if len == self.offset {
            return Ok(Vec::new());
        }
        file.seek(SeekFrom::Start(self.offset))?;
        let mut buf = Vec::new();
        file.by_ref().take(len - self.offset).read_to_end(&mut buf)?;
        self.offset += buf.len() as u64;
        self.partial.extend_from_slice(&buf);

        // Split in one pass and drop the consumed bytes once at the end:
        // draining line by line from the front is quadratic and took minutes
        // on a 13 MB log.
        let mut lines = Vec::new();
        let mut start = 0;
        while let Some(rel) = self.partial[start..].iter().position(|&b| b == b'\n') {
            let end = start + rel;
            let text = String::from_utf8_lossy(&self.partial[start..end]);
            let text = text.trim_end_matches(['\r', '\n']);
            if !text.is_empty() {
                lines.push(text.to_owned());
            }
            start = end + 1;
        }
        self.partial.drain(..start);
        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(line: &str) -> Option<EventKind> {
        parse_line(line).map(|e| e.kind)
    }

    #[test]
    fn parses_verified_line_formats() {
        assert_eq!(
            kind(
                r#"2026/09/11 19:49:40 93507890 2caa229f [DEBUG Client 57044] Generating level 1 area "G1_1" with seed 2905543419"#
            ),
            Some(EventKind::AreaGenerated {
                area_id: "G1_1".into(),
                area_level: 1,
                seed: 2905543419
            })
        );
        assert_eq!(
            kind("2026/09/11 19:52:59 93706281 3ef23348 [INFO Client 57044] : ExileOne (Monk) is now level 2"),
            Some(EventKind::LevelUp {
                character: "ExileOne".into(),
                class: "Monk".into(),
                level: 2
            })
        );
        assert_eq!(
            kind("2026/09/11 21:05:22 98049828 3ef23348 [INFO Client 57044] : ExileOne has been slain."),
            Some(EventKind::Slain {
                character: "ExileOne".into()
            })
        );
        assert_eq!(
            kind("2026/09/16 01:51:23 460797000 f4ab5ade [INFO Client 38036] Successfully allocated passive skill id: AscendancyWarrior3Notable4, name: Coal Stoker"),
            Some(EventKind::PassiveAllocated { id: "AscendancyWarrior3Notable4".into(), name: "Coal Stoker".into() })
        );
        assert_eq!(
            kind("2026/09/11 21:14:09 98576781 f4ab5b39 [INFO Client 4176] Successfully unallocated passive skill id: energy_shield15, name: Energy Shield"),
            Some(EventKind::PassiveUnallocated { id: "energy_shield15".into(), name: "Energy Shield".into() })
        );
        assert_eq!(
            kind("2026/09/11 23:50:54 107981781 3ef23348 [INFO Client 4176] : You have received 2 Weapon Set Passive Skill Points."),
            Some(EventKind::PassivePointsReceived { count: 2, weapon_set: true })
        );
        assert_eq!(
            kind(
                "2026/09/11 23:50:54 107981781 3ef23348 [INFO Client 4176] : You have received 2 Passive Skill Points."
            ),
            Some(EventKind::PassivePointsReceived {
                count: 2,
                weapon_set: false
            })
        );
        assert_eq!(
            kind("2026/09/22 10:00:00 1 2 [INFO Client 1] [SCENE] Set Source [Clearfell Encampment]"),
            Some(EventKind::SceneEntered {
                name: "Clearfell Encampment".into()
            })
        );
        assert_eq!(
            kind("2026/09/22 10:00:00 1 2 [INFO Client 1] [SCENE] Set Source [(unknown)]"),
            None
        );
        assert_eq!(
            kind("2026/09/11 22:40:06 103732953 3ef23348 [INFO Client 4176] : AFK mode is now ON. Autoreply \"This player is AFK.\""),
            Some(EventKind::AfkMode { on: true })
        );
        assert_eq!(
            kind("2026/09/15 01:18:16 372412718 3ef23348 [INFO Client 38976] : Trade accepted."),
            Some(EventKind::TradeAccepted)
        );
    }

    #[test]
    fn permanent_bonuses() {
        assert_eq!(
            kind("2026/09/25 22:47:48 766661218 3ef23348 [INFO Client 17048] : ExileOne has received 30% increased [Charm] Charges gained."),
            Some(EventKind::PermanentBonus {
                character: "ExileOne".into(),
                bonus: "30% increased [Charm] Charges gained".into(),
                lost: false
            })
        );
        assert_eq!(
            kind("2026/09/25 22:47:49 766661953 3ef23348 [INFO Client 17048] : ExileOne has lost 30% increased [Charm] Charges gained."),
            Some(EventKind::PermanentBonus {
                character: "ExileOne".into(),
                bonus: "30% increased [Charm] Charges gained".into(),
                lost: true
            })
        );
        assert_eq!(
            plain_text("+10% to [Resistances|Cold Resistance]"),
            "+10% to Cold Resistance"
        );
        assert_eq!(
            plain_text("30% increased [Charm] Charges gained"),
            "30% increased Charm Charges gained"
        );
    }

    #[test]
    fn ignores_noise() {
        assert_eq!(kind("2026/09/11 19:46:01 ***** LOG FILE OPENING *****"), None);
        assert_eq!(
            kind("2026/09/11 20:00:00 1 2 [WARN Client 1] [TEXTURE] Insufficient VRAM"),
            None
        );
    }

    #[test]
    fn acts_from_area_ids() {
        assert_eq!(campaign_act("G1_13_2"), Some(1));
        assert_eq!(campaign_act("G4_town"), Some(4));
        assert_eq!(campaign_act("HideoutShrine"), None);
        assert_eq!(campaign_act("Abyss_Hub"), None);
    }

    #[test]
    fn tailer_returns_only_complete_new_lines() {
        use std::io::Write;
        let path = std::env::temp_dir().join(format!("polr-tail-{}.txt", std::process::id()));
        std::fs::write(&path, "old line\n").unwrap();
        let mut t = LogTailer::from_end(&path).unwrap();
        assert!(t.poll().unwrap().is_empty());

        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        write!(f, "first\r\nsecond (partial").unwrap();
        assert_eq!(t.poll().unwrap(), ["first"]);
        writeln!(f, ")").unwrap();
        assert_eq!(t.poll().unwrap(), ["second (partial)"]);

        std::fs::write(&path, "after truncate\n").unwrap();
        assert_eq!(t.poll().unwrap(), ["after truncate"]);
        std::fs::remove_file(&path).ok();
    }
}
