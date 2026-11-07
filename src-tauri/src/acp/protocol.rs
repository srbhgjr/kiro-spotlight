//! JSON-RPC 2.0 framing for the Agent Client Protocol (newline-delimited JSON over stdio).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const PROTOCOL_VERSION: u64 = 1;
pub const METHOD_NOT_FOUND: i64 = -32601;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[error("{message} (code {code})")]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct Raw {
    id: Option<Value>,
    method: Option<String>,
    params: Option<Value>,
    result: Option<Value>,
    error: Option<RpcError>,
}

/// A classified inbound message from the agent.
#[derive(Debug, PartialEq)]
pub enum Inbound {
    Response { id: u64, outcome: Result<Value, RpcError> },
    Request { id: Value, method: String, params: Value },
    Notification { method: String, params: Value },
}

/// Parse one line from the agent's stdout. Returns `None` for blank or malformed lines.
pub fn parse_line(line: &str) -> Option<Inbound> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let raw: Raw = serde_json::from_str(trimmed).ok()?;
    classify(raw)
}

fn classify(raw: Raw) -> Option<Inbound> {
    let params = raw.params.unwrap_or(Value::Null);
    match (raw.id, raw.method) {
        (Some(id), Some(method)) => Some(Inbound::Request { id, method, params }),
        (None, Some(method)) => Some(Inbound::Notification { method, params }),
        (Some(id), None) => response(id, raw.result, raw.error),
        (None, None) => None,
    }
}

fn response(id: Value, result: Option<Value>, error: Option<RpcError>) -> Option<Inbound> {
    let id = id.as_u64()?;
    let outcome = match error {
        Some(err) => Err(err),
        None => Ok(result.unwrap_or(Value::Null)),
    };
    Some(Inbound::Response { id, outcome })
}

pub fn request(id: u64, method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

pub fn notification(method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "method": method, "params": params }).to_string()
}

pub fn result(id: &Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

pub fn error(id: &Value, code: i64, message: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }).to_string()
}

/// `initialize` params: no fs or terminal capability, so the agent uses its own tools.
pub fn initialize_params(client_version: &str) -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "clientCapabilities": {
            "fs": { "readTextFile": false, "writeTextFile": false },
            "terminal": false
        },
        "clientInfo": { "name": "kiro-spotlight", "version": client_version }
    })
}

pub fn permission_selected(option_id: &str) -> Value {
    json!({ "outcome": { "outcome": "selected", "optionId": option_id } })
}

pub fn permission_cancelled() -> Value {
    json!({ "outcome": { "outcome": "cancelled" } })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_response_with_result() {
        let msg = parse_line(r#"{"jsonrpc":"2.0","id":2,"result":{"sessionId":"s1"}}"#);
        assert_eq!(
            msg,
            Some(Inbound::Response { id: 2, outcome: Ok(json!({"sessionId":"s1"})) })
        );
    }

    #[test]
    fn parses_response_with_error() {
        let msg = parse_line(r#"{"jsonrpc":"2.0","id":3,"error":{"code":-32000,"message":"nope"}}"#);
        let Some(Inbound::Response { id, outcome: Err(err) }) = msg else {
            panic!("expected error response");
        };
        assert_eq!(id, 3);
        assert_eq!(err.code, -32000);
    }

    #[test]
    fn parses_agent_request() {
        let msg = parse_line(
            r#"{"jsonrpc":"2.0","id":"p-1","method":"session/request_permission","params":{"x":1}}"#,
        );
        assert_eq!(
            msg,
            Some(Inbound::Request {
                id: json!("p-1"),
                method: "session/request_permission".into(),
                params: json!({"x":1}),
            })
        );
    }

    #[test]
    fn parses_notification() {
        let msg = parse_line(r#"{"jsonrpc":"2.0","method":"session/update","params":{}}"#);
        assert!(matches!(msg, Some(Inbound::Notification { ref method, .. }) if method == "session/update"));
    }

    #[test]
    fn ignores_garbage_and_blank_lines() {
        assert_eq!(parse_line(""), None);
        assert_eq!(parse_line("not json"), None);
        assert_eq!(parse_line(r#"{"jsonrpc":"2.0"}"#), None);
    }

    #[test]
    fn encodes_messages_on_one_line() {
        let line = request(7, "session/prompt", json!({"text":"a\nb"}));
        assert!(!line.contains('\n'));
        let back: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(back["id"], 7);
        assert_eq!(back["method"], "session/prompt");
    }
}
