//! Bridge to the user's own Claude Code CLI (`claude -p`), which runs on their
//! Claude Pro/Max login. The app never sees, stores or forwards Claude
//! credentials; it only launches the CLI the user installed and logged into.
//! Flags and stream-json shapes: PLAN.md §6 (sources: code.claude.com
//! cli-reference, headless, sub-agents, and `claude --help` of 2.1.286).

pub mod agents;
pub mod jobs;
pub mod mcp_server;
pub mod stream;
pub mod tools;

use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::json;

pub use agents::{roster, write_agents_file, AgentSpec};
pub use jobs::{parse_report, Finding, Job, JobReport, Severity};
pub use stream::{parse_event, CliEvent, RunResult, ToolUse};

/// Name our MCP server is registered under; tools appear to Claude as
/// `mcp__polr__<tool>`.
pub const MCP_SERVER_NAME: &str = "polr";

pub const SYSTEM_PROMPT: &str = include_str!("../prompts/companion.md");

#[derive(Debug, Clone)]
pub struct ClaudeCli {
    /// `claude` on PATH, or an absolute path. Prefer the native installer's
    /// `claude.exe`: npm's `claude.cmd` shim needs batch-file argument quoting.
    pub program: PathBuf,
    /// App-owned directory so no project `CLAUDE.md`/settings leak in.
    pub working_dir: PathBuf,
    /// Model id or alias; defaults to [`agents::MODEL`] (Opus 5.5). `None`
    /// keeps the user's Claude Code default.
    pub model: Option<String>,
    /// Used when no `main_agent` is set.
    pub system_prompt: String,
    /// Run a specialist as the main session (`--agent <name>`) instead of the
    /// companion system prompt; background jobs do this.
    pub main_agent: Option<String>,
    /// `--agents <file>`: the specialist roster, see [`agents::write_agents_file`].
    pub agents_file: Option<PathBuf>,
    /// Path to a `{"mcpServers": {...}}` file, see [`mcp_config_json`].
    pub mcp_config: Option<PathBuf>,
    /// Built-in tools to expose (`--tools`); empty disables all of them.
    pub builtin_tools: Vec<String>,
    /// Tools that run without a permission prompt; everything else is denied
    /// (`--permission-mode dontAsk`).
    pub allowed_tools: Vec<String>,
    /// `--disallowedTools`, e.g. `Agent(Explore)` to hide a built-in agent.
    pub disallowed_tools: Vec<String>,
    /// `--restricted`: no code-running tools, ignores user/project/local
    /// settings files, file tools confined to the working directory.
    pub restricted: bool,
    /// `false` adds `--no-session-persistence` (one-shot background jobs).
    pub persist_session: bool,
    /// `--json-schema` for structured output.
    pub json_schema: Option<String>,
    /// `--max-turns` is described in the headless docs but not listed by
    /// `claude --help` 2.1.286, so it is off unless set explicitly; agents
    /// carry their own `maxTurns`.
    pub max_turns: Option<u32>,
}

