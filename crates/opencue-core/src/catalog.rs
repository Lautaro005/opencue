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
}

/// The MCP servers the catalog offers. Gmail, Drive and Calendar are Google's own servers;
/// Outlook is Microsoft 365's.
pub const APPS: &[CatalogApp] = &[
    CatalogApp {
        slug: "gmail",
        name: "Gmail",
        description: "Search, read and draft email",
        url: "https://gmailmcp.googleapis.com/mcp/v1",
    },
    CatalogApp {
        slug: "googledrive",
        name: "Google Drive",
        description: "Search, read and upload files",
        url: "https://drivemcp.googleapis.com/mcp/v1",
    },
    CatalogApp {
        slug: "googlecalendar",
        name: "Google Calendar",
        description: "See and manage your schedule",
        url: "https://calendarmcp.googleapis.com/mcp/v1",
    },
    CatalogApp {
        slug: "notion",
        name: "Notion",
        description: "Search and edit pages and databases",
        url: "https://mcp.notion.com/mcp",
    },
    CatalogApp {
        slug: "trello",
        name: "Trello",
        description: "Read and update boards and cards",
        url: "https://mcp.trello.com/v1",
    },
    CatalogApp {
        slug: "github",
        name: "GitHub",
        description: "Repositories, issues and pull requests",
        url: "https://api.githubcopilot.com/mcp/",
    },
    CatalogApp {
        slug: "supabase",
        name: "Supabase",
        description: "Databases, auth and storage",
        url: "https://mcp.supabase.com/mcp",
    },
    CatalogApp {
        slug: "vercel",
        name: "Vercel",
        description: "Projects and deployments",
        url: "https://mcp.vercel.com/",
    },
    CatalogApp {
        slug: "outlook",
        name: "Outlook",
        description: "Mail and calendar in Microsoft 365",
        url: "https://microsoft365.mcp.claude.com/mcp",
    },
];

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
        APPS.iter()
            .map(|a| {
                let st = status.iter().find(|s| s.id == server_id(a.slug));
                let signed_in = self.catalog_credentials(a.slug).exists();
                let ready = st.is_some_and(|s| s.state == "ready");
                let state = if ready {
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
                }
            })
            .collect()
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
                "gmail",
                "googledrive",
                "googlecalendar",
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
