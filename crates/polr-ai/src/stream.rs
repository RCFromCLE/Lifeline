//! Tolerant parser for `claude -p --output-format stream-json` lines. Only the
//! documented fields are interpreted; anything else is passed through as
//! [`CliEvent::Other`] so new CLI versions don't break the app.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum CliEvent {
    /// `{"type":"system","subtype":"init",...}` — first event of a run.
    Init {
        session_id: String,
        model: Option<String>,
        tools: Vec<String>,
        /// `(name, status)` for each MCP server; anything but `connected`
        /// means our game-data tools are unavailable this turn.
        mcp_servers: Vec<(String, String)>,
        /// Agent types the session can delegate to.
        agents: Vec<String>,
    },
    /// `{"type":"rate_limit_event","rate_limit_info":{...}}` — the
    /// subscription's usage windows (five-hour, seven-day), sent each turn.
    RateLimit(RateLimitInfo),
    /// Token-level text from `--include-partial-messages`.
    TextDelta(String),
    /// A complete assistant message: its text and any tool calls.
    Assistant {
        text: String,
        tool_uses: Vec<ToolUse>,
    },
    /// `{"type":"system","subtype":"api_retry",...}` — rate limits, overload.
    ApiRetry {
        attempt: u64,
        max_retries: u64,
        error_status: Option<u64>,
        error: Option<String>,
    },
    Result(RunResult),
    Other(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolUse {
    pub id: String,
    pub name: String,
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunResult {
    /// `success`, or an error subtype such as `error_max_turns`.
    pub subtype: String,
    pub is_error: bool,
    pub result: Option<String>,
    pub session_id: Option<String>,
    pub total_cost_usd: Option<f64>,
    pub usage: Option<Value>,
    /// Output validated against `--json-schema` (the session needs the
    /// `StructuredOutput` tool for this to be produced).
    pub structured_output: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RateLimitInfo {
    /// `allowed` while under the limit.
    pub status: String,
    /// The window currently governing, e.g. `five_hour`.
    pub limit_type: Option<String>,
    pub windows: Vec<UsageWindow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsageWindow {
    /// `five_hour`, `seven_day`, ...
    pub name: String,
    /// Fraction of the window used, 0.0–1.0.
    pub utilization: f64,
    /// Unix seconds.
    pub resets_at: Option<i64>,
}

pub fn parse_event(line: &str) -> Result<CliEvent, serde_json::Error> {
    let v: Value = serde_json::from_str(line)?;
    let ty = str_field(&v, "type").unwrap_or_default();
    let subtype = str_field(&v, "subtype").unwrap_or_default();
    Ok(match (ty.as_str(), subtype.as_str()) {
        ("system", "init") => CliEvent::Init {
            session_id: str_field(&v, "session_id").unwrap_or_default(),
            model: str_field(&v, "model"),
            tools: v
                .get("tools")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|t| t.as_str().map(str::to_owned)).collect())
                .unwrap_or_default(),
            mcp_servers: v
                .get("mcp_servers")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|s| {
                            (
                                str_field(s, "name").unwrap_or_default(),
                                str_field(s, "status").unwrap_or_default(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            agents: string_list(&v, "agents"),
        },
        ("rate_limit_event", _) => {
            let info = v.get("rate_limit_info").cloned().unwrap_or(Value::Null);
            let windows = info
                .get("unifiedWindows")
                .and_then(Value::as_object)
                .map(|w| {
                    w.iter()
                        .map(|(name, win)| UsageWindow {
                            name: name.clone(),
                            utilization: win.get("utilization").and_then(Value::as_f64).unwrap_or(0.0),
                            resets_at: win.get("resetsAt").and_then(Value::as_i64),
                        })
                        .collect()
                })
                .unwrap_or_default();
            CliEvent::RateLimit(RateLimitInfo {
                status: str_field(&info, "status").unwrap_or_default(),
                limit_type: str_field(&info, "rateLimitType"),
                windows,
            })
        }
        ("system", "api_retry") => CliEvent::ApiRetry {
            attempt: v.get("attempt").and_then(Value::as_u64).unwrap_or(0),
            max_retries: v.get("max_retries").and_then(Value::as_u64).unwrap_or(0),
            error_status: v.get("error_status").and_then(Value::as_u64),
            error: str_field(&v, "error"),
        },
        ("stream_event", _) => {
            let text = v
                .pointer("/event/delta")
                .filter(|d| d.get("type").and_then(Value::as_str) == Some("text_delta"))
                .and_then(|d| d.get("text"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            match text {
                Some(t) => CliEvent::TextDelta(t),
                None => CliEvent::Other(v),
            }
        }
        ("assistant", _) => {
            let blocks = v.pointer("/message/content").and_then(Value::as_array);
            let mut text = String::new();
            let mut tool_uses = Vec::new();
            for block in blocks.into_iter().flatten() {
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => text.push_str(block.get("text").and_then(Value::as_str).unwrap_or("")),
                    Some("tool_use") => tool_uses.push(ToolUse {
                        id: str_field(block, "id").unwrap_or_default(),
                        name: str_field(block, "name").unwrap_or_default(),
                        input: block.get("input").cloned().unwrap_or(Value::Null),
                    }),
                    _ => {}
                }
            }
            CliEvent::Assistant { text, tool_uses }
        }
        ("result", _) => CliEvent::Result(RunResult {
            subtype: subtype.clone(),
            is_error: v.get("is_error").and_then(Value::as_bool).unwrap_or(false),
            result: str_field(&v, "result"),
            session_id: str_field(&v, "session_id"),
            total_cost_usd: v.get("total_cost_usd").and_then(Value::as_f64),
            usage: v.get("usage").cloned(),
            structured_output: v.get("structured_output").cloned(),
        }),
        _ => CliEvent::Other(v),
    })
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn string_list(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_event() {
        let e = parse_event(r#"{"type":"system","subtype":"init","session_id":"s1","model":"claude-sonnet-5-5","tools":["WebSearch","mcp__polr__lookup_item"],"mcp_servers":[{"name":"polr","status":"connected"}],"agents":["route-coach"]}"#).unwrap();
        assert_eq!(
            e,
            CliEvent::Init {
                session_id: "s1".into(),
                model: Some("claude-sonnet-5-5".into()),
                tools: vec!["WebSearch".into(), "mcp__polr__lookup_item".into()],
                mcp_servers: vec![("polr".into(), "connected".into())],
                agents: vec!["route-coach".into()],
            }
        );
    }

    #[test]
    fn rate_limit_event_from_a_real_run() {
        // Captured from Claude Code 2.1.286 on a Max subscription.
        let e = parse_event(r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","resetsAt":1790878800,"rateLimitType":"five_hour","overageStatus":"rejected","overageDisabledReason":"org_level_disabled","isUsingOverage":false,"unifiedWindows":{"five_hour":{"utilization":0.09,"resetsAt":1790878800},"seven_day":{"utilization":0.03,"resetsAt":1791435600}}},"uuid":"ba9801df","session_id":"84916b93"}"#).unwrap();
        let CliEvent::RateLimit(info) = e else {
            panic!("expected rate limit")
        };
        assert_eq!(
            (info.status.as_str(), info.limit_type.as_deref()),
            ("allowed", Some("five_hour"))
        );
        let five = info.windows.iter().find(|w| w.name == "five_hour").unwrap();
        assert_eq!((five.utilization, five.resets_at), (0.09, Some(1790878800)));
        assert_eq!(info.windows.len(), 2);
    }

    #[test]
    fn text_delta_and_other_stream_events() {
        assert_eq!(
            parse_event(r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Cap "}}}"#).unwrap(),
            CliEvent::TextDelta("Cap ".into())
        );
        assert!(matches!(
            parse_event(r#"{"type":"stream_event","event":{"type":"message_start"}}"#).unwrap(),
            CliEvent::Other(_)
        ));
    }

    #[test]
    fn assistant_with_tool_call() {
        let e = parse_event(r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Checking."},{"type":"tool_use","id":"t1","name":"mcp__polr__lookup_item","input":{"name":"Trenchtimbre"}}]}}"#).unwrap();
        let CliEvent::Assistant { text, tool_uses } = e else {
            panic!("expected assistant")
        };
        assert_eq!(text, "Checking.");
        assert_eq!(tool_uses[0].name, "mcp__polr__lookup_item");
        assert_eq!(tool_uses[0].input["name"], "Trenchtimbre");
    }

    #[test]
    fn result_and_retry() {
        let e = parse_event(r#"{"type":"result","subtype":"success","result":"Yes.","is_error":false,"session_id":"s1","usage":{"input_tokens":5},"total_cost_usd":0.01}"#).unwrap();
        let CliEvent::Result(r) = e else {
            panic!("expected result")
        };
        assert_eq!(
            (r.subtype.as_str(), r.result.as_deref(), r.is_error),
            ("success", Some("Yes."), false)
        );

        let e = parse_event(r#"{"type":"system","subtype":"api_retry","attempt":1,"max_retries":10,"retry_delay_ms":500,"error_status":429,"error":"rate_limit"}"#).unwrap();
        assert_eq!(
            e,
            CliEvent::ApiRetry {
                attempt: 1,
                max_retries: 10,
                error_status: Some(429),
                error: Some("rate_limit".into())
            }
        );
    }
}
