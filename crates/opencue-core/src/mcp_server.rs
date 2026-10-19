//! OpenCue as an MCP server over stdio, so agents backed by the `claude` / `codex` CLIs can use
//! OpenCue's own tools (memory, web, skills, computer, files, email, connected apps). The CLI provider
//! starts it as `opencue mcp-serve --agent <id>`; the desktop binary and `opencue-cli` both serve it.

use std::sync::Arc;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::Core;

const PROTOCOL: &str = "2025-06-18";

/// Answer one JSON-RPC message. Notifications (no `id`) get no answer.
pub async fn handle(core: &Arc<Core>, agent_id: &str, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned()?;
    let method = msg
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(PROTOCOL),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "opencue", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Tools from OpenCue, the app you are running in: your memory, web search, skills, your computer and files, email and connected apps."
        })),
        "ping" => Ok(json!({})),
        "tools/list" => match core.external_tools(agent_id).await {
            Ok(specs) => Ok(json!({
                "tools": specs
                    .into_iter()
                    .map(|t| json!({
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": t.parameters,
                    }))
                    .collect::<Vec<_>>()
            })),
            Err(e) => Err((-32603, format!("{e:#}"))),
        },
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let args = params
                .get("arguments")
                .cloned()
                .filter(Value::is_object)
                .unwrap_or_else(|| json!({}));
            let (text, is_error) = match core.call_external_tool(agent_id, name, args).await {
                Ok(t) => (t, false),
                Err(e) => (format!("{e:#}"), true),
            };
            Ok(json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }))
        }
        other => Err((-32601, format!("method not found: {other}"))),
    };
    Some(match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err((code, message)) => {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
        }
    })
}

/// Serve newline-delimited JSON-RPC on stdin/stdout until stdin closes.
pub async fn serve_stdio(core: Arc<Core>, agent_id: &str) -> anyhow::Result<()> {
    core.store.get_agent(agent_id).await?;
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut out = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(&core, agent_id, &msg).await,
            Err(e) => Some(json!({
                "jsonrpc": "2.0", "id": null,
                "error": { "code": -32700, "message": format!("parse error: {e}") }
            })),
        };
        if let Some(reply) = reply {
            out.write_all(reply.to_string().as_bytes()).await?;
            out.write_all(b"\n").await?;
            out.flush().await?;
        }
    }
    Ok(())
}

/// Parse `mcp-serve --agent <id>` from a command line, if that is what was asked for.
pub fn agent_from_args(args: &[String]) -> Option<String> {
    let pos = args.iter().position(|a| a == "mcp-serve")?;
    args[pos + 1..]
        .windows(2)
        .find(|w| w[0] == "--agent")
        .map(|w| w[1].clone())
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_serve_arguments() {
        let args: Vec<String> = ["opencue", "mcp-serve", "--agent", "abc"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(super::agent_from_args(&args).as_deref(), Some("abc"));
        assert_eq!(super::agent_from_args(&args[..2]), None);
    }
}
