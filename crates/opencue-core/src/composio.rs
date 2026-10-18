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

/// An app Composio can connect (Gmail, Slack, Notion…).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposioToolkit {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub logo: String,
}

/// Composio Connect: a hosted MCP server that takes a consumer key (`ck_…`) in
/// `x-consumer-api-key`. Its tools search, connect and run 500+ apps for the user's account.
pub const CONNECT_URL: &str = "https://connect.composio.dev/mcp";
/// Id of the MCP server OpenCue keeps for it (like `vmpal:<name>`, hidden from the Apps list).
pub const CONNECT_SERVER_ID: &str = "composio:connect";

pub fn is_consumer_key(key: &str) -> bool {
    key.trim().starts_with("ck_")
}

/// Popular apps offered when Composio's own catalog can't be listed (consumer keys can't).
pub fn popular_toolkits(query: &str) -> Vec<ComposioToolkit> {
    const APPS: &[(&str, &str, &str)] = &[
        ("gmail", "Gmail", "Read, send and organize email"),
        (
            "googlecalendar",
            "Google Calendar",
            "Events and invitations",
        ),
        ("googledrive", "Google Drive", "Files and folders"),
        ("googledocs", "Google Docs", "Documents"),
        ("googlesheets", "Google Sheets", "Spreadsheets"),
        ("slack", "Slack", "Messages and channels"),
        ("notion", "Notion", "Pages, databases and notes"),
        ("github", "GitHub", "Repos, issues and pull requests"),
        ("linear", "Linear", "Issues and projects"),
        ("jira", "Jira", "Tickets and boards"),
        ("trello", "Trello", "Boards, lists and cards"),
        ("asana", "Asana", "Tasks and projects"),
        ("outlook", "Outlook", "Email and calendar"),
        ("microsoft_teams", "Microsoft Teams", "Chats and meetings"),
        ("discord", "Discord", "Servers and messages"),
        ("telegram", "Telegram", "Bots and chats"),
        ("twitter", "X (Twitter)", "Posts and searches"),
        ("linkedin", "LinkedIn", "Posts and profile"),
        ("instagram", "Instagram", "Posts and insights"),
        ("youtube", "YouTube", "Videos and channels"),
        ("hubspot", "HubSpot", "CRM contacts and deals"),
        ("salesforce", "Salesforce", "CRM records"),
        ("stripe", "Stripe", "Payments and customers"),
        ("shopify", "Shopify", "Orders and products"),
        ("airtable", "Airtable", "Bases and records"),
        ("dropbox", "Dropbox", "Files"),
        ("figma", "Figma", "Designs and comments"),
        ("canva", "Canva", "Designs"),
        ("zoom", "Zoom", "Meetings"),
        ("calendly", "Calendly", "Scheduling"),
        ("clickup", "ClickUp", "Tasks"),
        ("supabase", "Supabase", "Databases"),
        ("vercel", "Vercel", "Deployments"),
        ("google_analytics", "Google Analytics", "Traffic reports"),
        (
            "google_search_console",
            "Google Search Console",
            "Search performance",
        ),
    ];
    let needle = query.trim().to_lowercase();
    APPS.iter()
        .filter(|(slug, name, _)| {
            needle.is_empty() || slug.contains(&needle) || name.to_lowercase().contains(&needle)
        })
        .map(|(slug, name, description)| ComposioToolkit {
            slug: (*slug).into(),
            name: (*name).into(),
            description: (*description).into(),
            logo: format!("https://logos.composio.dev/api/{slug}"),
        })
        .collect()
}

