//! The app catalog (Settings → Apps → Catalog): a fixed list of MCP servers that sign in with
//! OAuth. Connecting opens the browser, the provider redirects back to a loopback page served by
//! the app, and the tokens are kept in `oauth/<app>.json` so the connection survives restarts.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rmcp::transport::auth::{
    AuthError, AuthorizationManager, AuthorizationRequest, CredentialStore, OAuthState,
    StoredCredentials,
};
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

use crate::events::CoreEvent;
use crate::models::{McpServerConfig, McpTransport};
use crate::Core;

pub struct CatalogApp {
    pub slug: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub url: &'static str,
    /// The provider only lets apps you registered yourself sign in (Google, Microsoft, GitHub): a
    /// client ID is needed, unless Composio, which has its own registered apps, signs you in.
    pub needs_client: bool,
    /// The app's name in Composio.
    pub composio: &'static str,
}

/// The MCP servers the catalog offers. Composio comes first: its hosted server signs in to the apps
/// that need an OAuth key of their own. Gmail and Drive are Google's own servers; Outlook is
/// Microsoft 365's.
pub const APPS: &[CatalogApp] = &[
    CatalogApp {
        slug: "composio",
        name: "Composio",
        description: "One sign-in for 500+ apps: Gmail, Slack, Calendar…",
        url: "https://connect.composio.dev/mcp",
        needs_client: false,
        composio: "",
    },
    CatalogApp {
        slug: "gmail",
        name: "Gmail",
        description: "Search, read and draft email",
        url: "https://gmailmcp.googleapis.com/mcp/v1",
        needs_client: true,
        composio: "gmail",
    },
    CatalogApp {
        slug: "googledrive",
        name: "Google Drive",
        description: "Search, read and upload files",
        url: "https://drivemcp.googleapis.com/mcp/v1",
        needs_client: true,
        composio: "googledrive",
    },
    CatalogApp {
        slug: "notion",
        name: "Notion",
        description: "Search and edit pages and databases",
        url: "https://mcp.notion.com/mcp",
        needs_client: false,
        composio: "notion",
    },
    CatalogApp {
        slug: "trello",
        name: "Trello",
        description: "Read and update boards and cards",
        url: "https://mcp.trello.com/v1",
        needs_client: false,
        composio: "trello",
    },
    CatalogApp {
        slug: "github",
        name: "GitHub",
        description: "Repositories, issues and pull requests",
        url: "https://api.githubcopilot.com/mcp/",
        needs_client: true,
        composio: "github",
    },
    CatalogApp {
        slug: "supabase",
        name: "Supabase",
        description: "Databases, auth and storage",
        url: "https://mcp.supabase.com/mcp",
        needs_client: false,
        composio: "supabase",
    },
    CatalogApp {
        slug: "vercel",
        name: "Vercel",
        description: "Projects and deployments",
        url: "https://mcp.vercel.com/",
        needs_client: false,
        composio: "vercel",
    },
    CatalogApp {
        slug: "outlook",
        name: "Outlook",
        description: "Mail and calendar in Microsoft 365",
        url: "https://microsoft365.mcp.claude.com/mcp",
        needs_client: true,
        composio: "outlook",
    },
];

pub const COMPOSIO: &str = "composio";

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
    fn find(v: &serde_json::Value) -> Option<String> {
        match v {
            serde_json::Value::Object(m) => m
                .get("redirect_url")
                .or_else(|| m.get("redirectUrl"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .or_else(|| m.values().find_map(find)),
            serde_json::Value::Array(a) => a.iter().find_map(find),
            _ => None,
        }
    }
    if let Some(u) = serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .as_ref()
        .and_then(find)
    {
        return Some(u);
    }
    let start = text.find("https://")?;
    let url: String = text[start..]
        .chars()
        .take_while(|c| !c.is_whitespace() && !matches!(c, '"' | '\'' | ')' | '<' | '>' | '\\'))
        .collect();
    Some(url.trim_end_matches([',', '.']).to_string())
}

pub const SERVER_PREFIX: &str = "app:";

pub fn app(slug: &str) -> Option<&'static CatalogApp> {
    APPS.iter().find(|a| a.slug == slug)
}

