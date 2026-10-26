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

/// What one tab is doing, and the address and title it showed last (so the tab strip can name
/// tabs that are out of sight).
#[derive(Default)]
struct Page {
    loading: AtomicBool,
    loads: AtomicU64,
    last: Mutex<(String, String)>,
}

/// One agent's tabs.
#[derive(Default)]
struct Tabs {
    ids: Vec<u32>,
    active: u32,
    next: u32,
}

/// A tab as the UI's strip shows it.
#[derive(serde::Serialize)]
pub struct TabInfo {
    pub id: u32,
    pub url: String,
    pub title: String,
    pub active: bool,
}

/// Every agent has its own tabs, and every tab its own webview (own page; the cookies and
/// storage are shared by the agent's tabs and kept from everyone else's). The UI shows one at a
/// time: the active tab of the agent whose Computer panel is open.
pub struct TauriBrowser {
    app: AppHandle,
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    data_dir: std::path::PathBuf,
    bounds: Mutex<Bounds>,
    /// The agent whose page is on screen right now.
    shown: Mutex<Option<String>>,
    /// The window the page is shown in: the main one, or a floating Computer window.
    win: Mutex<String>,
    /// How far the system's own placement was off from what the UI asked for, measured once the page
    /// is on screen (logical pixels, positive moves it down).
    correction: Mutex<f64>,
    /// Bumped on every placement, so only the last one is measured.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    placed: AtomicU64,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    me: std::sync::Weak<TauriBrowser>,
    pages: Mutex<HashMap<String, Arc<Page>>>,
    tabs: Mutex<HashMap<String, Tabs>>,
}

