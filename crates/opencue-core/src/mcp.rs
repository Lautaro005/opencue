//! MCP client manager: stdio servers (Claude Desktop-style JSON) and Streamable HTTP servers by URL.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use rmcp::model::{CallToolRequestParams, Tool};
use rmcp::service::RunningService;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::{StreamableHttpClientTransport, TokioChildProcess};
use rmcp::{RoleClient, ServiceExt};
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::models::{McpServerConfig, McpServerStatus, McpTransport};

type Client = RunningService<RoleClient, ()>;

struct Connection {
    client: Client,
    tools: Vec<Tool>,
}

#[derive(Default)]
struct Slot {
    conn: Option<Arc<Connection>>,
    state: String,
    error: Option<String>,
}

#[derive(Default, Clone)]
pub struct McpManager {
    slots: Arc<RwLock<HashMap<String, Slot>>>,
    /// Where keychain-held secrets are read from (see [`KEYCHAIN_PREFIX`]).
    secrets: Option<Arc<crate::secrets::Secrets>>,
}

/// A header value written `keychain:<name>` is replaced by that secret when the server connects,
/// so the key itself never sits in the database.
pub const KEYCHAIN_PREFIX: &str = "keychain:";

/// A tool exposed to agents as `mcp__<server>__<tool>`.
#[derive(Debug, Clone)]
pub struct McpTool {
    pub server_id: String,
    pub server_name: String,
    pub tool: Tool,
}

impl McpTool {
    pub fn qualified_name(&self) -> String {
        format!("{}{}", tool_prefix(&self.server_name), self.tool.name)
    }

    /// Per the MCP spec, tools are assumed destructive unless they say otherwise.
    pub fn is_destructive(&self) -> bool {
        match &self.tool.annotations {
            Some(a) if a.read_only_hint == Some(true) => false,
            Some(a) => a.destructive_hint != Some(false),
            None => true,
        }
    }
}

/// `mcp__<server>__`: what every tool of a server starts with. Long names are cut and given a
/// short hash, so two machines named alike never share tool names (APIs allow 64 characters).
pub fn tool_prefix(server_name: &str) -> String {
    format!("mcp__{}__", sanitize(server_name))
}

pub fn sanitize(name: &str) -> String {
    const MAX: usize = 24;
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.chars().count() <= MAX {
        return s;
    }
    let hash = name.bytes().fold(0xcbf29ce4_u32, |h, b| {
        (h ^ b as u32).wrapping_mul(0x0100_0193)
    });
    let head: String = s.chars().take(MAX - 5).collect();
    format!("{head}-{:04x}", hash & 0xffff)
}

impl McpManager {
    pub fn new(secrets_fallback: std::path::PathBuf) -> Self {
        Self {
            slots: Arc::default(),
            secrets: Some(Arc::new(crate::secrets::Secrets::new(secrets_fallback))),
        }
    }

    fn resolve_headers(&self, cfg: &McpServerConfig) -> BTreeMap<String, String> {
        cfg.headers
            .iter()
            .map(|(k, v)| {
                let value = v
                    .strip_prefix(KEYCHAIN_PREFIX)
                    .and_then(|name| self.secrets.as_ref()?.get(name))
                    .unwrap_or_else(|| v.clone());
                (k.clone(), value)
            })
            .collect()
    }