/// Apps the account already connected, read from Composio Connect's own tool description
/// ("User has manually connected the apps: a, b, c.").
pub fn connected_from_description(description: &str) -> Vec<String> {
    let Some(i) = description.find("connected the apps:") else {
        return vec![];
    };
    description[i + "connected the apps:".len()..]
        .split(['.', '\n'])
        .next()
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The sign-in link in the text a `COMPOSIO_MANAGE_CONNECTIONS` call returns (JSON or prose).
pub fn redirect_url_in(text: &str) -> Option<String> {
    if let Ok(v) = serde_json::from_str::<Value>(text) {
        fn find(v: &Value) -> Option<String> {
            match v {
                Value::Object(m) => m
                    .get("redirect_url")
                    .or_else(|| m.get("redirectUrl"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| m.values().find_map(find)),
                Value::Array(a) => a.iter().find_map(find),
                _ => None,
            }
        }
        if let Some(u) = find(&v) {
            return Some(u);
        }
    }
    let start = text.find("https://")?;
    let url: String = text[start..]
        .chars()
        .take_while(|c| !c.is_whitespace() && !matches!(c, '"' | '\'' | ')' | '<' | '>' | '\\'))
        .collect();
    Some(url.trim_end_matches([',', '.']).to_string())
}

/// Composio has several kinds of key and each travels in its own header: project keys (`ak_…`)
/// in `x-api-key`, user keys (`uak_…`) in `x-user-api-key` and consumer keys (`ck_…`) in
/// `x-consumer-api-key`. Sending a key in the wrong one is a 401, so the likeliest header goes
/// first and the others are tried when it is refused.
fn header_candidates(key: &str) -> &'static [&'static str] {
    const PROJECT: &[&str] = &["x-api-key", "x-consumer-api-key", "x-user-api-key"];
    const USER: &[&str] = &["x-user-api-key", "x-consumer-api-key", "x-api-key"];
    const CONSUMER: &[&str] = &["x-consumer-api-key", "x-api-key", "x-user-api-key"];
    let key = key.trim();
    if key.starts_with("uak_") {
        USER
    } else if key.starts_with("ck_") {
        CONSUMER
    } else {
        PROJECT
    }
}

pub struct Composio {
    http: reqwest::Client,
    api_key: String,
    user_id: String,
    /// The header that was accepted, once one was.
    header: std::sync::Mutex<Option<&'static str>>,
}

impl Composio {
    pub fn new(api_key: String, user_id: Option<String>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("http client"),
            api_key: api_key.trim().to_string(),
            user_id: user_id
                .filter(|u| !u.trim().is_empty())
                .unwrap_or_else(|| "default".into()),
            header: std::sync::Mutex::new(None),
        }
    }

    /// Send a request, authenticating with whichever header Composio accepts for this key.
    async fn send(
        &self,
        build: impl Fn(&reqwest::Client) -> reqwest::RequestBuilder,
    ) -> anyhow::Result<reqwest::Response> {
        let known = *self.header.lock().unwrap();
        let candidates: Vec<&'static str> = match known {
            Some(h) => vec![h],
            None => header_candidates(&self.api_key).to_vec(),
        };
        let mut last = None;
        for header in candidates {
            let res = build(&self.http)
                .header(header, &self.api_key)
                .send()
                .await?;
            if res.status() != reqwest::StatusCode::UNAUTHORIZED {
                *self.header.lock().unwrap() = Some(header);
                return Ok(res);
            }
            last = Some(res);
        }
        Ok(last.expect("at least one header was tried"))
    }

    async fn get(&self, path: &str, query: &[(&str, String)]) -> anyhow::Result<Value> {
        let res = self
            .send(|c| c.get(format!("{BASE}{path}")).query(query))
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
            if status == reqwest::StatusCode::UNAUTHORIZED {
                anyhow::bail!(
                    "Composio {status}: {msg}. Check the key: project keys start with ak_ (Platform → Settings → API Keys)."
                );
            }
            anyhow::bail!("Composio {status}: {msg}");
        }
        Ok(body)
    }

    /// The apps Composio can connect, most popular first. A query narrows them by name.
    pub async fn toolkits(
        &self,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<ComposioToolkit>> {
        let mut q = vec![("limit", limit.clamp(1, 100).to_string())];
        if !query.trim().is_empty() {
            q.push(("search", query.trim().to_string()));
        }
        let body = self.get("/toolkits", &q).await?;
        let needle = query.trim().to_lowercase();
        let mut out: Vec<ComposioToolkit> = body
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|t| ComposioToolkit {
                slug: str_at(t, "/slug"),
                name: str_at(t, "/name"),
                description: str_at(t, "/meta/description"),
                logo: str_at(t, "/meta/logo"),
            })
            .filter(|t| !t.slug.is_empty())
            .collect();
        // If the server ignored the search, narrow here.
        if !needle.is_empty() {
            out.retain(|t| {
                t.slug.to_lowercase().contains(&needle) || t.name.to_lowercase().contains(&needle)
            });
        }
        Ok(out)
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
            .send(|c| c.post(format!("{BASE}{path}")).json(&body))
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
        let payload = json!({ "user_id": self.user_id, "arguments": arguments });
        let res = self
            .send(|c| {
                c.post(format!("{BASE}/tools/execute/{}", slug.trim()))
                    .json(&payload)
            })
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
    fn picks_the_header_for_each_kind_of_key() {
        assert_eq!(super::header_candidates("ak_abc")[0], "x-api-key");
        assert_eq!(super::header_candidates("ck_abc")[0], "x-consumer-api-key");
        assert_eq!(super::header_candidates("uak_abc")[0], "x-user-api-key");
        // An unknown key still gets every header tried.
        assert!(super::header_candidates("whatever").contains(&"x-consumer-api-key"));
    }

    #[test]
    fn reads_connect_replies() {
        assert!(super::is_consumer_key(" ck_abc"));
        assert!(!super::is_consumer_key("ak_abc"));
        assert_eq!(
            super::connected_from_description(
                "Use it. User has manually connected the apps: canva, notion, ollama. Prefer these."
            ),
            vec!["canva", "notion", "ollama"]
        );
        assert_eq!(
            super::redirect_url_in(
                r#"{"data":{"results":[{"redirect_url":"https://connect.composio.dev/link/x"}]}}"#
            )
            .as_deref(),
            Some("https://connect.composio.dev/link/x")
        );
        assert_eq!(
            super::redirect_url_in("Open https://example.com/auth?a=1, then reply.").as_deref(),
            Some("https://example.com/auth?a=1")
        );
        assert!(super::popular_toolkits("sla")
            .iter()
            .any(|t| t.slug == "slack"));
    }

    #[test]
    fn classifies_actions() {
        assert!(super::looks_read_only("GMAIL_FETCH_EMAILS"));
        assert!(super::looks_read_only("GITHUB_LIST_REPOS"));
        assert!(!super::looks_read_only("GMAIL_SEND_EMAIL"));
        assert!(!super::looks_read_only("SLACK_DELETE_MESSAGE"));
    }
}
