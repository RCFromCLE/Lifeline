//! The two AI jobs: one-shot runs where a specialist is the main session
//! (`--agent <name>`), nothing is saved to session history, and the answer
//! must match a schema (`--json-schema`). The player starts each with a
//! button; neither sends anything to the game.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::agents::{BUILD_ARCHITECT, SKILL_COACH};
use crate::{ClaudeCli, RunResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    /// Skills, supports, buttons and rotations for the current stage.
    Skills,
    /// Design the whole build from a catalog archetype the player picked.
    DesignBuild,
}

impl Job {
    pub fn agent(self) -> &'static str {
        match self {
            Job::Skills => SKILL_COACH,
            Job::DesignBuild => BUILD_ARCHITECT,
        }
    }

    fn instruction(self) -> &'static str {
        match self {
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

    /// The prompt for this job; `context_json` carries the details (the
    /// character, the archetype, preferences). Everything else comes from tools.
    pub fn prompt(self, context_json: &str) -> String {
        format!(
            "{}\n\nContext (JSON):\n{context_json}\n\nAnswer with the report object only.",
            self.instruction()
        )
    }
}

/// Report shape of [`Job::DesignBuild`].
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
    /// This CLI configured for a job: the job's agent runs as the main
    /// session, the session isn't persisted, and its schema is enforced.
    pub fn for_job(&self, job: Job) -> ClaudeCli {
        let mut cli = self.clone();
        cli.main_agent = Some(job.agent().to_owned());
        cli.persist_session = false;
        cli.json_schema = Some(
            match job {
                Job::Skills => skills_schema(),
                Job::DesignBuild => report_schema(),
            }
            .to_string(),
        );
        cli
    }
}

/// Reads the report from a design job's result: the validated structured
/// output if the CLI provides one, otherwise the JSON object in the text.
pub fn parse_report(result: &RunResult) -> Option<JobReport> {
    structured(result).and_then(|v| serde_json::from_value(v).ok())
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
        let cli = ClaudeCli::new(std::env::temp_dir()).for_job(Job::DesignBuild);
        let a = args(&cli);
        assert!(a.windows(2).any(|w| w == ["--agent", "build-architect"]));
        assert!(!a.iter().any(|x| x == "--system-prompt"));
        assert!(a.iter().any(|x| x == "--no-session-persistence"));
        assert!(a
            .windows(2)
            .any(|w| w[0] == "--json-schema" && w[1].contains("\"findings\"")));
        let skills = args(&ClaudeCli::new(std::env::temp_dir()).for_job(Job::Skills));
        assert!(skills.windows(2).any(|w| w[0] == "--json-schema" && w[1].contains("\"rotations\"")));
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
        for job in [Job::Skills, Job::DesignBuild] {
            let agent = roster.iter().find(|a| a.name == job.agent()).unwrap();
            assert!(agent.tools.iter().any(|t| t == crate::agents::STRUCTURED_OUTPUT), "{}", agent.name);
        }
    }

    #[test]
    fn prompts_carry_context() {
        let p = Job::Skills.prompt(r#"{"class":"Sorceress","level":7}"#);
        assert!(p.contains("Sorceress") && p.ends_with("report object only."));
    }
}