impl ClaudeCli {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            program: default_program(),
            working_dir,
            model: Some(agents::MODEL.to_owned()),
            system_prompt: SYSTEM_PROMPT.to_owned(),
            main_agent: None,
            agents_file: None,
            mcp_config: None,
            builtin_tools: vec!["WebSearch".into(), "WebFetch".into()],
            allowed_tools: vec!["WebSearch".into(), "WebFetch".into()],
            disallowed_tools: Vec::new(),
            restricted: true,
            persist_session: true,
            json_schema: None,
            max_turns: None,
        }
    }

    /// The full companion: our MCP tools, the specialist roster, and the
    /// `Agent` tool so it can delegate to them.
    pub fn companion(working_dir: PathBuf, mcp_config: PathBuf, agents_file: PathBuf) -> Self {
        let mut cli = Self::new(working_dir);
        cli.mcp_config = Some(mcp_config);
        cli.agents_file = Some(agents_file);
        cli.builtin_tools.push("Agent".into());
        cli.allowed_tools = ["WebSearch", "WebFetch", "Agent"]
            .into_iter()
            .map(str::to_owned)
            .chain(tools::qualified(tools::ALL))
            .collect();
        // Claude Code's own coding agents are no use in a game companion.
        cli.disallowed_tools = agents::BUILT_IN_AGENTS.iter().map(|a| format!("Agent({a})")).collect();
        cli
    }

    /// Builds one headless turn. The prompt goes right after `-p` because
    /// list-valued flags such as `--allowedTools` would otherwise swallow it.
    pub fn command(&self, prompt: &str, resume_session: Option<&str>) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.current_dir(&self.working_dir)
            .arg("-p")
            .arg(guard_leading_dash(prompt))
            .args([
                "--output-format",
                "stream-json",
                "--verbose",
                "--include-partial-messages",
            ])
            .args(["--permission-mode", "dontAsk"])
            .arg("--tools")
            .arg(self.builtin_tools.join(","));
        if self.restricted {
            cmd.arg("--restricted");
        }
        match &self.main_agent {
            Some(agent) => cmd.arg("--agent").arg(agent),
            None => cmd.arg("--system-prompt").arg(&self.system_prompt),
        };
        if let Some(file) = &self.agents_file {
            cmd.arg("--agents").arg(file);
        }
        if !self.allowed_tools.is_empty() {
            cmd.arg("--allowedTools").arg(self.allowed_tools.join(","));
        }
        if !self.disallowed_tools.is_empty() {
            cmd.arg("--disallowedTools").arg(self.disallowed_tools.join(","));
        }
        // Always strict: without it the session also loads the user's other
        // MCP servers, including claude.ai connectors such as Gmail and Drive.
        if let Some(cfg) = &self.mcp_config {
            cmd.arg("--mcp-config").arg(cfg);
        }
        cmd.arg("--strict-mcp-config");
        if let Some(model) = &self.model {
            cmd.arg("--model").arg(model);
        }
        if let Some(n) = self.max_turns {
            cmd.arg("--max-turns").arg(n.to_string());
        }
        if let Some(schema) = &self.json_schema {
            cmd.arg("--json-schema").arg(schema);
        }
        if !self.persist_session {
            cmd.arg("--no-session-persistence");
        }
        if let Some(id) = resume_session {
            cmd.arg("--resume").arg(id);
        }
        // An API key in the environment outranks the subscription login, so
        // drop it: this mode must bill the user's Claude plan, not the API.
        cmd.env_remove("ANTHROPIC_API_KEY").env_remove("ANTHROPIC_AUTH_TOKEN");
        cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd
    }

    /// Runs one turn to completion, calling `on_event` for every stream event
    /// (text deltas, tool calls, retries) as it arrives.
    pub fn run_turn(
        &self,
        prompt: &str,
        resume_session: Option<&str>,
        mut on_event: impl FnMut(&CliEvent),
    ) -> Result<TurnOutcome, AiError> {
        let mut child = self.command(prompt, resume_session).spawn().map_err(AiError::Spawn)?;
        let stdout = child.stdout.take().expect("stdout is piped");
        let mut stderr = child.stderr.take().expect("stderr is piped");
        // Drain stderr concurrently so a chatty CLI can't block on a full pipe.
        let stderr_reader = std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stderr.read_to_string(&mut text);
            text
        });

        let mut init_session = None;
        let mut result = None;
        for line in BufReader::new(stdout).lines() {
            let line = line.map_err(AiError::Io)?;
            if line.trim().is_empty() {
                continue;
            }
            let Ok(event) = parse_event(&line) else { continue };
            match &event {
                CliEvent::Init { session_id, .. } => init_session = Some(session_id.clone()),
                CliEvent::Result(r) => result = Some(r.clone()),
                _ => {}
            }
            on_event(&event);
        }
        let status = child.wait().map_err(AiError::Io)?;
        let stderr = stderr_reader.join().unwrap_or_default();
        match result {
            Some(result) => Ok(TurnOutcome {
                session_id: result.session_id.clone().or(init_session),
                result,
            }),
            None => Err(AiError::NoResult {
                exit_code: status.code(),
                stderr,
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TurnOutcome {
    /// Pass back as `resume_session` to continue the same conversation.
    pub session_id: Option<String>,
    pub result: RunResult,
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("could not start the Claude Code CLI (is it installed and on PATH?): {0}")]
    Spawn(std::io::Error),
    #[error("i/o error talking to the Claude Code CLI: {0}")]
    Io(std::io::Error),
    #[error("Claude Code exited (code {exit_code:?}) without a result: {stderr}")]
    NoResult { exit_code: Option<i32>, stderr: String },
}

/// `{"mcpServers": {"polr": {"type": "http", ...}}}` pointing Claude Code at
/// the app's local MCP endpoint. The bearer token is generated per app launch.
pub fn mcp_config_json(url: &str, bearer_token: &str) -> String {
    let mut servers = serde_json::Map::new();
    servers.insert(
        MCP_SERVER_NAME.to_owned(),
        json!({
            "type": "http",
            "url": url,
            "headers": { "Authorization": format!("Bearer {bearer_token}") }
        }),
    );
    json!({ "mcpServers": servers }).to_string()
}

/// The native installer puts `claude.exe` in `%USERPROFILE%\.local\bin`,
/// which isn't always on PATH; prefer it when present.
fn default_program() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(|home| PathBuf::from(home).join(".local").join("bin").join("claude.exe"))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("claude"))
}

