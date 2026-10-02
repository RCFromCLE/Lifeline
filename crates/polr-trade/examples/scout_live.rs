//! Live check: the market-scout agent (Opus 5.5) turns a plain request into
//! trade2 searches through our MCP tools and ranks real listings.
//! cargo run -p polr-trade --example scout_live -- "<request>"

use std::sync::Arc;

use polr_ai::mcp_server::serve;
use polr_ai::{agents, ClaudeCli, CliEvent};
use polr_trade::Market;
use serde_json::{json, Value};

fn main() {
    let request = std::env::args().nth(1).unwrap_or_else(|| {
        "Find me Instant Buyout boots in HC Forbidden Rites with at least 25% cold resistance and some life, \
         under 5 exalted. Show the best two."
            .into()
    });
    let market = Arc::new(Market::new());
    let tools = json!([
        {"name": "trade_find_stat", "description": "Find trade-site stat ids by text.",
         "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}, "limit": {"type": "integer"}}, "required": ["query"]}},
        {"name": "trade_search", "description": "Search the PoE2 trade market (league HC Forbidden Rites). Pass the trade site's `query` object. Instant Buyout only unless instant_buyout_only is false.",
         "inputSchema": {"type": "object", "properties": {"query": {"type": "object"}, "sort": {"type": "object"}, "instant_buyout_only": {"type": "boolean"}, "max_listings": {"type": "integer"}}, "required": ["query"]}},
        {"name": "price", "description": "poe.ninja price by name.", "inputSchema": {"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]}},
        {"name": "character_state", "description": "Player state.", "inputSchema": {"type": "object", "properties": {}}}
    ]);
    let m = market.clone();
    let server = serve(
        "polr",
        tools,
        Arc::new(move |_scope, tool, args: &Value| {
            println!("  [tool] {tool} {}", args);
            match tool {
                "trade_find_stat" => Ok(json!(m.find_stats(args["query"].as_str().unwrap_or(""), 6)?)),
                "trade_search" => {
                    let mut q = args["query"].clone();
                    if q.get("status").is_none() {
                        q["status"] = json!({"option": "securable"});
                    }
                    let r = m.search("HC Forbidden Rites", &json!({"query": q}), 8)?;
                    println!("  [tool] → {} results", r.total);
                    Ok(json!({"total": r.total, "search_id": r.query_id, "listings": r.listings}))
                }
                "price" => Ok(json!(
                    m.price("HC Forbidden Rites", args["name"].as_str().unwrap_or(""))?
                )),
                "character_state" => {
                    Ok(json!({"league": "HC Forbidden Rites", "level": 45, "act": 3, "res_penalty": -20}))
                }
                _ => Err(format!("{tool} is not available in this test")),
            }
        }),
    )
    .expect("server");

    let dir = std::env::temp_dir().join("polr-scout-live");
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = dir.join("mcp.json");
    std::fs::write(&cfg, server.config_json(1)).unwrap();
    let mut cli = ClaudeCli::new(dir.clone());
    cli.agents_file = Some(agents::write_agents_file(&dir).unwrap());
    cli.main_agent = Some(agents::MARKET_SCOUT.into());
    cli.mcp_config = Some(cfg);
    cli.allowed_tools
        .extend(["trade_find_stat", "trade_search", "price", "character_state"].map(polr_ai::mcp_tool_name));
    let out = cli.run_turn(&request, None, |e| {
        if let CliEvent::RateLimit(r) = e {
            println!(
                "  [usage] {:?}",
                r.windows.iter().map(|w| (&w.name, w.utilization)).collect::<Vec<_>>()
            );
        }
    });
    match out {
        Ok(o) => println!("\n=== market-scout answer ===\n{}", o.result.result.unwrap_or_default()),
        Err(e) => println!("ERROR {e}"),
    }
}
