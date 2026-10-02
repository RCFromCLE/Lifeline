//! Minimal MCP server for Claude Code: Streamable-HTTP transport answering
//! every JSON-RPC request with a plain JSON body (no server-initiated SSE),
//! bound to 127.0.0.1 with a per-launch bearer token. Requests go to
//! `/mcp/<scope>`; the scope (e.g. a conversation id) is passed to the
//! handler so tools know where they were called from.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::Arc;

use serde_json::{json, Value};
use tiny_http::{Header, Response, Server};

/// `(scope, tool name, arguments)` → tool output (text or JSON) or an error
/// message shown to Claude.
pub type ToolHandler = Arc<dyn Fn(u64, &str, &Value) -> Result<Value, String> + Send + Sync>;

#[derive(Debug, Clone)]
pub struct McpServer {
    pub port: u16,
    pub token: String,
}

impl McpServer {
    /// Value for `--mcp-config` for one scope.
    pub fn config_json(&self, scope: u64) -> String {
        crate::mcp_config_json(&format!("http://127.0.0.1:{}/mcp/{scope}", self.port), &self.token)
    }
}

fn random_token() -> String {
    (0..2u64)
        .map(|i| {
            let mut h = RandomState::new().build_hasher();
            h.write_u64(i ^ u64::from(std::process::id()));
            format!("{:016x}", h.finish())
        })
        .collect()
}

/// Starts serving `tools` (the `tools/list` array) on a random local port.
pub fn serve(name: &str, tools: Value, handler: ToolHandler) -> Result<McpServer, String> {
    let server = Server::http("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = server.server_addr().to_ip().map(|a| a.port()).ok_or("no port")?;
    let token = random_token();
    let expected = format!("Bearer {token}");
    let tools = Arc::new(tools);
    let name = name.to_owned();
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let authorized = request
                .headers()
                .iter()
                .any(|h| h.field.equiv("Authorization") && h.value.as_str() == expected);
            let scope: Option<u64> = request.url().strip_prefix("/mcp/").and_then(|s| s.parse().ok());
            let Some(scope) = scope.filter(|_| authorized) else {
                let _ = request.respond(Response::empty(401));
                continue;
            };
            if request.method().as_str() != "POST" {
                let _ = request.respond(Response::empty(405));
                continue;
            }
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            let (tools, handler, name) = (tools.clone(), handler.clone(), name.clone());
            std::thread::spawn(move || {
                let reply = serde_json::from_str::<Value>(&body)
                    .ok()
                    .and_then(|msg| handle(&name, &tools, &handler, scope, &msg));
                let response = match reply {
                    Some(v) => Response::from_string(v.to_string()).with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).expect("static header"),
                    ),
                    None => Response::from_string(String::new()).with_status_code(202),
                };
                let _ = request.respond(response);
            });
        }
    });
    Ok(McpServer { port, token })
}

/// JSON-RPC dispatch; `None` for notifications (answered with 202).
pub fn handle(name: &str, tools: &Value, handler: &ToolHandler, scope: u64, msg: &Value) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let method = msg["method"].as_str().unwrap_or("");
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": {"name": name, "version": env!("CARGO_PKG_VERSION")}
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools})),
        "tools/call" => {
            let tool = msg["params"]["name"].as_str().unwrap_or("");
            let args = &msg["params"]["arguments"];
            let (text, is_error) = match handler(scope, tool, args) {
                Ok(Value::String(s)) => (s, false),
                Ok(v) => (v.to_string(), false),
                Err(e) => (e, true),
            };
            Ok(json!({"content": [{"type": "text", "text": text}], "isError": is_error}))
        }
        _ => Err(json!({"code": -32601, "message": format!("method not found: {method}")})),
    };
    Some(match result {
        Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
        Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e}),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handler() -> ToolHandler {
        Arc::new(|scope, tool, args| match tool {
            "echo" => Ok(json!({"scope": scope, "said": args["text"]})),
            _ => Err(format!("unknown tool {tool}")),
        })
    }

    #[test]
    fn json_rpc_round_trip() {
        let tools = json!([{"name": "echo", "inputSchema": {"type": "object"}}]);
        let h = handler();
        let init = handle(
            "t",
            &tools,
            &h,
            7,
            &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}),
        )
        .unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
        assert!(handle(
            "t",
            &tools,
            &h,
            7,
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
        )
        .is_none());
        let list = handle(
            "t",
            &tools,
            &h,
            7,
            &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        )
        .unwrap();
        assert_eq!(list["result"]["tools"][0]["name"], "echo");
        let call = handle("t", &tools, &h, 7, &json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "echo", "arguments": {"text": "hi"}}})).unwrap();
        let text: Value = serde_json::from_str(call["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(text, json!({"scope": 7, "said": "hi"}));
        let bad = handle(
            "t",
            &tools,
            &h,
            7,
            &json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "nope"}}),
        )
        .unwrap();
        assert_eq!(bad["result"]["isError"], true);
    }
}
