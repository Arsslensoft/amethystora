//! The Model Context Protocol, written by hand: JSON-RPC 2.0, one message a line on stdin and stdout, with
//! initialize (the protocol's version agreed, and the server's short instructions), tools/list, tools/call
//! and ping. The server lives as long as the agent that started it, and nothing else: no daemon, no socket.

use std::io::{BufRead, Write};

use serde_json::{Value, json};

use crate::policy::Policy;
use crate::server::{INSTRUCTIONS, Server};

/// The protocol versions this server speaks, newest first. A client asking for another gets the newest.
pub const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
/// The most a request may be: far more than any tool's arguments
const MAX_LINE: usize = 1 << 20;

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn result(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// The answer to one line, or None for a notification, which has none
pub fn answer(server: &Server, policy: &dyn Fn() -> Policy, line: &str) -> Option<Value> {
    if line.len() > MAX_LINE {
        return Some(error(Value::Null, -32600, "Request too large"));
    }
    let Ok(message) = serde_json::from_str::<Value>(line) else {
        return Some(error(Value::Null, -32700, "Parse error"));
    };
    if message.is_array() {
        return Some(error(Value::Null, -32600, "Batches are not supported"));
    }
    let id = message.get("id").cloned();
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        // A response from the client, which this server never asks for, or nonsense
        return id.map(|id| error(id, -32600, "Invalid Request"));
    };
    if message.get("jsonrpc") != Some(&json!("2.0")) {
        return id.map(|id| error(id, -32600, "Invalid Request"));
    }
    let id = id?;
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
            let version = VERSIONS.iter().find(|known| **known == asked).copied().unwrap_or(VERSIONS[0]);
            result(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": "amethystora", "title": "Amethystora", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": INSTRUCTIONS,
                }),
            )
        }
        "ping" => result(id, json!({})),
        "tools/list" => result(id, json!({"tools": server.tools(policy().level())})),
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return Some(error(id, -32602, "tools/call needs a name"));
            };
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            if !args.is_object() {
                return Some(error(id, -32602, "arguments must be an object"));
            }
            let policy = policy();
            if !server.tools(crate::policy::Level::Act).iter().any(|tool| tool["name"] == name) {
                return Some(error(id, -32602, &format!("Unknown tool: {name}")));
            }
            // The live pill says what is being read, by the tool's own name (R7-13), and takes it back when it is done
            crate::history::working(name, args.get("topic").and_then(Value::as_str));
            let (text, is_error) = match server.call_as(&policy, name, &args) {
                Ok(text) => (text, false),
                Err(text) => (text, true),
            };
            crate::history::worked();
            result(id, json!({"content": [{"type": "text", "text": text}], "isError": is_error}))
        }
        _ => error(id, -32601, &format!("Method not found: {method}")),
    })
}

/// Serves until stdin closes
pub fn serve(server: &Server, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = answer(server, &Policy::current, &line) {
            writeln!(output, "{reply}")?;
            output.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{Conf, Level};
    use std::path::PathBuf;

    fn policy(level: Level) -> Policy {
        Policy {
            conf: Conf::default(),
            account: "test".into(),
            level1: level >= Level::Know,
            level2: level == Level::Act,
        }
    }

    fn ask(server: &Server, level: Level, line: &str) -> Option<Value> {
        answer(server, &|| policy(level), line)
    }

    #[test]
    fn speaks_json_rpc() {
        let server = Server::with_index(PathBuf::from("/nonexistent"));
        let init = ask(&server, Level::Know, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#).unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(init["result"]["serverInfo"]["name"], "amethystora");
        assert!(init["result"]["instructions"].as_str().unwrap().len() < 400);
        let newest = ask(&server, Level::Know, r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}"#).unwrap();
        assert_eq!(newest["result"]["protocolVersion"], VERSIONS[0]);
        assert!(ask(&server, Level::Know, r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
        assert_eq!(ask(&server, Level::Know, r#"{"jsonrpc":"2.0","id":"p","method":"ping"}"#).unwrap()["result"], json!({}));
        assert_eq!(ask(&server, Level::Know, "{nope").unwrap()["error"]["code"], -32700);
        assert_eq!(ask(&server, Level::Know, r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#).unwrap()["error"]["code"], -32600);
        assert_eq!(ask(&server, Level::Know, r#"{"jsonrpc":"2.0","id":3,"method":"resources/list"}"#).unwrap()["error"]["code"], -32601);
        assert_eq!(ask(&server, Level::Know, r#"{"id":3,"method":"ping"}"#).unwrap()["error"]["code"], -32600);
        let unknown = ask(&server, Level::Know, r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"rm","arguments":{}}}"#).unwrap();
        assert_eq!(unknown["error"]["code"], -32602);
    }

    #[test]
    fn lists_and_calls_by_level() {
        let server = Server::with_index(PathBuf::from("/nonexistent"));
        let list = |level| ask(&server, level, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).unwrap()["result"]["tools"].as_array().unwrap().len();
        assert_eq!(list(Level::Off), 0);
        assert_eq!(list(Level::Know), 5);
        assert_eq!(list(Level::Act), 10);
        let call = r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"read","arguments":{"id":"x"}}}"#;
        let refused = ask(&server, Level::Off, call).unwrap();
        assert_eq!(refused["result"]["isError"], true);
        assert!(refused["result"]["content"][0]["text"].as_str().unwrap().contains("off"));
        // With level 1, the call runs: here the index is missing, which it says as a tool's error
        let missing = ask(&server, Level::Know, call).unwrap();
        assert_eq!(missing["result"]["isError"], true);
        assert!(missing["result"]["content"][0]["text"].as_str().unwrap().contains("index"));
        let mut out = Vec::new();
        serve(&server, std::io::Cursor::new("\n{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"ping\"}\n"), &mut out).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert_eq!(out.lines().count(), 1);
        assert_eq!(serde_json::from_str::<Value>(&out).unwrap(), json!({"jsonrpc": "2.0", "id": 9, "result": {}}));
    }
}
