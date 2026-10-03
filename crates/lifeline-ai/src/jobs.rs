//! Background agent jobs (PLAN.md §6.8): one-shot runs where a specialist is
//! the main session (`--agent <name>`), nothing is saved to session history,
//! and the answer must match a report schema (`--json-schema`). The app
//! triggers them from game events; none of them sends anything to the game.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::agents::{BUILD_ARCHITECT, BUILD_AUDITOR, BUILD_RATER, HC_SAFETY_OFFICER, PATCH_ANALYST, ROUTE_COACH, SKILL_COACH};
use crate::{ClaudeCli, RunResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    /// The client log reports a game version newer than our data.
    PatchWatch,
    /// A build was imported or designed.
    BuildAudit,
    /// The tracked character was slain.
    DeathDebrief,
    /// The game closed or the player went AFK after a session.
    SessionRecap,
    /// Grade the build F–S+ for the current stage (rating screen, overlay).
    Rating,
    /// Skills, supports, buttons and rotations for the current stage.
    Skills,
    /// Design the whole build from a catalog archetype the player picked.
    DesignBuild,
}

impl Job {
    pub fn agent(self) -> &'static str {
        match self {
            Job::PatchWatch => PATCH_ANALYST,
            Job::BuildAudit => BUILD_AUDITOR,
            Job::DeathDebrief => HC_SAFETY_OFFICER,
            Job::SessionRecap => ROUTE_COACH,
            Job::Rating => BUILD_RATER,
            Job::Skills => SKILL_COACH,
            Job::DesignBuild => BUILD_ARCHITECT,
        }
    }

    fn instruction(self) -> &'static str {
        match self {
            Job::PatchWatch => {
                "A new game version was detected (see context). Report the changes that affect this \
                 player's build, remaining campaign stages and hardcore safety."
            }
            Job::BuildAudit => {
                "Audit the player's current build plan stage by stage and report problems, most \
                 dangerous first."
            }
            Job::DeathDebrief => {
                "The character was just slain (see context). Reconstruct the most likely cause and \
                 give one or two concrete changes. Say how confident you are."
            }
            Job::SessionRecap => {
                "The play session ended (see context). Summarise progress against the plan and list \
                 the first steps for next session."
            }
            Job::Rating => "Rate the character's build for where it is right now in the campaign or endgame.",
            Job::Skills => "Set up this character's skills, supports, buttons and rotations for where it is right now.",
            Job::DesignBuild => {
                "Design this player's whole build, Act 1 to Endgame, from the archetype they picked and their \
                 preferences (context). Follow the archetype's skills, ascendancy order and key passives; fill \
                 in every stage. Save it with design_build until the report is clean. Then return the report \
                 object: summary = the build in two short sentences; findings = the hardcore risks and what \
                 the player should watch, most dangerous first."
            }
        }
    }

    /// The prompt for this job; `context_json` carries event details (version
    /// strings, death record, session span). Everything else comes from tools.
    pub fn prompt(self, context_json: &str) -> String {
        format!(
            "{}\n\nContext (JSON):\n{context_json}\n\nAnswer with the report object only.",
            self.instruction()
        )
    }
}

/// Report shape every job returns.
pub fn report_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "summary": { "type": "string" },
            "findings": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "severity": { "type": "string", "enum": ["danger", "warning", "info"] },
                        "stage": { "type": "string" },
                        "title": { "type": "string" },
                        "detail": { "type": "string" },
                        "source": { "type": "string" }
                    },
                    "required": ["severity", "title", "detail"]
                }
            }
        },
        "required": ["summary", "findings"]
    })
}

pub const GRADES: &[&str] = &[
    "F", "F+", "D-", "D", "D+", "C-", "C", "C+", "B-", "B", "B+", "A-", "A", "A+", "S", "S+",
];

/// Lowers `rating` to at most `max` (a [`GRADES`] entry), noting why. Used
/// for hard limits the rater may not talk its way past (e.g. unknown gear).
pub fn cap_grade(rating: &mut Rating, max: &str, reason: &str) {
    let rank = |g: &str| GRADES.iter().position(|x| *x == g);
    let (Some(cur), Some(cap)) = (rank(&rating.grade), rank(max)) else {
        return;
    };
    if cur > cap {
        rating.grade = max.to_owned();
        // Keep the score in step: grade bands are 100 / 16 wide.
        rating.score = rating.score.min(((cap + 1) * 100 / GRADES.len()) as f64);
        rating.explanation = format!("Capped at {max}: {reason} {}", rating.explanation);
    }
}