/// The agent's name as it appears in webview labels and profile folders.
fn slug(agent_id: &str) -> String {
    agent_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn label(agent_id: &str, tab: u32) -> String {
    format!("{LABEL}-{}-{tab}", slug(agent_id))
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
        Arc::new_cyclic(|me| Self {
            me: me.clone(),
            app,
            data_dir,
            bounds: Mutex::new(Bounds::default()),
            shown: Mutex::new(None),
            win: Mutex::new("main".into()),
            correction: Mutex::new(0.0),
            placed: AtomicU64::new(0),
            pages: Mutex::new(HashMap::new()),
            tabs: Mutex::new(HashMap::new()),
        })
    }

    fn page(&self, agent_id: &str, tab: u32) -> Arc<Page> {
        self.pages
            .lock()
            .unwrap()
            .entry(label(agent_id, tab))
            .or_default()
            .clone()
    }

    /// The tab the agent is looking at, if it has any.
    fn active(&self, agent_id: &str) -> Option<u32> {
        let tabs = self.tabs.lock().unwrap();
        tabs.get(agent_id)
            .filter(|t| t.ids.contains(&t.active))
            .map(|t| t.active)
    }

    fn webview(&self, agent_id: &str) -> Option<Webview> {
        let tab = self.active(agent_id)?;
        self.app.get_webview(&label(agent_id, tab))
    }

    fn is_shown(&self, agent_id: &str) -> bool {
        self.shown.lock().unwrap().as_deref() == Some(agent_id)
    }

    /// On macOS the system places a child view from the top of the window frame, title bar
    /// included, so everything lands that much too high (over the toolbar). The difference between
    /// the window and its content is the title bar (zero in full screen); other systems need none.
    fn origin_shift(&self) -> f64 {
        #[cfg(target_os = "macos")]
        {
            let label = self.win.lock().unwrap().clone();
            if let Some(window) = self.app.get_window(&label) {
                if window.is_fullscreen().unwrap_or(false) {
                    return 0.0;
                }
                if let (Ok(outer), Ok(inner), Ok(scale)) = (
                    window.outer_size(),
                    window.inner_size(),
                    window.scale_factor(),
                ) {
                    let bar = (outer.height as f64 - inner.height as f64) / scale;
                    if (0.0..200.0).contains(&bar) {
                        return bar;
                    }
                }
            }
        }
        0.0
    }

    /// The vertical shift to add to what the UI asked for: the title-bar guess plus what was measured.
    fn shift(&self) -> f64 {
        self.origin_shift() + *self.correction.lock().unwrap()
    }

    /// A new tab (and its webview), born out of sight: it only appears when the UI has placed it,
    /// so it never flashes over the app while the panel opens. It becomes the active tab.
    fn open_tab(&self, agent_id: &str, url: &str) -> anyhow::Result<(u32, Webview)> {
        let win_label = self.win.lock().unwrap().clone();
        let window = self
            .app
            .get_window(&win_label)
            .or_else(|| self.app.get_window("main"))
            .ok_or_else(|| anyhow::anyhow!("the app window is not open"))?;
        let url: url::Url = url.parse()?;
        let tab = {
            let mut all = self.tabs.lock().unwrap();
            let t = all.entry(agent_id.to_string()).or_default();
            t.next += 1;
            t.next
        };
        let page = self.page(agent_id, tab);
        let loaded = page.clone();
        let mut builder = WebviewBuilder::new(label(agent_id, tab), WebviewUrl::External(url))
            .on_page_load(move |view, payload| match payload.event() {
                PageLoadEvent::Started => page.loading.store(true, Ordering::SeqCst),
                PageLoadEvent::Finished => {
                    page.loading.store(false, Ordering::SeqCst);
                    page.loads.fetch_add(1, Ordering::SeqCst);
                    let page = loaded.clone();
                    let _ = view.eval_with_callback(
                        "JSON.stringify([location.href, document.title])".to_string(),
                        move |out| {
                            let raw = serde_json::from_str::<String>(&out).unwrap_or(out);
                            if let Ok(pair) = serde_json::from_str::<(String, String)>(&raw) {
                                *page.last.lock().unwrap() = pair;
                            }
                        },
                    );
                }
            });
        #[cfg(target_os = "macos")]
        {
            builder = builder.data_store_identifier(store_id(agent_id));
        }
        #[cfg(not(target_os = "macos"))]
        {
            // Tabs of one agent share its profile folder.
            let dir = self.data_dir.join(format!("{LABEL}-{}", slug(agent_id)));
            let _ = std::fs::create_dir_all(&dir);
            builder = builder.data_directory(dir);
        }
        let b = *self.bounds.lock().unwrap();
        let on_screen = b.known && self.is_shown(agent_id);
        let (x, y, w, h) = if on_screen {
            (b.x, b.y + self.shift(), b.w, b.h)
        } else {
            (-10000.0, -10000.0, 900.0, 600.0)
        };
        // Hide whatever else this agent had on screen: the new tab takes its place.
        let others: Vec<u32> = self
            .tabs
            .lock()
            .unwrap()
            .get(agent_id)
            .map(|t| t.ids.clone())
            .unwrap_or_default();
        for o in others {
            if let Some(v) = self.app.get_webview(&label(agent_id, o)) {
                let _ = v.hide();
            }
        }
        let view = window.add_child(builder, LogicalPosition::new(x, y), LogicalSize::new(w, h))?;
        if !on_screen {
            let _ = view.hide();
        }
        let mut all = self.tabs.lock().unwrap();
        let t = all.entry(agent_id.to_string()).or_default();
        t.ids.push(tab);
        t.active = tab;
        Ok((tab, view))
    }

    /// The agent's current page, opening a first tab if it has none.
    fn ensure(&self, agent_id: &str, url: &str) -> anyhow::Result<Webview> {
        if let Some(w) = self.webview(agent_id) {
            return Ok(w);
        }
        Ok(self.open_tab(agent_id, url)?.1)
    }

    /// Show this agent's active tab at these coordinates of `win` (the window the UI is in), and
    /// hide everything else. A page shown in another window moves over to this one.
    pub fn place(&self, win: &str, agent_id: &str, x: f64, y: f64, w: f64, h: f64) {
        *self.bounds.lock().unwrap() = Bounds {
            x,
            y,
            w,
            h,
            known: true,
        };
        *self.shown.lock().unwrap() = Some(agent_id.to_string());
        *self.win.lock().unwrap() = win.to_string();
        let mine = self
            .active(agent_id)
            .map(|t| label(agent_id, t))
            .unwrap_or_default();
        for (l, v) in self.app.webviews() {
            if l.starts_with(LABEL) && l != mine {
                let _ = v.hide();
            }
        }
        if let Some(v) = self.webview(agent_id) {
            if v.window().label() != win {
                if let Some(target) = self.app.get_window(win) {
                    let _ = v.reparent(&target);
                }
            }
            self.apply(&v, x, y, w, h);
            let _ = v.show();
            self.measure_later(agent_id.to_string(), y);
        }
    }

    fn apply(&self, v: &Webview, x: f64, y: f64, w: f64, h: f64) {
        let _ = v.set_position(LogicalPosition::new(x, y + self.shift()));
        let _ = v.set_size(LogicalSize::new(w.max(1.0), h.max(1.0)));
    }

    /// Once the page has settled where it was put, check where it really is next to the app's own
    /// page and nudge it by the difference, so it never sits over the toolbar above it (or leaves a
    /// gap) however the system counts the title bar.
    fn measure_later(&self, agent_id: String, target_y: f64) {
        #[cfg(target_os = "macos")]
        {
            let gen = self.placed.fetch_add(1, Ordering::SeqCst) + 1;
            let Some(this) = self.me.upgrade() else {
                return;
            };
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_millis(160)).await;
                if this.placed.load(Ordering::SeqCst) != gen {
                    return;
                }
                for _ in 0..2 {
                    let Some(view) = this.webview(&agent_id) else {
                        return;
                    };
                    let win = this.win.lock().unwrap().clone();
                    let Some(ui) = this.app.get_webview(&win) else {
                        return;
                    };
                    let measured = tauri::async_runtime::spawn_blocking(move || {
                        Some((macos::top_in_window(&view)?, macos::top_in_window(&ui)?))
                    })
                    .await
                    .ok()
                    .flatten();
                    let Some((child_top, ui_top)) = measured else {
                        return;
                    };
                    // Where the page should be: the UI's top edge plus what the UI asked for.
                    let err = (ui_top + target_y) - child_top;
                    if err.abs() < 1.0 {
                        return;
                    }
                    let corr = *this.correction.lock().unwrap() + err;
                    if corr.abs() > 160.0 || this.placed.load(Ordering::SeqCst) != gen {
                        return;
                    }
                    *this.correction.lock().unwrap() = corr;
                    let b = *this.bounds.lock().unwrap();
                    if let Some(v) = this.webview(&agent_id) {
                        this.apply(&v, b.x, b.y, b.w, b.h);
                    }
                    tokio::time::sleep(Duration::from_millis(80)).await;
                }
            });
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (agent_id, target_y);
        }
    }

    /// Hide the page when `win` is the window showing it (another window's panel closing must not
    /// take it away).
    pub fn hide(&self, win: &str) {
        if *self.win.lock().unwrap() != win {
            return;
        }
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

    /// The agent's tabs, with the address and title each last showed.
    pub async fn tabs(&self, agent_id: &str) -> Vec<TabInfo> {
        // Refresh the visible one; the others keep what they showed when they were left.
        if let Some(active) = self.active(agent_id) {
            let (url, title) = self.state(agent_id).await;
            if !url.is_empty() {
                *self.page(agent_id, active).last.lock().unwrap() = (url, title);
            }
        }
        let (ids, active) = {
            let all = self.tabs.lock().unwrap();
            match all.get(agent_id) {
                Some(t) => (t.ids.clone(), t.active),
                None => return vec![],
            }
        };
        ids.into_iter()
            .map(|id| {
                let (url, title) = self.page(agent_id, id).last.lock().unwrap().clone();
                TabInfo {
                    id,
                    url,
                    title,
                    active: id == active,
                }
            })
            .collect()
    }

    /// Open a new tab (blank, or on an address) and make it the current one.
    pub fn new_tab(&self, agent_id: &str, url: Option<String>) -> anyhow::Result<u32> {
        let target = match url.filter(|u| !u.trim().is_empty()) {
            Some(u) => checked_url(&u)?.to_string(),
            None => "about:blank".to_string(),
        };
        Ok(self.open_tab(agent_id, &target)?.0)
    }

    /// Bring a tab to the front.
    pub fn select_tab(&self, agent_id: &str, tab: u32) {
        {
            let mut all = self.tabs.lock().unwrap();
            match all.get_mut(agent_id) {
                Some(t) if t.ids.contains(&tab) => t.active = tab,
                _ => return,
            }
        }
        let b = *self.bounds.lock().unwrap();
        if b.known && self.is_shown(agent_id) {
            let win = self.win.lock().unwrap().clone();
            self.place(&win, agent_id, b.x, b.y, b.w, b.h);
        }
    }

    /// Close a tab; the one next to it takes over.
    pub fn close_tab(&self, agent_id: &str, tab: u32) {
        let next = {
            let mut all = self.tabs.lock().unwrap();
            let Some(t) = all.get_mut(agent_id) else {
                return;
            };
            let Some(pos) = t.ids.iter().position(|i| *i == tab) else {
                return;
            };
            t.ids.remove(pos);
            if t.active == tab {
                t.active = t
                    .ids
                    .get(pos.min(t.ids.len().saturating_sub(1)))
                    .copied()
                    .unwrap_or(0);
            }
            t.active
        };
        if let Some(v) = self.app.get_webview(&label(agent_id, tab)) {
            let _ = v.close();
        }
        self.pages.lock().unwrap().remove(&label(agent_id, tab));
        if next != 0 {
            self.select_tab(agent_id, next);
        }
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
        let Some(tab) = self.active(agent_id) else {
            return;
        };
        let page = self.page(agent_id, tab);
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
        let view = match action {
            "open" => {
                let url = args["url"].as_str().unwrap_or_default().to_string();
                let existing = if args["new_tab"].as_bool().unwrap_or(false) {
                    None
                } else {
                    self.webview(agent_id)
                };
                let v = match existing {
                    Some(v) => {
                        if let Some(tab) = self.active(agent_id) {
                            self.page(agent_id, tab)
                                .loading
                                .store(true, Ordering::SeqCst);
                        }
                        v.navigate(url.parse()?)?;
                        v
                    }
                    None => {
                        let (tab, v) = self.open_tab(agent_id, &url)?;
                        self.page(agent_id, tab)
                            .loading
                            .store(true, Ordering::SeqCst);
                        v
                    }
                };
                self.settle(agent_id, true).await;
                v
            }
            "close" => {
                if let Some(tab) = self.active(agent_id) {
                    self.close_tab(agent_id, tab);
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

#[cfg(target_os = "macos")]
mod macos {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSRect;
    use std::time::Duration;
    use tauri::Webview;

    /// How far the top edge of a webview is below the top of its window (points, the same unit as
    /// CSS pixels), read from the native view. Blocks until the main thread answers: call it off
    /// the main thread.
    pub fn top_in_window(view: &Webview) -> Option<f64> {
        let (tx, rx) = std::sync::mpsc::channel();
        view.with_webview(move |wv| {
            let ptr = wv.inner() as *mut AnyObject;
            if ptr.is_null() {
                return;
            }
            // SAFETY: the pointer is the live WKWebView the runtime handed us, used on the main
            // thread inside this callback.
            let top = unsafe {
                let v: &AnyObject = &*ptr;
                let bounds: NSRect = msg_send![v, bounds];
                let in_window: NSRect =
                    msg_send![v, convertRect: bounds, toView: None::<&AnyObject>];
                let window: *mut AnyObject = msg_send![v, window];
                if window.is_null() {
                    return;
                }
                let frame: NSRect = msg_send![&*window, frame];
                // Window coordinates count up from the bottom edge.
                frame.size.height - (in_window.origin.y + in_window.size.height)
            };
            let _ = tx.send(top);
        })
        .ok()?;
        rx.recv_timeout(Duration::from_millis(400)).ok()
    }
}
