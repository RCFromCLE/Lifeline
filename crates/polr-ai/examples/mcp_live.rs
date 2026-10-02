//! Live check: Claude Code connects to our MCP server and calls a tool.
//! cargo run -p polr-ai --example mcp_live

use std::sync::Arc;

use polr_ai::mcp_server::serve;
use polr_ai::{ClaudeCli, CliEvent};
use serde_json::json;

fn main() {
    let tools = json!([{
        "name": "secret_word",
        "description": "Returns today's secret word.",
        "inputSchema": {"type": "object", "properties": {}}
    }]);
    let server = serve(
        "polr",
        tools,
        Arc::new(|scope, tool, _| {
            println!("  [server] scope {scope} called {tool}");
            Ok(json!("The secret word is LANTERN."))
        }),
    )
    .expect("server");
    let dir = std::env::temp_dir().join("polr-mcp-live");
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = dir.join("mcp.json");
    std::fs::write(&cfg, server.config_json(42)).unwrap();

    let mut cli = ClaudeCli::new(dir);
    cli.model = Some("haiku".into());
    cli.mcp_config = Some(cfg);
    cli.allowed_tools.push(polr_ai::mcp_tool_name("secret_word"));
    let out = cli.run_turn(
        "Call the secret_word tool and reply with just the word.",
        None,
        |e| match e {
            CliEvent::Init { mcp_servers, tools, .. } => println!("init mcp={mcp_servers:?} tools={tools:?}"),
            CliEvent::Assistant { tool_uses, .. } if !tool_uses.is_empty() => {
                println!(
                    "tool calls: {:?}",
                    tool_uses.iter().map(|t| &t.name).collect::<Vec<_>>()
                )
            }
            _ => {}
        },
    );
    match out {
        Ok(o) => println!("result: {:?}", o.result.result),
        Err(e) => println!("ERROR {e}"),
    }
}