/// Schema for [`Job::Rating`].
pub fn rating_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "grade": {"type": "string", "enum": GRADES},
            "score": {"type": "number"},
            "summary": {"type": "string"},
            "explanation": {"type": "string"},
            "categories": {"type": "array", "items": {"type": "object", "properties": {
                "name": {"type": "string"}, "grade": {"type": "string", "enum": GRADES}, "note": {"type": "string"}
            }, "required": ["name", "grade", "note"]}},
            "pieces": {"type": "array", "items": {"type": "object", "properties": {
                "group": {"type": "string", "enum": PIECE_GROUPS},
                "name": {"type": "string"},
                "grade": {"type": "string", "enum": GRADES},
                "have": {"type": "string"},
                "note": {"type": "string"},
                "verified": {"type": "boolean"}
            }, "required": ["group", "name", "grade", "note", "verified"]}},
            "recommendations": {"type": "array", "items": {"type": "object", "properties": {
                "slot": {"type": "string"}, "title": {"type": "string"}, "why": {"type": "string"}, "look_for": {"type": "string"}
            }, "required": ["slot", "title", "why", "look_for"]}}
        },
        "required": ["grade", "score", "summary", "explanation", "categories", "pieces", "recommendations"]
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rating {
    pub grade: String,
    pub score: f64,
    pub summary: String,
    pub explanation: String,
    pub categories: Vec<RatingCategory>,
    /// Every graded piece: each gear slot, skill, defence, passives, flasks.
    #[serde(default)]
    pub pieces: Vec<RatingPiece>,
    pub recommendations: Vec<Recommendation>,
}

/// Groups the rating screen shows pieces under, in order.
pub const PIECE_GROUPS: [&str; 5] = ["Gear", "Skills", "Defences", "Passives", "Flasks & charms"];

/// Gear slots every rating grades, recorded or not.
pub const GEAR_SLOTS: [&str; 10] =
    ["Weapon", "Off-hand", "Helmet", "Body Armour", "Gloves", "Boots", "Amulet", "Ring 1", "Ring 2", "Belt"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RatingPiece {
    pub group: String,
    pub name: String,
    pub grade: String,
    /// What the character has there (item name, skill and supports, value).
    #[serde(default)]
    pub have: String,
    pub note: String,
    /// False when the game doesn't show it (unrecorded slot, gems).
    #[serde(default)]
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RatingCategory {
    pub name: String,
    pub grade: String,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recommendation {
    pub slot: String,
    pub title: String,
    pub why: String,
    pub look_for: String,
}

/// Schema for [`Job::Skills`].
pub fn skills_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "summary": {"type": "string"},
            "skills": {"type": "array", "items": {"type": "object", "properties": {
                "name": {"type": "string"}, "role": {"type": "string"}, "button": {"type": "string"}, "notes": {"type": "string"},
                "supports": {"type": "array", "items": {"type": "object", "properties": {
                    "name": {"type": "string"}, "why": {"type": "string"}
                }, "required": ["name", "why"]}}
            }, "required": ["name", "role", "button", "supports"]}},
            "rotations": {"type": "array", "items": {"type": "object", "properties": {
                "situation": {"type": "string"}, "steps": {"type": "array", "items": {"type": "string"}}
            }, "required": ["situation", "steps"]}}
        },
        "required": ["summary", "skills", "rotations"]
    })
}

/// The structured object from a job's result (validated output, else the
/// JSON in the result text).
pub fn structured(result: &RunResult) -> Option<Value> {
    if let Some(v) = result.structured_output.clone() {
        return Some(v);
    }
    let text = result.result.as_deref()?;
    let (start, end) = (text.find('{')?, text.rfind('}')?);
    serde_json::from_str(text.get(start..=end)?).ok()
}

