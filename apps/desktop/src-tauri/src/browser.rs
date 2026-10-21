//! The built-in browser pane: a second webview inside the main window, placed by the UI over the
//! Computer panel's Browser tab and driven by the agent's browser tools through small scripts.
//! Its cookies and storage are kept apart from the app's own and from the user's browsers.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use opencue_core::browser::{checked_url, BrowserHost};
use serde_json::Value;
use tauri::webview::{PageLoadEvent, WebviewBuilder};
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Webview, WebviewUrl};

const LABEL: &str = "browser";

const SNAPSHOT_JS: &str = r#"(function(){
  const vis = e => { const r = e.getBoundingClientRect(); const s = getComputedStyle(e); return r.width > 0 && r.height > 0 && s.visibility !== 'hidden' && s.display !== 'none'; };
  document.querySelectorAll('[data-oc]').forEach(e => e.removeAttribute('data-oc'));
  const sel = 'a[href],button,input:not([type=hidden]),textarea,select,[role=button],[role=link],[role=tab],[onclick],[contenteditable=true]';
  const els = [...document.querySelectorAll(sel)].filter(vis).slice(0, 150);
  const lines = els.map((e, i) => {
    e.setAttribute('data-oc', i);
    const tag = e.tagName.toLowerCase();
    const t = (e.innerText || (e.type === 'password' ? '' : e.value) || e.getAttribute('aria-label') || e.placeholder || e.title || '').trim().replace(/\s+/g, ' ').slice(0, 80);
    const type = tag === 'input' && e.type ? '[' + e.type + ']' : '';
    const href = tag === 'a' ? ' -> ' + (e.href || '').slice(0, 100) : '';
    return '[' + i + '] ' + tag + type + ' "' + t + '"' + href;
  });
  const text = (document.body ? document.body.innerText : '').replace(/\n{3,}/g, '\n\n').slice(0, 6000);
  return JSON.stringify({ url: location.href, title: document.title, text: text, elements: lines });
})()"#;

#[derive(Default, Clone, Copy)]
struct Bounds {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    known: bool,
}

/// What one agent's page is doing.
#[derive(Default)]
struct Page {
    loading: AtomicBool,
    loads: AtomicU64,
}

/// Every agent has its own webview (its own page, cookies and storage); the UI shows one at a
/// time, the one of the agent whose Computer panel is open.
pub struct TauriBrowser {
    app: AppHandle,
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    data_dir: std::path::PathBuf,
    bounds: Mutex<Bounds>,
    /// The agent whose page is on screen right now.
    shown: Mutex<Option<String>>,
    pages: Mutex<HashMap<String, Arc<Page>>>,
}

fn label(agent_id: &str) -> String {
    let id: String = agent_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{LABEL}-{id}")
}

/// A stable 16-byte name for an agent's storage (macOS keys it by an identifier).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn store_id(agent_id: &str) -> [u8; 16] {
    let mut out = [0u8; 16];
    for (half, seed) in [0xcbf29ce484222325u64, 0x84222325cbf29ce4u64]
        .into_iter()
        .enumerate()
    {
        let mut h = seed;
        for b in agent_id.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        out[half * 8..half * 8 + 8].copy_from_slice(&h.to_le_bytes());
    }
    out
}

impl TauriBrowser {
    pub fn new(app: AppHandle, data_dir: std::path::PathBuf) -> Arc<Self> {
        Arc::new(Self {
            app,
            data_dir,
            bounds: Mutex::new(Bounds::default()),
            shown: Mutex::new(None),
            pages: Mutex::new(HashMap::new()),
        })
    }

    fn page(&self, agent_id: &str) -> Arc<Page> {
        self.pages
            .lock()
            .unwrap()
            .entry(agent_id.to_string())
            .or_default()
            .clone()
    }

    fn webview(&self, agent_id: &str) -> Option<Webview> {
        self.app.get_webview(&label(agent_id))
    }

    fn is_shown(&self, agent_id: &str) -> bool {
        self.shown.lock().unwrap().as_deref() == Some(agent_id)
    }

