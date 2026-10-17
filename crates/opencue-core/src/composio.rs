//! Composio (https://composio.dev): 1000+ app integrations behind one API key. Accounts are
//! connected in the Composio dashboard; agents search for actions and run them by slug.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const BASE: &str = "https://backend.composio.dev/api/v3";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposioTool {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub toolkit: String,
    #[serde(default)]
    pub input_parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposioAccount {
    pub id: String,
    pub toolkit: String,
    pub status: String,
}

pub struct Composio {
    http: reqwest::Client,
    api_key: String,
    user_id: String,
}

impl Composio {
    pub fn new(api_key: String, user_id: Option<String>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("http client"),
            api_key,
            user_id: user_id
                .filter(|u| !u.trim().is_empty())
                .unwrap_or_else(|| "default".into()),
        }
    }

    async fn get(&self, path: &str, query: &[(&str, String)]) -> anyhow::Result<Value> {
        let res = self
            .http
            .get(format!("{BASE}{path}"))
            .header("x-api-key", &self.api_key)
            .query(query)
            .send()
            .await?;
        Self::json(res).await
    }

    async fn json(res: reqwest::Response) -> anyhow::Result<Value> {
        let status = res.status();
        let body: Value = res.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            let msg = body
                .pointer("/error/message")
                .or_else(|| body.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("request failed");
            anyhow::bail!("Composio {status}: {msg}");
        }
        Ok(body)
    }

    pub async fn search_tools(
        &self,
        query: &str,
        toolkit: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<ComposioTool>> {
        let mut q = vec![("limit", limit.clamp(1, 50).to_string())];
        if !query.trim().is_empty() {
            q.push(("query", query.trim().to_string()));
        }
        if let Some(t) = toolkit.filter(|t| !t.trim().is_empty()) {
            q.push(("toolkit_slug", t.trim().to_lowercase()));
        }
        let body = self.get("/tools", &q).await?;
        Ok(body
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|t| ComposioTool {
                slug: str_at(t, "/slug"),
                name: str_at(t, "/name"),
                description: str_at(t, "/description"),
                toolkit: str_at(t, "/toolkit/slug"),
                input_parameters: t.get("input_parameters").cloned().unwrap_or(Value::Null),
            })
            .collect())
    }

    pub async fn accounts(&self) -> anyhow::Result<Vec<ComposioAccount>> {
        let body = self
            .get(
                "/connected_accounts",
                &[("user_ids", self.user_id.clone()), ("limit", "100".into())],
            )
            .await?;
        Ok(body
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|a| ComposioAccount {
                id: str_at(a, "/id"),
                toolkit: str_at(a, "/toolkit/slug"),
                status: str_at(a, "/status"),
            })
            .collect())
    }

    /// Run an action. Returns its `data` as pretty JSON.
    pub async fn execute(&self, slug: &str, arguments: Value) -> anyhow::Result<String> {
        let res = self
            .http
            .post(format!("{BASE}/tools/execute/{}", slug.trim()))
            .header("x-api-key", &self.api_key)
            .json(&json!({ "user_id": self.user_id, "arguments": arguments }))
            .send()
            .await?;
        let body = Self::json(res).await?;
        if body.get("successful").and_then(Value::as_bool) == Some(false) {
            anyhow::bail!(
                "{}",
                body.get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("the action failed")
            );
        }
        Ok(serde_json::to_string_pretty(
            body.get("data").unwrap_or(&Value::Null),
        )?)
    }
}

fn str_at(v: &Value, ptr: &str) -> String {
    v.pointer(ptr)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Read-only actions don't need approval; everything else might change data.
pub fn looks_read_only(slug: &str) -> bool {
    let s = slug.to_uppercase();
    [
        "_GET",
        "_LIST",
        "_SEARCH",
        "_FETCH",
        "_FIND",
        "_READ",
        "_RETRIEVE",
    ]
    .iter()
    .any(|k| s.contains(k))
}

#[cfg(test)]
mod tests {
    #[test]
    fn classifies_actions() {
        assert!(super::looks_read_only("GMAIL_FETCH_EMAILS"));
        assert!(super::looks_read_only("GITHUB_LIST_REPOS"));
        assert!(!super::looks_read_only("GMAIL_SEND_EMAIL"));
        assert!(!super::looks_read_only("SLACK_DELETE_MESSAGE"));
    }
}
