//! VMPal machines. Each one is an MCP server started by the VMPal app, declared in TOML the same
//! way Codex declares MCP servers:
//!
//! ```toml
//! [mcp_servers.vmpal-office]
//! command = "/Applications/VMPal.app/Contents/Helpers/VMPalMachine.app/Contents/MacOS/VMPalMachine"
//! args = ["--mcp", "E7D-…"]
//! ```
//!
//! The TOML lives in Settings → Computers. Every machine is mirrored as an MCP server with the id
//! `vmpal:<name>`; agents only get the machines they picked, and the server starts on first use.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::events::CoreEvent;
use crate::models::{McpServerConfig, McpServerStatus, McpTransport};
use crate::Core;

pub const SETTING: &str = "vmpal_toml";
pub const ID_PREFIX: &str = "vmpal:";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VmPalMachine {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

pub fn id_for(name: &str) -> String {
    format!("{ID_PREFIX}{name}")
}

pub fn is_vmpal_id(id: &str) -> bool {
    id.starts_with(ID_PREFIX)
}

/// Read `[mcp_servers.<name>]` tables. Empty text means no machines.
pub fn parse(text: &str) -> anyhow::Result<Vec<VmPalMachine>> {
    if text.trim().is_empty() {
        return Ok(vec![]);
    }
    let doc: toml::Table =
        toml::from_str(text).map_err(|e| anyhow::anyhow!("that is not valid TOML: {e}"))?;
    let servers = match doc.get("mcp_servers").or_else(|| doc.get("mcpServers")) {
        Some(toml::Value::Table(t)) => t,
        _ => anyhow::bail!("expected one or more [mcp_servers.<name>] tables"),
    };
    let mut out = Vec::new();
    for (name, value) in servers {
        let toml::Value::Table(t) = value else {
            anyhow::bail!("[mcp_servers.{name}] must be a table");
        };
        let command = t
            .get("command")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .ok_or_else(|| anyhow::anyhow!("[mcp_servers.{name}] needs a `command`"))?
            .to_string();
        let args = match t.get("args") {
            None => vec![],
            Some(toml::Value::Array(a)) => a
                .iter()
                .map(|v| {
                    v.as_str().map(str::to_string).ok_or_else(|| {
                        anyhow::anyhow!("`args` of {name} must be a list of strings")
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?,
            Some(_) => anyhow::bail!("`args` of {name} must be a list of strings"),
        };
        let env = match t.get("env") {
            None => BTreeMap::new(),
            Some(toml::Value::Table(e)) => e
                .iter()
                .map(|(k, v)| {
                    v.as_str()
                        .map(|s| (k.clone(), s.to_string()))
                        .ok_or_else(|| anyhow::anyhow!("`env.{k}` of {name} must be a string"))
                })
                .collect::<anyhow::Result<_>>()?,
            Some(_) => anyhow::bail!("`env` of {name} must be a table"),
        };
        out.push(VmPalMachine {
            name: name.clone(),
            command,
            args,
            env,
        });
    }
    Ok(out)
}

impl Core {
    pub async fn vmpal_toml(&self) -> String {
        self.store
            .setting(SETTING)
            .await
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    pub async fn vmpal_machines(&self) -> Vec<VmPalMachine> {
        parse(&self.vmpal_toml().await).unwrap_or_default()
    }

    /// Replace the TOML, then mirror the machines as MCP servers (removed ones disappear).
    pub async fn save_vmpal(&self, text: &str) -> anyhow::Result<Vec<VmPalMachine>> {
        let machines = parse(text)?;
        self.store.set_setting(SETTING, text.trim()).await?;
        let existing = self.store.list_mcp_servers().await?;
        let keep: HashSet<String> = machines.iter().map(|m| id_for(&m.name)).collect();
        for old in existing
            .iter()
            .filter(|s| is_vmpal_id(&s.id) && !keep.contains(&s.id))
        {
            self.mcp.disconnect(&old.id).await;
            self.store.delete_mcp_server(&old.id).await?;
        }
        for m in &machines {
            let cfg = McpServerConfig {
                id: id_for(&m.name),
                name: m.name.clone(),
                transport: McpTransport::Stdio,
                command: Some(m.command.clone()),
                args: m.args.clone(),
                env: m.env.clone(),
                url: None,
                headers: BTreeMap::new(),
                enabled: true,
            };
            let unchanged = existing.iter().any(|s| {
                s.id == cfg.id && s.command == cfg.command && s.args == cfg.args && s.env == cfg.env
            });
            self.store.upsert_mcp_server(&cfg).await?;
            if !unchanged {
                self.mcp.disconnect(&cfg.id).await;
            }
        }
        self.emit(CoreEvent::McpChanged);
        Ok(machines)
    }

    /// Append machines pasted from an agent's page to the shared list.
    pub async fn add_vmpal(&self, snippet: &str) -> anyhow::Result<Vec<VmPalMachine>> {
        let added = parse(snippet)?;
        anyhow::ensure!(
            !added.is_empty(),
            "paste at least one [mcp_servers.<name>] table"
        );
        let current = self.vmpal_toml().await;
        let names: HashSet<String> = parse(&current)?.into_iter().map(|m| m.name).collect();
        if let Some(dup) = added.iter().find(|m| names.contains(&m.name)) {
            anyhow::bail!("a machine named {} already exists", dup.name);
        }
        let combined = if current.trim().is_empty() {
            snippet.trim().to_string()
        } else {
            format!("{}\n\n{}", current.trim_end(), snippet.trim())
        };
        self.save_vmpal(&combined).await?;
        Ok(added)
    }

    /// Start a machine's MCP server now and report what it exposes.
    pub async fn test_vmpal(self: &Arc<Self>, name: &str) -> anyhow::Result<McpServerStatus> {
        let id = id_for(name);
        let server = self
            .store
            .list_mcp_servers()
            .await?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| anyhow::anyhow!("no machine named {name}"))?;
        self.mcp.disconnect(&id).await;
        self.mcp.connect(&server).await;
        self.emit(CoreEvent::McpChanged);
        let status = self
            .mcp
            .status(std::slice::from_ref(&server))
            .await
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("no status"))?;
        Ok(status)
    }

    /// Start the machines an agent picked, if they are not running yet.
    pub(crate) async fn ensure_vmpal(&self, agent: &Agent) {
        if agent.settings.vmpal.is_empty() {
            return;
        }
        let Ok(servers) = self.store.list_mcp_servers().await else {
            return;
        };
        for s in servers.into_iter().filter(|s| {
            is_vmpal_id(&s.id) && agent.settings.vmpal.iter().any(|n| id_for(n) == s.id)
        }) {
            if !self.mcp.is_ready(&s.id).await {
                self.mcp.connect(&s).await;
            }
        }
    }
}

use crate::models::Agent;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codex_style_tables() {
        let m = parse(
            r#"
[mcp_servers.vmpal-one]
command = "/Applications/VMPal.app/Contents/Helpers/VMPalMachine.app/Contents/MacOS/VMPalMachine"
args = ["--mcp", "E7D-abc"]

[mcp_servers.vmpal-two]
command = "vmpal"
env = { TOKEN = "x" }
"#,
        )
        .unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].name, "vmpal-one");
        assert_eq!(m[0].args, vec!["--mcp", "E7D-abc"]);
        assert_eq!(m[1].env["TOKEN"], "x");
    }

    #[test]
    fn rejects_missing_command_and_bad_toml() {
        assert!(parse("[mcp_servers.x]\nargs = []").is_err());
        assert!(parse("not = [valid").is_err());
        assert!(parse("").unwrap().is_empty());
    }
}