    pub async fn connect(&self, cfg: &McpServerConfig) {
        {
            let mut slots = self.slots.write().await;
            let slot = slots.entry(cfg.id.clone()).or_default();
            slot.state = "connecting".into();
            slot.error = None;
        }
        let resolved = McpServerConfig {
            headers: self.resolve_headers(cfg),
            ..cfg.clone()
        };
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(60), open(&resolved)).await;
        let mut slots = self.slots.write().await;
        let slot = slots.entry(cfg.id.clone()).or_default();
        match result {
            Ok(Ok(conn)) => {
                slot.conn = Some(Arc::new(conn));
                slot.state = "ready".into();
            }
            Ok(Err(e)) => {
                tracing::warn!("MCP server {} failed: {e:#}", cfg.name);
                slot.conn = None;
                slot.state = "error".into();
                slot.error = Some(format!("{e:#}"));
            }
            Err(_) => {
                slot.conn = None;
                slot.state = "error".into();
                slot.error = Some("timed out while starting".into());
            }
        }
    }

    pub async fn is_ready(&self, id: &str) -> bool {
        self.slots
            .read()
            .await
            .get(id)
            .is_some_and(|s| s.conn.is_some())
    }

    pub async fn disconnect(&self, id: &str) {
        let slot = self.slots.write().await.remove(id);
        if let Some(conn) = slot.and_then(|s| s.conn) {
            if let Ok(conn) = Arc::try_unwrap(conn) {
                let _ = conn.client.cancel().await;
            }
        }
    }

    pub async fn status(&self, servers: &[McpServerConfig]) -> Vec<McpServerStatus> {
        let slots = self.slots.read().await;
        servers
            .iter()
            .map(|s| {
                let slot = slots.get(&s.id);
                McpServerStatus {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    state: slot
                        .map(|x| x.state.clone())
                        .unwrap_or_else(|| "stopped".into()),
                    error: slot.and_then(|x| x.error.clone()),
                    tools: slot
                        .and_then(|x| x.conn.as_ref())
                        .map(|c| c.tools.iter().map(|t| t.name.to_string()).collect())
                        .unwrap_or_default(),
                }
            })
            .collect()
    }

    pub async fn tools(&self, servers: &[McpServerConfig]) -> Vec<McpTool> {
        let slots = self.slots.read().await;
        servers
            .iter()
            .filter_map(|s| Some((s, slots.get(&s.id)?.conn.clone()?)))
            .flat_map(|(s, conn)| {
                conn.tools
                    .iter()
                    .map(|t| McpTool {
                        server_id: s.id.clone(),
                        server_name: s.name.clone(),
                        tool: t.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub async fn call(
        &self,
        server_id: &str,
        tool: &str,
        args: serde_json::Value,
    ) -> anyhow::Result<(String, bool)> {
        let conn = self
            .slots
            .read()
            .await
            .get(server_id)
            .and_then(|s| s.conn.clone())
            .ok_or_else(|| anyhow::anyhow!("MCP server is not connected"))?;
        let mut params = CallToolRequestParams::new(tool.to_string());
        if let serde_json::Value::Object(map) = args {
            params = params.with_arguments(map);
        }
        let result = conn.client.call_tool(params).await?;
        let mut text = result
            .content
            .iter()
            .map(|c| match c.as_text() {
                Some(t) => t.text.clone(),
                None => serde_json::to_string(c).unwrap_or_default(),
            })
            .collect::<Vec<_>>()
            .join("\n");
        if text.is_empty() {
            if let Some(s) = &result.structured_content {
                text = s.to_string();
            }
        }
        Ok((text, result.is_error.unwrap_or(false)))
    }
}

async fn open(cfg: &McpServerConfig) -> anyhow::Result<Connection> {
    let client: Client = match cfg.transport {
        McpTransport::Stdio => {
            let program = cfg
                .command
                .as_deref()
                .filter(|c| !c.is_empty())
                .ok_or_else(|| anyhow::anyhow!("missing command"))?;
            // Resolve through PATH so `npx` → `npx.cmd` works on Windows.
            let resolved = which::which(program).unwrap_or_else(|_| program.into());
            let mut cmd = tokio::process::Command::new(resolved);
            cmd.args(&cfg.args).envs(&cfg.env);
            #[cfg(windows)]
            {
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            let (transport, _stderr) = TokioChildProcess::builder(cmd)
                .stderr(std::process::Stdio::null())
                .spawn()?;
            ().serve(transport).await?
        }
        McpTransport::Http => {
            let url = cfg
                .url
                .as_deref()
                .filter(|u| !u.is_empty())
                .ok_or_else(|| anyhow::anyhow!("missing URL"))?;
            let mut config = StreamableHttpClientTransportConfig::with_uri(url.to_string());
            let mut headers = HashMap::new();
            for (k, v) in &cfg.headers {
                if k.eq_ignore_ascii_case("authorization") {
                    config = config.auth_header(v.trim_start_matches("Bearer ").to_string());
                } else {
                    headers.insert(
                        reqwest::header::HeaderName::from_bytes(k.as_bytes())?,
                        reqwest::header::HeaderValue::from_str(v)?,
                    );
                }
            }
            if !headers.is_empty() {
                config = config.custom_headers(headers);
            }
            ().serve(StreamableHttpClientTransport::from_config(config))
                .await?
        }
    };
    let tools = client.list_all_tools().await?;
    Ok(Connection { client, tools })
}

/// Parse a Claude Desktop / Claude Code style config: `{ "mcpServers": { name: {command,args,env} | {url,headers} } }`.
pub fn parse_config_json(json: &str) -> anyhow::Result<Vec<McpServerConfig>> {
    #[derive(Deserialize)]
    struct Root {
        #[serde(alias = "servers", alias = "mcp_servers")]
        #[serde(rename = "mcpServers")]
        mcp_servers: BTreeMap<String, Entry>,
    }
    #[derive(Deserialize)]
    struct Entry {
        command: Option<String>,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
        url: Option<String>,
        #[serde(default)]
        headers: BTreeMap<String, String>,
        #[serde(default)]
        disabled: bool,
    }
    let root: Root = serde_json::from_str(json)?;
    Ok(root
        .mcp_servers
        .into_iter()
        .map(|(name, e)| McpServerConfig {
            id: crate::db::new_id(),
            transport: if e.url.is_some() && e.command.is_none() {
                McpTransport::Http
            } else {
                McpTransport::Stdio
            },
            name,
            command: e.command,
            args: e.args,
            env: e.env,
            url: e.url,
            headers: e.headers,
            enabled: !e.disabled,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_server_names_stay_unique_and_short() {
        let a = sanitize("vmpal-e7df9524-8116-4523-9516-b0f8a7c4115e");
        let b = sanitize("vmpal-e7df9524-8116-4523-9516-aaaaaaaaaaaa");
        assert!(a.len() <= 24 && b.len() <= 24);
        assert_ne!(a, b);
        assert_eq!(sanitize("filesystem"), "filesystem");
    }

    #[test]
    fn parses_claude_desktop_config() {
        let cfg = r#"{"mcpServers":{
            "filesystem":{"command":"npx","args":["-y","@modelcontextprotocol/server-filesystem","/tmp"]},
            "remote":{"url":"https://example.com/mcp","headers":{"Authorization":"Bearer x"}}
        }}"#;
        let servers = parse_config_json(cfg).unwrap();
        assert_eq!(servers.len(), 2);
        let fs = servers.iter().find(|s| s.name == "filesystem").unwrap();
        assert_eq!(fs.transport, McpTransport::Stdio);
        assert_eq!(fs.args.len(), 3);
        assert_eq!(
            servers
                .iter()
                .find(|s| s.name == "remote")
                .unwrap()
                .transport,
            McpTransport::Http
        );
    }
}
