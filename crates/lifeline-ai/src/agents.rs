//! The companion's specialist agents (PLAN.md §6.8), handed to Claude Code as
//! `--agents <file>`. Field names follow the Claude Code subagent reference:
//! `description`, `prompt`, `tools`, `model`, `effort`, `maxTurns`,
//! `omitClaudeMd`. The companion launches them with the `Agent` tool.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::mcp_tool_name;
use crate::tools::*;

pub const BUILD_ARCHITECT: &str = "build-architect";
pub const BUILD_AUDITOR: &str = "build-auditor";
pub const GEAR_APPRAISER: &str = "gear-appraiser";
pub const MARKET_SCOUT: &str = "market-scout";
pub const HC_SAFETY_OFFICER: &str = "hc-safety-officer";
pub const ROUTE_COACH: &str = "route-coach";
pub const LOOT_FILTER_SMITH: &str = "loot-filter-smith";
pub const FACT_CHECKER: &str = "fact-checker";
pub const PATCH_ANALYST: &str = "patch-analyst";
pub const BUILD_RATER: &str = "build-rater";
pub const SKILL_COACH: &str = "skill-coach";

pub const AGENTS_FILE: &str = "agents.json";

/// Every agent, the companion and background jobs run on Claude Opus 5.5
/// (owner's decision, 2026-10-01). Pinned by full id rather than the `opus`
/// alias so a future Opus release doesn't change behaviour silently.
pub const MODEL: &str = "claude-opus-5-5";

/// Market searching runs on Claude Sonnet 5.5 to save usage (owner's decision,
/// 2026-10-01).
pub const MARKET_MODEL: &str = "claude-sonnet-5-5";

const SHARED: &str = include_str!("../prompts/agents/shared.md");
const WEB: &[&str] = &["WebSearch", "WebFetch"];

/// Built-in tool that produces `--json-schema` output. An agent that runs a
/// background job as the main session must list it, or no structured output
/// is produced (seen live with Claude Code 2.1.286).
pub const STRUCTURED_OUTPUT: &str = "StructuredOutput";
const WEB_REPORT: &[&str] = &["WebSearch", "WebFetch", STRUCTURED_OUTPUT];