    /// The agent's page webview, created on first use. It is born out of sight and only appears
    /// when the UI has placed it, so it never flashes over the app while the panel opens.
    fn ensure(&self, agent_id: &str, url: &str) -> anyhow::Result<Webview> {
        if let Some(w) = self.webview(agent_id) {
            return Ok(w);
        }
        let window = self
            .app
            .get_window("main")
            .ok_or_else(|| anyhow::anyhow!("the app window is not open"))?;
        let url: url::Url = url.parse()?;
        let page = self.page(agent_id);
        let mut builder = WebviewBuilder::new(label(agent_id), WebviewUrl::External(url))
            .on_page_load(move |_, payload| match payload.event() {
                PageLoadEvent::Started => page.loading.store(true, Ordering::SeqCst),
                PageLoadEvent::Finished => {
                    page.loading.store(false, Ordering::SeqCst);
                    page.loads.fetch_add(1, Ordering::SeqCst);
                }
            });
        #[cfg(target_os = "macos")]
        {
            builder = builder.data_store_identifier(store_id(agent_id));
        }
        #[cfg(not(target_os = "macos"))]
        {
            let dir = self.data_dir.join(label(agent_id));
            let _ = std::fs::create_dir_all(&dir);
            builder = builder.data_directory(dir);
        }
        let b = *self.bounds.lock().unwrap();
        let on_screen = b.known && self.is_shown(agent_id);
        let (x, y, w, h) = if on_screen {
            (b.x, b.y, b.w, b.h)
        } else {
            (-10000.0, -10000.0, 900.0, 600.0)
        };
        let view = window.add_child(builder, LogicalPosition::new(x, y), LogicalSize::new(w, h))?;
        if !on_screen {
            let _ = view.hide();
        }
        Ok(view)
    }

    /// Show this agent's page at these window coordinates (and hide everyone else's).
    pub fn place(&self, agent_id: &str, x: f64, y: f64, w: f64, h: f64) {
        *self.bounds.lock().unwrap() = Bounds {
            x,
            y,
            w,
            h,
            known: true,
        };
        *self.shown.lock().unwrap() = Some(agent_id.to_string());
        let mine = label(agent_id);
        for (l, v) in self.app.webviews() {
            if l.starts_with(LABEL) && l != mine {
                let _ = v.hide();
            }
        }
        if let Some(v) = self.webview(agent_id) {
            let _ = v.set_position(LogicalPosition::new(x, y));
            let _ = v.set_size(LogicalSize::new(w.max(1.0), h.max(1.0)));
            let _ = v.show();
        }
    }

    pub fn hide(&self) {
        *self.shown.lock().unwrap() = None;
        for (l, v) in self.app.webviews() {
            if l.starts_with(LABEL) {
                let _ = v.hide();
            }
        }
    }

    pub fn clear(&self, agent_id: &str) -> anyhow::Result<()> {
        if let Some(v) = self.webview(agent_id) {
            v.clear_all_browsing_data()?;
        }
        Ok(())
    }

