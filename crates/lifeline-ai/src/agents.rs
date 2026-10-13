//! Lifeline's specialist agents, handed to Claude Code as `--agents <file>`:
//! the build architect and the skill coach (each runs one job), and the two
//! checkers the architect can consult. Field names follow the Claude Code
//! subagent reference: `description`, `prompt`, `tools`, `model`, `effort`,
//! `maxTurns`, `omitClaudeMd`.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::mcp_tool_name;
use crate::tools::*;

pub const BUILD_ARCHITECT: &str = "build-architect";
pub const SKILL_COACH: &str = "skill-coach";
pub const HC_SAFETY_OFFICER: &str = "hc-safety-officer";
pub const FACT_CHECKER: &str = "fact-checker";

pub const AGENTS_FILE: &str = "agents.json";

/// Every agent runs on Claude Opus 5.5 (owner's decision, 2026-10-01). Pinned
/// by full id rather than the `opus` alias so a future Opus release doesn't
/// change behaviour silently.
pub const MODEL: &str = "claude-opus-5-5";

const SHARED: &str = include_str!("../prompts/agents/shared.md");
const WEB: &[&str] = &["WebSearch", "WebFetch"];

/// Built-in tool that produces `--json-schema` output. An agent that runs a
/// job as the main session must list it, or no structured output is produced
/// (seen live with Claude Code 2.1.286).
pub const STRUCTURED_OUTPUT: &str = "StructuredOutput";

/// Claude Code's own agents, as listed in the `init` event of 2.1.286. They
/// are denied so a job only delegates to Lifeline's checkers.
pub const BUILT_IN_AGENTS: &[&str] = &[
    "claude",
    "claude-code-guide",
    "Explore",
    "general-purpose",
    "Plan",
    "statusline-setup",
];

#[derive(Debug, Clone, PartialEq)]
pub struct AgentSpec {
    pub name: &'static str,
    /// What Claude reads to decide when to delegate to this agent.
    pub description: &'static str,
    role: &'static str,
    /// MCP tools as `mcp__lifeline__…` plus any built-in tools.
    pub tools: Vec<String>,
    /// Pinned model id; see [`MODEL`].
    pub model: &'static str,
    pub effort: Option<&'static str>,
    pub max_turns: Option<u32>,
}

impl AgentSpec {
    /// Shared grounding/hardcore rules followed by the role's own prompt.
    pub fn prompt(&self) -> String {
        format!("{SHARED}\n{}", self.role)
    }

    fn to_json(&self) -> Value {
        let mut spec = json!({
            "description": self.description,
            "prompt": self.prompt(),
            "tools": self.tools,
            "model": self.model,
            "omitClaudeMd": true,
        });
        if let Some(effort) = self.effort {
            spec["effort"] = json!(effort);
        }
        if let Some(turns) = self.max_turns {
            spec["maxTurns"] = json!(turns);
        }
        spec
    }
}

fn tool_list(mcp_groups: &[&[&str]], builtin: &[&str]) -> Vec<String> {
    mcp_groups
        .iter()
        .flat_map(|group| group.iter())
        .map(|name| mcp_tool_name(name))
        .chain(builtin.iter().map(|name| (*name).to_owned()))
        .collect()
}