/// Claude Code's own agents, as listed in the `init` event of 2.1.286. The
/// companion denies them so it only delegates to the game specialists.
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
    let consults = "Agent(fact-checker, hc-safety-officer)";
    vec![
        AgentSpec {
            name: BUILD_ARCHITECT,
            description: "Designs the player's whole build for their class (Act 1 to Endgame: \
                          passives, ascendancy, skills and supports, gear goals, hardcore safety) and \
                          saves it in the app with design_build, which checks paths, points and gems. \
                          Give it the player's answers (playstyle, ascendancy, trade/budget, safety). \
                          Use for new builds, respecs and reworking a stage.",
            role: include_str!("../prompts/agents/build-architect.md"),
            tools: tool_list(
                &[
                    GAME_DATA,
                    &[BUILD_PLAN, CHARACTER_STATE, DESIGN_BUILD, PROPOSE_ACTION],
                ],
                &["WebSearch", "WebFetch", consults, STRUCTURED_OUTPUT],
            ),
            model: MODEL,
            effort: Some("high"),
            max_turns: Some(120),
        },
        AgentSpec {
            name: BUILD_AUDITOR,
            description: "Audits an imported or newly designed build stage by stage for invalid ids, \
                          unreachable gems or points, resistance gaps and hardcore death risks. Use \
                          after any import or major build change.",
            role: include_str!("../prompts/agents/build-auditor.md"),
            tools: tool_list(
                &[
                    GAME_DATA,
                    &[BUILD_PLAN, CHARACTER_STATE, VALIDATE_BUILD, CAMPAIGN_REWARDS],
                ],
                &[consults, STRUCTURED_OUTPUT],
            ),
            model: MODEL,
            effort: Some("high"),
            max_turns: Some(40),
        },
        AgentSpec {
            name: GEAR_APPRAISER,
            description: "Judges a specific item (usually the one the player just copied) against the \
                          current stage's slot target with hardcore priorities. Use for 'does this \
                          fit' and equip / keep / sell questions.",
            role: include_str!("../prompts/agents/gear-appraiser.md"),
            tools: tool_list(
                &[&[
                    EQUIPPED_ITEMS,
                    LAST_COPIED_ITEM,
                    CHARACTER_STATE,
                    BUILD_PLAN,
                    LOOKUP_BASE,
                    LOOKUP_MOD,
                    LOOKUP_UNIQUE,
                    WIKI,
                    PRICE,
                ]],
                &[],
            ),
            model: MODEL,
            effort: None,
            max_turns: Some(15),
        },
        AgentSpec {
            name: MARKET_SCOUT,
            description: "Builds precise trade searches from the player's plan and budget, ranks \
                          listings by value for this character and queues travel to a seller. Use \
                          for buying gear or pricing an upgrade.",
            role: include_str!("../prompts/agents/market-scout.md"),
            tools: tool_list(
                &[&[
                    BUILD_PLAN,
                    CHARACTER_STATE,
                    TRADE_SEARCH,
                    TRADE_FIND_STAT,
                    PRICE,
                    LOOKUP_BASE,
                    LOOKUP_MOD,
                    LOOKUP_UNIQUE,
                    RATE_LISTINGS,
                    EQUIPPED_ITEMS,
                    PROPOSE_ACTION,
                ]],
                &[],
            ),
            model: MARKET_MODEL,
            effort: None,
            max_turns: Some(20),
        },
        AgentSpec {
            name: HC_SAFETY_OFFICER,
            description: "Finds what is most likely to kill this hardcore character next: resistance \
                          gaps against upcoming penalties, dangerous bosses and areas, missing \
                          permanent buffs. Also debriefs deaths. Use before acts and bosses and to \
                          review risky plans.",
            role: include_str!("../prompts/agents/hc-safety-officer.md"),
            tools: tool_list(
                &[
                    GAME_DATA,
                    &[CHARACTER_STATE, BUILD_PLAN, CAMPAIGN_REWARDS, DEATH_JOURNAL],
                ],
                WEB_REPORT,
            ),
            model: MODEL,
            effort: Some("high"),
            max_turns: Some(25),
        },
        AgentSpec {
            name: ROUTE_COACH,
            description: "Gives the next one to three campaign steps: points to spend, gems now \
                          available, buffs and trials not yet done, next zone. Use for 'what now?' \
                          and overlay tips.",
            role: include_str!("../prompts/agents/route-coach.md"),
            tools: tool_list(
                &[&[CHARACTER_STATE, NEXT_STEPS, CAMPAIGN_REWARDS, AREA_INFO, BUILD_PLAN]],
                &[STRUCTURED_OUTPUT],
            ),
            model: MODEL,
            effort: None,
            max_turns: Some(10),
        },
        AgentSpec {
            name: LOOT_FILTER_SMITH,
            description: "Writes item filter rules on top of the player's NeverSink filter that \
                          highlight this build's bases, gems, charms and currency per stage. Use \
                          when the build or stage changes or the player asks about loot.",
            role: include_str!("../prompts/agents/loot-filter-smith.md"),
            tools: tool_list(
                &[&[
                    BUILD_PLAN,
                    CHARACTER_STATE,
                    LOOKUP_BASE,
                    SEARCH_GAME_DATA,
                    FILTER_PREVIEW,
                    PROPOSE_ACTION,
                ]],
                &[],
            ),
            model: MODEL,
            effort: None,
            max_turns: Some(20),
        },
        AgentSpec {
            name: FACT_CHECKER,
            description: "Adversarially verifies Path of Exile 2 claims against current game data, the \
                          wiki and patch notes, returning CONFIRMED / CONTRADICTED / UNVERIFIED per \
                          claim. Use before stating any mechanic not looked up in this conversation.",
            role: include_str!("../prompts/agents/fact-checker.md"),
            tools: tool_list(&[GAME_DATA], WEB),
            model: MODEL,
            effort: Some("high"),
            max_turns: Some(20),
        },
        AgentSpec {
            name: PATCH_ANALYST,
            description: "Reads the patch notes for a newly detected game version and reports changes \
                          that affect the player's build, campaign route and hardcore safety.",
            role: include_str!("../prompts/agents/patch-analyst.md"),
            tools: tool_list(&[GAME_DATA, &[BUILD_PLAN]], WEB_REPORT),
            model: MODEL,
            effort: None,
            max_turns: Some(25),
        },
        AgentSpec {
            name: BUILD_RATER,
            description:
                "Grades the character F to S+ for its current campaign stage (survivability, gear, damage, plan \
                          progress) with an explanation and the purchases that would raise the grade most. Use for \
                          'rate my build' and the rating screen.",
            role: include_str!("../prompts/agents/build-rater.md"),
            tools: tool_list(
                &[
                    GAME_DATA,
                    &[CHARACTER_STATE, BUILD_PLAN, EQUIPPED_ITEMS, CAMPAIGN_REWARDS, BUILD_ALIGNMENT],
                ],
                &[STRUCTURED_OUTPUT],
            ),
            model: MODEL,
            effort: Some("high"),
            max_turns: Some(40),
        },
        AgentSpec {
            name: SKILL_COACH,
            description: "Sets up skills for the current stage: which skills, which support gems (only ones the game \
                          allows on each skill), which controller button, and rotations for clearing, bossing and \
                          emergencies. Use for skill/support/rotation questions.",
            role: include_str!("../prompts/agents/skill-coach.md"),
            tools: tool_list(
                &[&[
                    CHARACTER_STATE,
                    BUILD_PLAN,
                    LOOKUP_GEM,
                    LOOKUP_SUPPORTS_FOR,
                    LOOKUP_PASSIVE,
                    EQUIPPED_ITEMS,
                ]],
                &[STRUCTURED_OUTPUT],
            ),
            model: MODEL,
            effort: Some("high"),
            max_turns: Some(40),
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
        assert_eq!(agents.len(), 11);
        for (name, spec) in agents {
            for field in ["description", "prompt", "tools", "model", "omitClaudeMd"] {
                assert!(spec.get(field).is_some(), "{name} lacks {field}");
            }
            let expected = if name == MARKET_SCOUT { MARKET_MODEL } else { MODEL };
            assert_eq!(spec["model"], expected, "{name}");
            assert!(
                spec["prompt"].as_str().unwrap().starts_with("You are a specialist"),
                "{name}"
            );
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
    fn only_coordinators_can_delegate_and_checkers_cannot_act() {
        for agent in roster() {
            let delegates = agent.tools.iter().any(|t| t.starts_with("Agent"));
            assert_eq!(
                delegates,
                matches!(agent.name, BUILD_ARCHITECT | BUILD_AUDITOR),
                "{}",
                agent.name
            );
        }
        let checker = roster().into_iter().find(|a| a.name == FACT_CHECKER).unwrap();
        assert!(!checker.tools.contains(&mcp_tool_name(PROPOSE_ACTION)));
    }

    #[test]
    fn writes_agents_file() {
        let dir = std::env::temp_dir().join(format!("lifeline-agents-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_agents_file(&dir).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"hc-safety-officer\""));
        std::fs::remove_dir_all(&dir).ok();
    }
}