pub fn server_id(slug: &str) -> String {
    format!("{SERVER_PREFIX}{slug}")
}

pub fn slug_of(server_id: &str) -> Option<&str> {
    server_id.strip_prefix(SERVER_PREFIX)
}

/// The MCP server config for a catalog app (they are not kept in the database).
pub fn server_config(a: &CatalogApp) -> McpServerConfig {
    McpServerConfig {
        id: server_id(a.slug),
        name: a.slug.to_string(),
        transport: McpTransport::Http,
        command: None,
        args: vec![],
        env: Default::default(),
        url: Some(a.url.to_string()),
        headers: Default::default(),
        enabled: true,
    }
}

/// Where an app's OAuth tokens are kept.
pub fn credentials_path(dir: &std::path::Path, slug: &str) -> PathBuf {
    dir.join(format!("{slug}.json"))
}

/// Tokens in a `0600` file next to the database (not in the system keychain).
pub struct FileStore {
    pub path: PathBuf,
}

#[async_trait]
impl CredentialStore for FileStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        match std::fs::read_to_string(&self.path) {
            Ok(s) => Ok(serde_json::from_str(&s).ok()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(AuthError::InternalError(e.to_string())),
        }
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        let io = |e: std::io::Error| AuthError::InternalError(e.to_string());
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let json = serde_json::to_string(&credentials)
            .map_err(|e| AuthError::InternalError(e.to_string()))?;
        std::fs::write(&self.path, json).map_err(io)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    async fn clear(&self) -> Result<(), AuthError> {
        let _ = std::fs::remove_file(&self.path);
        Ok(())
    }
}

/// State of a catalog card: red `disconnected`, yellow `reconnect`, green `connected`.
#[derive(Debug, Clone, Serialize)]
pub struct CatalogStatus {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub state: String,
    pub error: Option<String>,
    pub tools: usize,
    /// Needs an OAuth key of your own (Composio can sign you in instead).
    pub needs_client: bool,
    /// Connected through Composio rather than directly.
    pub via_composio: bool,
}

const PAGE: &str = "<!doctype html><meta charset=utf-8><title>OpenCue</title><body style=\"font-family:system-ui;display:grid;place-items:center;height:100vh;margin:0;background:#111;color:#eee\"><div style=\"text-align:center\"><h2>{TITLE}</h2><p>{BODY}</p></div><script>setTimeout(()=>window.close(),1500)</script>";

fn page(title: &str, body: &str) -> String {
    let html = PAGE.replace("{TITLE}", title).replace("{BODY}", body);
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
        html.len()
    )
}

impl Core {
    fn oauth_dir(&self) -> PathBuf {
        self.paths().root().join("oauth")
    }

    fn catalog_credentials(&self, slug: &str) -> PathBuf {
        credentials_path(&self.oauth_dir(), slug)
    }

    /// Every catalog app with its state.
    pub async fn catalog_status(&self) -> Vec<CatalogStatus> {
        let configs: Vec<McpServerConfig> = APPS.iter().map(server_config).collect();
        let status = self.mcp.status(&configs).await;
        let through_composio = self.composio_apps(false).await;
        APPS.iter()
            .map(|a| {
                let st = status.iter().find(|s| s.id == server_id(a.slug));
                let signed_in = self.catalog_credentials(a.slug).exists();
                let ready = st.is_some_and(|s| s.state == "ready");
                let via_composio = !ready
                    && !a.composio.is_empty()
                    && through_composio
                        .iter()
                        .any(|c| c.eq_ignore_ascii_case(a.composio));
                let state = if ready || via_composio {
                    "connected"
                } else if signed_in {
                    "reconnect"
                } else {
                    "disconnected"
                };
                CatalogStatus {
                    slug: a.slug.into(),
                    name: a.name.into(),
                    description: a.description.into(),
                    state: state.into(),
                    error: st
                        .and_then(|s| s.error.clone())
                        .filter(|_| state == "reconnect"),
                    tools: st.map(|s| s.tools.len()).unwrap_or(0),
                    needs_client: a.needs_client,
                    via_composio,
                }
            })
            .collect()
    }

