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

    async fn post(&self, path: &str, body: Value) -> anyhow::Result<(reqwest::StatusCode, Value)> {
        let res = self
            .http
            .post(format!("{BASE}{path}"))
            .header("x-api-key", &self.api_key)
            .json(&body)
            .send()
            .await?;
        let status = res.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok((status, Value::Null));
        }
        Ok((status, Self::json(res).await?))
    }

    /// Start connecting an app (e.g. "gmail"): finds or creates a Composio-managed auth config for
    /// the toolkit and returns the page where the user signs in.
    pub async fn connect(&self, toolkit: &str) -> anyhow::Result<String> {
        let toolkit = toolkit.trim().to_lowercase();
        anyhow::ensure!(!toolkit.is_empty(), "enter an app, e.g. gmail");
        let configs = self
            .get("/auth_configs", &[("toolkit_slug", toolkit.clone())])
            .await?;
        let existing = configs
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .map(|c| str_at(c, "/id"))
            .filter(|id| !id.is_empty());
        let auth_config = match existing {
            Some(id) => id,
            None => {
                let (_, body) = self
                    .post(
                        "/auth_configs",
                        json!({
                            "toolkit": { "slug": toolkit },
                            "auth_config": { "type": "use_composio_managed_auth" }
                        }),
                    )
                    .await?;
                let id = body
                    .pointer("/auth_config/id")
                    .or_else(|| body.get("id"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                anyhow::ensure!(!id.is_empty(), "Composio has no managed sign-in for {toolkit}; set it up in the Composio dashboard");
                id
            }
        };
        let (status, body) = self
            .post(
                "/connected_accounts/link",
                json!({ "auth_config_id": auth_config, "user_id": self.user_id }),
            )
            .await?;
        let body = if status == reqwest::StatusCode::NOT_FOUND {
            self.post(
                "/connected_accounts",
                json!({ "auth_config": { "id": auth_config }, "connection": { "user_id": self.user_id } }),
            )
            .await?
            .1
        } else {
            body
        };
        body.get("redirect_url")
            .or_else(|| body.pointer("/connectionData/val/redirectUrl"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "{toolkit} doesn't need a sign-in page; check it in the Composio dashboard"
                )
            })
    }

    /// Run an action and return its raw `data`.
    pub async fn execute_value(&self, slug: &str, arguments: Value) -> anyhow::Result<Value> {
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
        Ok(body.get("data").cloned().unwrap_or(Value::Null))
    }

    /// Run an action. Returns its `data` as pretty JSON.
    pub async fn execute(&self, slug: &str, arguments: Value) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(
            &self.execute_value(slug, arguments).await?,
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