/// `lookup_item` → `mcp__polr__lookup_item`.
pub fn mcp_tool_name(tool: &str) -> String {
    format!("mcp__{MCP_SERVER_NAME}__{tool}")
}

/// A prompt starting with `-` would be parsed as a flag.
fn guard_leading_dash(prompt: &str) -> String {
    if prompt.starts_with('-') {
        format!(" {prompt}")
    } else {
        prompt.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(cmd: &Command) -> Vec<String> {
        cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn prompt_follows_print_flag_and_resume_is_last() {
        let mut cli = ClaudeCli::new(std::env::temp_dir());
        cli.allowed_tools.push(mcp_tool_name("lookup_item"));
        let a = args(&cli.command("does this ring fit?", Some("sess_1")));
        assert_eq!(&a[..2], ["-p", "does this ring fit?"]);
        assert!(a
            .windows(2)
            .any(|w| w == ["--allowedTools", "WebSearch,WebFetch,mcp__polr__lookup_item"]));
        assert_eq!(&a[a.len() - 2..], ["--resume", "sess_1"]);
    }

    #[test]
    fn subscription_mode_strips_api_keys() {
        let cmd = ClaudeCli::new(std::env::temp_dir()).command("hi", None);
        let removed: Vec<_> = cmd
            .get_envs()
            .filter(|(_, v)| v.is_none())
            .map(|(k, _)| k.to_string_lossy().into_owned())
            .collect();
        assert!(removed.contains(&"ANTHROPIC_API_KEY".to_owned()));
    }

    #[test]
    fn companion_gets_agents_tools_and_isolation() {
        let dir = std::env::temp_dir();
        let cli = ClaudeCli::companion(dir.clone(), dir.join("mcp.json"), dir.join("agents.json"));
        let a = args(&cli.command("hi", None));
        assert!(a.windows(2).any(|w| w == ["--tools", "WebSearch,WebFetch,Agent"]));
        assert!(a
            .windows(2)
            .any(|w| w[0] == "--agents" && w[1].ends_with("agents.json")));
        assert!(a.iter().any(|x| x == "--restricted"));
        assert!(a.iter().any(|x| x == "--system-prompt"));
        assert!(!a.iter().any(|x| x == "--max-turns"));
        assert!(a.windows(2).any(|w| w == ["--model", "claude-opus-5-5"]));
        let allowed = a
            .windows(2)
            .find(|w| w[0] == "--allowedTools")
            .map(|w| w[1].clone())
            .unwrap();
        assert!(allowed.contains("Agent") && allowed.contains("mcp__polr__propose_action"));
    }

    #[test]
    fn leading_dash_is_guarded() {
        assert_eq!(guard_leading_dash("-5% res?"), " -5% res?");
    }

    #[test]
    fn mcp_config_shape() {
        let v: serde_json::Value = serde_json::from_str(&mcp_config_json("http://127.0.0.1:47123/mcp", "t0k")).unwrap();
        assert_eq!(v["mcpServers"]["polr"]["type"], "http");
        assert_eq!(v["mcpServers"]["polr"]["headers"]["Authorization"], "Bearer t0k");
    }
}