    /// The apps connected in the user's Composio account, read from its server's tool descriptions.
    /// `refresh` reconnects first so apps connected a moment ago show up.
    pub async fn composio_apps(&self, refresh: bool) -> Vec<String> {
        let Some(a) = app(COMPOSIO) else {
            return vec![];
        };
        let cfg = server_config(a);
        if !self.catalog_credentials(COMPOSIO).exists() {
            return vec![];
        }
        if refresh {
            self.mcp.disconnect(&cfg.id).await;
            self.mcp.connect(&cfg).await;
        }
        if !self.mcp.is_ready(&cfg.id).await {
            return vec![];
        }
        let tools = self.mcp.tools(std::slice::from_ref(&cfg)).await;
        tools
            .iter()
            .flat_map(|t| {
                connected_from_description(t.tool.description.as_deref().unwrap_or_default())
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Sign in to an app through Composio: its sign-in link, to open in the browser.
    async fn composio_sign_in(&self, a: &CatalogApp) -> anyhow::Result<String> {
        let cfg = server_config(app(COMPOSIO).expect("composio is in the catalog"));
        if !self.mcp.is_ready(&cfg.id).await {
            self.mcp.connect(&cfg).await;
        }
        let (text, is_error) = self
            .mcp
            .call(
                &cfg.id,
                "COMPOSIO_MANAGE_CONNECTIONS",
                serde_json::json!({ "toolkits": [{ "name": a.composio, "action": "add" }] }),
            )
            .await?;
        anyhow::ensure!(
            !is_error,
            "Composio said: {}",
            text.chars().take(300).collect::<String>()
        );
        redirect_url_in(&text).ok_or_else(|| {
            anyhow::anyhow!(
                "{} is already connected through Composio, or Composio gave no sign-in link",
                a.name
            )
        })
    }

    /// Composio was removed in 0.3.6: drop what it left behind (its MCP server and key).
    pub(crate) fn forget_composio(self: &Arc<Self>) {
        let core = self.clone();
        tokio::spawn(async move {
            if core
                .store
                .list_mcp_servers()
                .await
                .is_ok_and(|l| l.iter().any(|s| s.id == "composio:connect"))
            {
                core.mcp.disconnect("composio:connect").await;
                let _ = core.store.delete_mcp_server("composio:connect").await;
            }
            let _ = core.secrets.delete("composio");
        });
    }

    /// Connect the apps that were signed in before, in the background.
    pub(crate) fn start_catalog(self: &Arc<Self>) {
        let core = self.clone();
        tokio::spawn(async move {
            for a in APPS {
                if core.catalog_credentials(a.slug).exists() {
                    core.mcp.connect(&server_config(a)).await;
                }
            }
            core.emit(CoreEvent::McpChanged);
        });
    }

    /// Make sure the apps an agent chose are connected, and give back their servers.
    pub(crate) async fn catalog_servers(&self, slugs: &[String]) -> Vec<McpServerConfig> {
        let mut out = vec![];
        for slug in slugs {
            let Some(a) = app(slug) else { continue };
            if !self.catalog_credentials(a.slug).exists() {
                continue;
            }
            let cfg = server_config(a);
            if !self.mcp.is_ready(&cfg.id).await {
                self.mcp.connect(&cfg).await;
            }
            out.push(cfg);
        }
        out
    }

    /// Start signing in to an app: returns the page to open in the browser. The app waits for the
    /// provider to redirect back to a loopback port and finishes the connection by itself.
    /// `client_id` / `client_secret` are for providers that need an app registered by the user.
    pub async fn catalog_connect(
        self: &Arc<Self>,
        slug: &str,
        client_id: Option<String>,
        client_secret: Option<String>,
    ) -> anyhow::Result<String> {
        let a = app(slug).ok_or_else(|| anyhow::anyhow!("unknown app {slug}"))?;
        let id_key = format!("catalog:{slug}:client_id");
        let secret_key = format!("catalog:{slug}:client_secret");
        let own_key = client_id.as_deref().is_some_and(|s| !s.trim().is_empty())
            || self.secrets.get(&id_key).is_some();
        if a.needs_client && !own_key {
            // These providers don't let OpenCue sign in by itself: Composio's apps can.
            anyhow::ensure!(
                self.mcp.is_ready(&server_id(COMPOSIO)).await,
                "{} needs an OAuth key of its own. Connect Composio first (the first card) and it signs you in, or enter your own client ID.",
                a.name
            );
            return self.composio_sign_in(a).await;
        }
        if let Some(id) = client_id.filter(|s| !s.trim().is_empty()) {
            self.secrets.set(&id_key, id.trim())?;
            self.secrets
                .set(&secret_key, client_secret.as_deref().unwrap_or("").trim())?;
        }
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
        let port = listener.local_addr()?.port();
        let redirect = format!("http://127.0.0.1:{port}/callback");

        let mut state = OAuthState::new(a.url, None).await?;
        if let OAuthState::Unauthorized(m) = &mut state {
            m.set_credential_store(FileStore {
                path: self.catalog_credentials(slug),
            });
        }
        let mut request = AuthorizationRequest::new(redirect).with_client_name("OpenCue");
        if let Some(id) = self.secrets.get(&id_key) {
            request = request.with_preregistered_client(id);
            if let Some(secret) = self.secrets.get(&secret_key).filter(|s| !s.is_empty()) {
                request = request.with_client_secret(secret);
            }
        }
        state.start_authorization(request).await.map_err(|e| {
            anyhow::anyhow!(
                "{} did not accept the sign-in ({e}). It may need an OAuth app registered by you: enter its client ID.",
                a.name
            )
        })?;
        let url = state.get_authorization_url().await?;

        // A new attempt replaces the previous one.
        let token = CancellationToken::new();
        if let Some(old) = self
            .catalog_flows
            .lock()
            .unwrap()
            .insert(slug.to_string(), token.clone())
        {
            old.cancel();
        }
        let core = self.clone();
        let slug = slug.to_string();
        tokio::spawn(async move {
            let result = tokio::select! {
                _ = token.cancelled() => return,
                r = tokio::time::timeout(Duration::from_secs(300), core.finish_catalog(listener, port, state)) => {
                    r.unwrap_or_else(|_| Err(anyhow::anyhow!("sign-in timed out")))
                }
            };
            core.catalog_flows.lock().unwrap().remove(&slug);
            let error = match result {
                Ok(()) => {
                    if let Some(a) = app(&slug) {
                        core.mcp.disconnect(&server_id(&slug)).await;
                        core.mcp.connect(&server_config(a)).await;
                    }
                    None
                }
                Err(e) => Some(format!("{e:#}")),
            };
            core.emit(CoreEvent::CatalogChanged {
                slug,
                connected: error.is_none(),
                error,
            });
            core.emit(CoreEvent::McpChanged);
        });
        Ok(url)
    }

    async fn finish_catalog(
        &self,
        listener: tokio::net::TcpListener,
        port: u16,
        mut state: OAuthState,
    ) -> anyhow::Result<()> {
        loop {
            let (mut stream, _) = listener.accept().await?;
            let mut buf = vec![0u8; 8192];
            let n = stream.read(&mut buf).await?;
            let head = String::from_utf8_lossy(&buf[..n]).to_string();
            let path = head
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .unwrap_or("/")
                .to_string();
            if !path.starts_with("/callback") {
                let _ = stream
                    .write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .await;
                continue;
            }
            let result = state
                .handle_callback_url(&format!("http://127.0.0.1:{port}{path}"))
                .await;
            match result {
                Ok(()) => {
                    let _ = stream
                        .write_all(page("Connected", "You can go back to OpenCue.").as_bytes())
                        .await;
                    return Ok(());
                }
                Err(e) => {
                    let _ = stream
                        .write_all(
                            page("Sign-in failed", "Go back to OpenCue and try again.").as_bytes(),
                        )
                        .await;
                    anyhow::bail!("{e}");
                }
            }
        }
    }

    /// Sign out of an app: forget its tokens and disconnect it.
    pub async fn catalog_disconnect(&self, slug: &str) -> anyhow::Result<()> {
        anyhow::ensure!(app(slug).is_some(), "unknown app {slug}");
        if let Some(t) = self.catalog_flows.lock().unwrap().remove(slug) {
            t.cancel();
        }
        self.mcp.disconnect(&server_id(slug)).await;
        let _ = std::fs::remove_file(self.catalog_credentials(slug));
        self.emit(CoreEvent::McpChanged);
        Ok(())
    }
}

/// A manager that signs requests with the stored tokens, for [`crate::mcp`].
pub async fn authorized_manager(url: &str, path: PathBuf) -> anyhow::Result<AuthorizationManager> {
    let mut manager = AuthorizationManager::new(url).await?;
    manager.set_credential_store(FileStore { path });
    if !manager.initialize_from_store().await? {
        anyhow::bail!("not signed in: connect it again in Settings → Apps → Catalog");
    }
    Ok(manager)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalog_lists_the_nine_apps() {
        let slugs: Vec<_> = APPS.iter().map(|a| a.slug).collect();
        assert_eq!(
            slugs,
            [
                "composio",
                "gmail",
                "googledrive",
                "notion",
                "trello",
                "github",
                "supabase",
                "vercel",
                "outlook"
            ]
        );
        assert!(APPS.iter().all(|a| a.url.starts_with("https://")));
        assert_eq!(slug_of(&server_id("notion")), Some("notion"));
        assert_eq!(slug_of("composio:connect"), None);
        // Google, Microsoft and GitHub need a key of their own; Composio signs those in.
        let own_key: Vec<_> = APPS
            .iter()
            .filter(|a| a.needs_client)
            .map(|a| a.slug)
            .collect();
        assert_eq!(own_key, ["gmail", "googledrive", "github", "outlook"]);
        assert!(APPS
            .iter()
            .filter(|a| a.needs_client)
            .all(|a| !a.composio.is_empty()));
    }

    #[test]
    fn composio_answers_are_read() {
        assert_eq!(
            connected_from_description(
                "Manage. User has manually connected the apps: gmail, slack. More."
            ),
            ["gmail", "slack"]
        );
        assert!(connected_from_description("nothing here").is_empty());
        assert_eq!(
            redirect_url_in(
                r#"{"data":{"results":[{"redirect_url":"https://connect.composio.dev/x?a=1"}]}}"#
            )
            .as_deref(),
            Some("https://connect.composio.dev/x?a=1")
        );
        assert_eq!(
            redirect_url_in("Sign in at https://example.com/auth?x=1.").as_deref(),
            Some("https://example.com/auth?x=1")
        );
    }

    #[tokio::test]
    async fn tokens_are_kept_in_a_private_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileStore {
            path: credentials_path(dir.path(), "notion"),
        };
        assert!(store.load().await.unwrap().is_none());
        store
            .save(StoredCredentials::new(
                "client-1".into(),
                None,
                vec![],
                None,
            ))
            .await
            .unwrap();
        assert_eq!(store.load().await.unwrap().unwrap().client_id, "client-1");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&store.path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        store.clear().await.unwrap();
        assert!(store.load().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn an_app_without_tokens_is_red() {
        let dir = tempfile::tempdir().unwrap();
        let core = Core::open(crate::paths::Paths::at(dir.path()).unwrap())
            .await
            .unwrap();
        let status = core.catalog_status().await;
        assert_eq!(status.len(), 9);
        assert!(status.iter().all(|s| s.state == "disconnected"));
        // With tokens saved but no connection, it asks to connect again (yellow).
        FileStore {
            path: core.catalog_credentials("trello"),
        }
        .save(StoredCredentials::new("c".into(), None, vec![], None))
        .await
        .unwrap();
        let trello = core
            .catalog_status()
            .await
            .into_iter()
            .find(|s| s.slug == "trello")
            .unwrap();
        assert_eq!(trello.state, "reconnect");
    }
}
