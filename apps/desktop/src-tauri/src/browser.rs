//! The built-in browser pane: a second webview inside the main window, placed by the UI over the
//! Computer panel's Browser tab and driven by the agent's browser tools through small scripts.
//! Its cookies and storage are kept apart from the app's own and from the user's browsers.

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
    shown: bool,
}

pub struct TauriBrowser {
    app: AppHandle,
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    data_dir: std::path::PathBuf,
    bounds: Mutex<Bounds>,
    loading: Arc<AtomicBool>,
    loads: Arc<AtomicU64>,
}

impl TauriBrowser {
    pub fn new(app: AppHandle, data_dir: std::path::PathBuf) -> Arc<Self> {
        Arc::new(Self {
            app,
            data_dir,
            bounds: Mutex::new(Bounds::default()),
            loading: Arc::new(AtomicBool::new(false)),
            loads: Arc::new(AtomicU64::new(0)),
        })
    }

    fn webview(&self) -> Option<Webview> {
        self.app.get_webview(LABEL)
    }

    /// The page webview, created on first use.
    fn ensure(&self, url: &str) -> anyhow::Result<Webview> {
        if let Some(w) = self.webview() {
            return Ok(w);
        }
        let window = self
            .app
            .get_window("main")
            .ok_or_else(|| anyhow::anyhow!("the app window is not open"))?;
        let url: url::Url = url.parse()?;
        let loading = self.loading.clone();
        let loads = self.loads.clone();
        let mut builder = WebviewBuilder::new(LABEL, WebviewUrl::External(url)).on_page_load(
            move |_, payload| match payload.event() {
                PageLoadEvent::Started => loading.store(true, Ordering::SeqCst),
                PageLoadEvent::Finished => {
                    loading.store(false, Ordering::SeqCst);
                    loads.fetch_add(1, Ordering::SeqCst);
                }
            },
        );
        #[cfg(target_os = "macos")]
        {
            builder = builder.data_store_identifier(*b"opencue-browser1");
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = std::fs::create_dir_all(&self.data_dir);
            builder = builder.data_directory(self.data_dir.clone());
        }
        let b = *self.bounds.lock().unwrap();
        let (w, h) = if b.w > 20.0 && b.h > 20.0 {
            (b.w, b.h)
        } else {
            (900.0, 600.0)
        };
        let view = window.add_child(
            builder,
            LogicalPosition::new(b.x, b.y),
            LogicalSize::new(w, h),
        )?;
        if !b.shown {
            let _ = view.hide();
        }
        Ok(view)
    }

    /// Where the UI has the pane: shown at these window coordinates.
    pub fn place(&self, x: f64, y: f64, w: f64, h: f64) {
        *self.bounds.lock().unwrap() = Bounds {
            x,
            y,
            w,
            h,
            shown: true,
        };
        if let Some(v) = self.webview() {
            let _ = v.set_position(LogicalPosition::new(x, y));
            let _ = v.set_size(LogicalSize::new(w.max(1.0), h.max(1.0)));
            let _ = v.show();
        }
    }

    pub fn hide(&self) {
        self.bounds.lock().unwrap().shown = false;
        if let Some(v) = self.webview() {
            let _ = v.hide();
        }
    }

    pub fn clear(&self) -> anyhow::Result<()> {
        if let Some(v) = self.webview() {
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
    async fn settle(&self, after_click: bool) {
        if after_click {
            tokio::time::sleep(Duration::from_millis(700)).await;
        }
        for _ in 0..60 {
            if !self.loading.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    /// What the toolbar shows: the current address and title.
    pub async fn state(&self) -> (String, String) {
        let Some(v) = self.webview() else {
            return (String::new(), String::new());
        };
        let js = "JSON.stringify([location.href, document.title])".to_string();
        match self.eval(&v, js).await {
            Ok(s) => serde_json::from_str::<(String, String)>(&s).unwrap_or_default(),
            Err(_) => (String::new(), String::new()),
        }
    }

    /// Toolbar buttons: back, forward, reload or go to an address.
    pub async fn control(&self, action: &str, url: Option<String>) -> anyhow::Result<()> {
        match action {
            "go" => {
                let url = checked_url(&url.unwrap_or_default())?;
                match self.webview() {
                    Some(v) => v.navigate(url)?,
                    None => {
                        self.ensure(url.as_str())?;
                    }
                }
            }
            "back" | "forward" | "reload" => {
                let v = self
                    .webview()
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
    async fn run(&self, action: &str, args: Value) -> anyhow::Result<String> {
        let view = match action {
            "open" => {
                let url = args["url"].as_str().unwrap_or_default().to_string();
                let existing = self.webview();
                match existing {
                    Some(v) => {
                        self.loading.store(true, Ordering::SeqCst);
                        v.navigate(url.parse()?)?;
                        self.settle(true).await;
                        v
                    }
                    None => {
                        let v = self.ensure(&url)?;
                        self.loading.store(true, Ordering::SeqCst);
                        self.settle(true).await;
                        v
                    }
                }
            }
            "close" => {
                if let Some(v) = self.webview() {
                    v.close()?;
                }
                return Ok("Closed.".into());
            }
            _ => self
                .webview()
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
                self.settle(true).await;
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
                    self.settle(true).await;
                }
                Ok(format!("{out} Take a snapshot to see the page now."))
            }
            _ => anyhow::bail!("unknown browser action {action}"),
        }
    }
}