/// Reads a [`Rating`] from a rating job's result.
pub fn parse_rating(result: &RunResult) -> Option<Rating> {
    if let Some(r) = result
        .structured_output
        .clone()
        .and_then(|v| serde_json::from_value(v).ok())
    {
        return Some(r);
    }
    let text = result.result.as_deref()?;
    let (start, end) = (text.find('{')?, text.rfind('}')?);
    serde_json::from_str(text.get(start..=end)?).ok()
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct JobReport {
    pub summary: String,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    #[serde(default)]
    pub stage: Option<String>,
    pub title: String,
    pub detail: String,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Danger,
    Warning,
    Info,
}

impl ClaudeCli {
    /// This CLI configured for a background job: the job's agent runs as the
    /// main session, the session isn't persisted, and the report schema is
    /// enforced.
    pub fn for_job(&self, job: Job) -> ClaudeCli {
        let mut cli = self.clone();
        cli.main_agent = Some(job.agent().to_owned());
        cli.persist_session = false;
        cli.json_schema = Some(
            match job {
                Job::Rating => rating_schema(),
                Job::Skills => skills_schema(),
                _ => report_schema(),
            }
            .to_string(),
        );
        cli
    }
}

/// Reads the report from a job's result: the validated structured output if
/// the CLI provides one, otherwise the JSON object in the result text.
pub fn parse_report(result: &RunResult) -> Option<JobReport> {
    if let Some(report) = result
        .structured_output
        .clone()
        .and_then(|v| serde_json::from_value(v).ok())
    {
        return Some(report);
    }
    let text = result.result.as_deref()?;
    let (start, end) = (text.find('{')?, text.rfind('}')?);
    serde_json::from_str(text.get(start..=end)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(cli: &ClaudeCli) -> Vec<String> {
        cli.command(None)
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn job_runs_its_agent_as_main_session() {
        let cli = ClaudeCli::new(std::env::temp_dir()).for_job(Job::DeathDebrief);
        let a = args(&cli);
        assert!(a.windows(2).any(|w| w == ["--agent", "hc-safety-officer"]));
        assert!(!a.iter().any(|x| x == "--system-prompt"));
        assert!(a.iter().any(|x| x == "--no-session-persistence"));
        assert!(a
            .windows(2)
            .any(|w| w[0] == "--json-schema" && w[1].contains("\"findings\"")));
    }

    fn result(text: Option<&str>, structured: Option<Value>) -> RunResult {
        RunResult {
            subtype: "success".into(),
            is_error: false,
            result: text.map(str::to_owned),
            session_id: None,
            total_cost_usd: None,
            usage: None,
            structured_output: structured,
        }
    }

    #[test]
    fn report_from_structured_output_or_text() {
        let report = json!({"summary": "Fire res short for Act 3", "findings": [
            {"severity": "danger", "stage": "Act 3", "title": "Fire resistance", "detail": "61% after the -20% penalty"}
        ]});
        let parsed = parse_report(&result(None, Some(report.clone()))).unwrap();
        assert_eq!(parsed.findings[0].severity, Severity::Danger);

        let text = format!("Here is the report:\n{report}\n");
        assert_eq!(parse_report(&result(Some(&text), None)), Some(parsed));
        assert_eq!(parse_report(&result(Some("no json here"), None)), None);
    }

    #[test]
    fn every_job_agent_can_return_structured_output() {
        let roster = crate::agents::roster();
        for job in [
            Job::PatchWatch,
            Job::BuildAudit,
            Job::DeathDebrief,
            Job::SessionRecap,
            Job::Rating,
        ] {
            let agent = roster.iter().find(|a| a.name == job.agent()).unwrap();
            assert!(
                agent.tools.iter().any(|t| t == crate::agents::STRUCTURED_OUTPUT),
                "{}",
                agent.name
            );
        }
    }

    #[test]
    fn prompts_carry_context() {
        let p = Job::PatchWatch.prompt(r#"{"from":"4.5.5.2","to":"4.5.6.0"}"#);
        assert!(p.contains("4.5.6.0") && p.ends_with("report object only."));
    }

    #[test]
    fn caps_only_lower_grades() {
        let mut r = Rating {
            grade: "B".into(),
            score: 70.0,
            summary: String::new(),
            explanation: "Looks fine.".into(),
            categories: vec![],
            pieces: vec![],
            recommendations: vec![],
        };
        cap_grade(&mut r, "D+", "no gear recorded.");
        assert_eq!(r.grade, "D+");
        assert!(r.score <= 31.0 && r.explanation.starts_with("Capped at D+"));
        cap_grade(&mut r, "C", "irrelevant");
        assert_eq!(r.grade, "D+", "a higher cap never raises a grade");
    }
}