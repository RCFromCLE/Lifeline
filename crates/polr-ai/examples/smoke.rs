//! Live check against the installed Claude Code CLI, using whatever account it
//! is logged into (the subscription in normal use). Spends a little usage.
//!
//! cargo run -p polr-ai --example smoke

use polr_ai::jobs::Job;
use polr_ai::{agents, parse_report, ClaudeCli, CliEvent};

fn print(event: &CliEvent) {
    match event {
        CliEvent::Init {
            session_id,
            model,
            tools,
            mcp_servers,
            agents,
        } => println!(
            "init  session={session_id} model={model:?}\n      tools={tools:?}\n      mcp={mcp_servers:?}\n      agents={agents:?}"
        ),
        CliEvent::RateLimit(info) => {
            let windows: Vec<String> = info
                .windows
                .iter()
                .map(|w| format!("{} {:.0}%", w.name, w.utilization * 100.0))
                .collect();
            println!("usage {} [{}]", info.status, windows.join(", "))
        }
        CliEvent::TextDelta(_) => {}
        CliEvent::Assistant { text, tool_uses } => {
            let names: Vec<&str> = tool_uses.iter().map(|t| t.name.as_str()).collect();
            println!("asst  {text:?} tools={names:?}")
        }
        CliEvent::ApiRetry { attempt, error, .. } => println!("retry #{attempt} {error:?}"),
        CliEvent::Result(r) => println!(
            "done  {} is_error={} structured_output={} text={:?}",
            r.subtype,
            r.is_error,
            r.structured_output.is_some(),
            r.result
        ),
        CliEvent::Other(_) => {}
    }
}

fn main() {
    let dir = std::env::temp_dir().join("polr-smoke");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let mut cli = ClaudeCli::new(dir.clone());
    cli.agents_file = Some(agents::write_agents_file(&dir).expect("agents file"));
    cli.builtin_tools.push("Agent".into());
    cli.allowed_tools.push("Agent".into());
    cli.disallowed_tools = agents::BUILT_IN_AGENTS.iter().map(|a| format!("Agent({a})")).collect();
    println!("program: {}", cli.program.display());

    println!("\n== companion turn (--restricted, strict MCP, specialists only) ==");
    match cli.run_turn("Reply with exactly the word: ready", None, print) {
        Ok(outcome) => println!("ok, session {:?}", outcome.session_id),
        Err(e) => println!("ERROR {e}"),
    }

    println!("\n== background job: route-coach as main agent with the report schema ==");
    let job = Job::SessionRecap;
    let context = r#"{"note":"Smoke test. No game data or tools are connected yet; say so in the summary and return zero findings."}"#;
    match cli.for_job(job).run_turn(&job.prompt(context), None, print) {
        Ok(outcome) => println!("report: {:#?}", parse_report(&outcome.result)),
        Err(e) => println!("ERROR {e}"),
    }
}