pub fn roster() -> Vec<AgentSpec> {
    vec![
        AgentSpec {
            name: BUILD_ARCHITECT,
            description: "Designs a whole build (Act 1 to Endgame: passives, ascendancy, skills and \
                          supports, gear goals, hardcore safety) from the archetype the player picked \
                          and saves it with design_build, which checks paths, points and gems.",
            role: include_str!("../prompts/agents/build-architect.md"),
            tools: tool_list(
                &[GAME_DATA, &[BUILD_PLAN, CHARACTER_STATE, DESIGN_BUILD]],
                &["WebSearch", "WebFetch", "Agent(fact-checker, hc-safety-officer)", STRUCTURED_OUTPUT],
            ),
            model: MODEL,
            effort: Some("high"),
            max_turns: Some(120),
        },
        AgentSpec {
            name: SKILL_COACH,
            description: "Sets up skills for the current stage: which skills, which support gems (only \
                          ones the game allows on each skill), which button, and rotations for clearing, \
                          bossing and emergencies.",
            role: include_str!("../prompts/agents/skill-coach.md"),
            tools: tool_list(
                &[&[CHARACTER_STATE, BUILD_PLAN, LOOKUP_GEM, LOOKUP_SUPPORTS_FOR, LOOKUP_PASSIVE]],
                &[STRUCTURED_OUTPUT],
            ),
            model: MODEL,
            effort: Some("high"),
            max_turns: Some(40),
        },
        AgentSpec {
            name: HC_SAFETY_OFFICER,
            description: "Reviews a stage of a build plan for what is most likely to kill a hardcore \
                          character there: resistance gaps, dangerous bosses and areas, ailments and \
                          recovery. Use to review the riskiest stage of a design.",
            role: include_str!("../prompts/agents/hc-safety-officer.md"),
            tools: tool_list(&[GAME_DATA, &[BUILD_PLAN]], WEB),
            model: MODEL,
            effort: Some("high"),
            max_turns: Some(25),
        },
        AgentSpec {
            name: FACT_CHECKER,
            description: "Adversarially verifies Path of Exile 2 claims against current game data, the \
                          wiki and patch notes, returning CONFIRMED / CONTRADICTED / UNVERIFIED per \
                          claim. Use before relying on any mechanic the tools didn't state.",
            role: include_str!("../prompts/agents/fact-checker.md"),
            tools: tool_list(&[GAME_DATA], WEB),
            model: MODEL,
            effort: Some("high"),
            max_turns: Some(20),
        },
    ]
}

/// The `--agents` JSON object: `{"<name>": {description, prompt, ...}}`.
pub fn agents_json(agents: &[AgentSpec]) -> String {
    let map: Map<String, Value> = agents.iter().map(|a| (a.name.to_owned(), a.to_json())).collect();
    Value::Object(map).to_string()
}

/// Writes the roster to `dir/agents.json`. With `--print`, `--agents` takes
/// this path, which keeps long prompts off the command line (Windows caps a
/// command line at 32,767 characters).
pub fn write_agents_file(dir: &Path) -> io::Result<PathBuf> {
    let path = dir.join(AGENTS_FILE);
    std::fs::write(&path, agents_json(&roster()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed() -> Value {
        serde_json::from_str(&agents_json(&roster())).unwrap()
    }

    #[test]
    fn every_agent_has_the_documented_fields() {
        let v = parsed();
        let agents = v.as_object().unwrap();
        assert_eq!(agents.len(), 4);
        for (name, spec) in agents {
            for field in ["description", "prompt", "tools", "model", "omitClaudeMd"] {
                assert!(spec.get(field).is_some(), "{name} lacks {field}");
            }
            assert_eq!(spec["model"], MODEL, "{name}");
            assert!(spec["prompt"].as_str().unwrap().starts_with("You are a specialist"), "{name}");
        }
    }

    #[test]
    fn granted_mcp_tools_exist_on_the_server() {
        for agent in roster() {
            for tool in agent.tools.iter().filter_map(|t| t.strip_prefix("mcp__lifeline__")) {
                assert!(ALL.contains(&tool), "{} is granted unknown tool {tool}", agent.name);
            }
        }
    }

    #[test]
    fn only_the_architect_delegates_and_only_it_can_save_a_build() {
        for agent in roster() {
            let delegates = agent.tools.iter().any(|t| t.starts_with("Agent"));
            assert_eq!(delegates, agent.name == BUILD_ARCHITECT, "{}", agent.name);
            let saves = agent.tools.contains(&mcp_tool_name(DESIGN_BUILD));
            assert_eq!(saves, agent.name == BUILD_ARCHITECT, "{}", agent.name);
        }
    }

    #[test]
    fn prompts_do_not_name_removed_tools() {
        for agent in roster() {
            let p = agent.prompt();
            for gone in ["propose_action", "trade_search", "equipped_items", "death_journal", "campaign_rewards", "rate_listings"] {
                assert!(!p.contains(gone), "{} still mentions {gone}", agent.name);
            }
        }
    }

    #[test]
    fn writes_agents_file() {
        let dir = std::env::temp_dir().join(format!("lifeline-agents-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_agents_file(&dir).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"build-architect\"") && text.contains("\"skill-coach\""));
        std::fs::remove_dir_all(&dir).ok();
    }
}
