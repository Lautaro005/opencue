//! The `opencue-view` scheme: pages the app shows in sandboxed frames, an HTML artifact or the
//! dashboard. They are served with their own Content-Security-Policy, so a page an agent wrote
//! can run scripts and styles of its own but cannot reach the network (beyond three public
//! script CDNs) or the app. The frame itself is sandboxed without access to the app's origin.

use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext};

use crate::AppState;

pub const SCHEME: &str = "opencue-view";

const CSP: &str = "default-src 'none'; \
    script-src 'unsafe-inline' 'unsafe-eval' https://cdnjs.cloudflare.com https://cdn.jsdelivr.net https://unpkg.com; \
    style-src 'unsafe-inline' https://cdnjs.cloudflare.com https://cdn.jsdelivr.net https://fonts.googleapis.com; \
    font-src data: https://fonts.gstatic.com https://cdnjs.cloudflare.com https://cdn.jsdelivr.net; \
    img-src data: blob:; media-src data: blob:; connect-src 'none'; frame-src 'none'; \
    form-action 'none'; base-uri 'none'";

/// Where the frames load from, which differs by system.
pub fn base() -> String {
    if cfg!(any(windows, target_os = "android")) {
        format!("http://{SCHEME}.localhost")
    } else {
        format!("{SCHEME}://localhost")
    }
}

fn reply(status: StatusCode, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header("Content-Security-Policy", CSP)
        .header(header::CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

/// `/artifact/<artifact id>/<version>` and `/dashboard`.
pub fn handle<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let Some(state) = ctx.app_handle().try_state::<AppState>() else {
        return reply(
            StatusCode::SERVICE_UNAVAILABLE,
            b"The app is starting.".to_vec(),
        );
    };
    let core = &state.core;
    let path = request.uri().path().trim_matches('/').to_string();
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        ["dashboard"] => reply(StatusCode::OK, core.dashboard_html().into_bytes()),
        ["artifact", id, version] => {
            let found = core.get_artifact(id).ok().and_then(|a| {
                let n: u32 = version.parse().ok()?;
                (a.kind == "html")
                    .then(|| a.versions.into_iter().find(|v| v.version == n))
                    .flatten()
            });
            match found.and_then(|v| {
                std::fs::read(
                    core.paths()
                        .attachment_file(&v.attachment.id, &v.attachment.name),
                )
                .ok()
            }) {
                Some(bytes) => reply(StatusCode::OK, bytes),
                None => reply(StatusCode::NOT_FOUND, b"Not found.".to_vec()),
            }
        }
        _ => reply(StatusCode::NOT_FOUND, b"Not found.".to_vec()),
    }
}