    /// Run a script in the page and read what it returns (a string).
    async fn eval(&self, view: &Webview, js: String) -> anyhow::Result<String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = Mutex::new(Some(tx));
        view.eval_with_callback(js, move |out| {
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(out);
            }
        })?;
        let raw = tokio::time::timeout(Duration::from_secs(15), rx)
            .await
            .map_err(|_| anyhow::anyhow!("the page did not answer"))??;
        // The callback gets the result as JSON: a string comes back quoted.
        Ok(serde_json::from_str::<String>(&raw).unwrap_or(raw))
    }

    /// Wait for a page that started loading to finish (and for one to start, briefly).
    async fn settle(&self, agent_id: &str, after_click: bool) {
        let page = self.page(agent_id);
        if after_click {
            tokio::time::sleep(Duration::from_millis(700)).await;
        }
        for _ in 0..60 {
            if !page.loading.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    /// What the toolbar shows: the current address and title.
    pub async fn state(&self, agent_id: &str) -> (String, String) {
        let Some(v) = self.webview(agent_id) else {
            return (String::new(), String::new());
        };
        let js = "JSON.stringify([location.href, document.title])".to_string();
        match self.eval(&v, js).await {
            Ok(s) => serde_json::from_str::<(String, String)>(&s).unwrap_or_default(),
            Err(_) => (String::new(), String::new()),
        }
    }

    /// Toolbar buttons: back, forward, reload or go to an address.
    pub async fn control(
        &self,
        agent_id: &str,
        action: &str,
        url: Option<String>,
    ) -> anyhow::Result<()> {
        match action {
            "go" => {
                let url = checked_url(&url.unwrap_or_default())?;
                match self.webview(agent_id) {
                    Some(v) => v.navigate(url)?,
                    None => {
                        self.ensure(agent_id, url.as_str())?;
                    }
                }
            }
            "back" | "forward" | "reload" => {
                let v = self
                    .webview(agent_id)
                    .ok_or_else(|| anyhow::anyhow!("no page is open"))?;
                let js = match action {
                    "back" => "history.back()",
                    "forward" => "history.forward()",
                    _ => "location.reload()",
                };
                v.eval(js)?;
            }
            _ => anyhow::bail!("unknown browser action"),
        }
        Ok(())
    }
}

fn js_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

#[async_trait]
impl BrowserHost for TauriBrowser {
    async fn run(&self, agent_id: &str, action: &str, args: Value) -> anyhow::Result<String> {
        let page = self.page(agent_id);
        let view = match action {
            "open" => {
                let url = args["url"].as_str().unwrap_or_default().to_string();
                let existing = self.webview(agent_id);
                match existing {
                    Some(v) => {
                        page.loading.store(true, Ordering::SeqCst);
                        v.navigate(url.parse()?)?;
                        self.settle(agent_id, true).await;
                        v
                    }
                    None => {
                        let v = self.ensure(agent_id, &url)?;
                        page.loading.store(true, Ordering::SeqCst);
                        self.settle(agent_id, true).await;
                        v
                    }
                }
            }
            "close" => {
                if let Some(v) = self.webview(agent_id) {
                    v.close()?;
                }
                return Ok("Closed.".into());
            }
            _ => self
                .webview(agent_id)
                .ok_or_else(|| anyhow::anyhow!("no page is open: call browser_open first"))?,
        };
        match action {
            "open" | "snapshot" => {
                let raw = self.eval(&view, SNAPSHOT_JS.to_string()).await?;
                let v: Value = serde_json::from_str(&raw)?;
                let elements = v["elements"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str())
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default();
                Ok(format!(
                    "URL: {}\nTitle: {}\n\nPage text:\n{}\n\nElements:\n{}",
                    v["url"].as_str().unwrap_or_default(),
                    v["title"].as_str().unwrap_or_default(),
                    v["text"].as_str().unwrap_or_default(),
                    if elements.is_empty() {
                        "(none)".into()
                    } else {
                        elements
                    }
                ))
            }
            "click" => {
                let i = args["index"]
                    .as_u64()
                    .ok_or_else(|| anyhow::anyhow!("`index` is required"))?;
                let js = format!(
                    "(function(){{const e=document.querySelector('[data-oc=\"{i}\"]');if(!e)return 'No element {i}: take a snapshot first.';e.scrollIntoView({{block:'center'}});if(e.focus)e.focus();e.click();return 'Clicked.';}})()"
                );
                let out = self.eval(&view, js).await?;
                self.settle(agent_id, true).await;
                Ok(format!("{out} Take a snapshot to see the page now."))
            }
            "type" => {
                let i = args["index"]
                    .as_u64()
                    .ok_or_else(|| anyhow::anyhow!("`index` is required"))?;
                let text = args["text"].as_str().unwrap_or_default();
                let submit = args["submit"].as_bool().unwrap_or(false);
                let js = format!(
                    "(function(){{const e=document.querySelector('[data-oc=\"{i}\"]');if(!e)return 'No element {i}: take a snapshot first.';if(e.type==='password')return 'Refused: type passwords yourself in the pane.';e.focus();const T={text};if(e.isContentEditable){{e.textContent=T}}else{{const p=e instanceof HTMLTextAreaElement?HTMLTextAreaElement.prototype:HTMLInputElement.prototype;Object.getOwnPropertyDescriptor(p,'value').set.call(e,T)}}e.dispatchEvent(new Event('input',{{bubbles:true}}));e.dispatchEvent(new Event('change',{{bubbles:true}}));if({submit}){{const f=e.form;if(f&&f.requestSubmit){{f.requestSubmit()}}else{{e.dispatchEvent(new KeyboardEvent('keydown',{{key:'Enter',keyCode:13,bubbles:true}}))}}}}return 'Typed.';}})()",
                    text = js_str(text),
                );
                let out = self.eval(&view, js).await?;
                if submit {
                    self.settle(agent_id, true).await;
                }
                Ok(format!("{out} Take a snapshot to see the page now."))
            }
            _ => anyhow::bail!("unknown browser action {action}"),
        }
    }
}
