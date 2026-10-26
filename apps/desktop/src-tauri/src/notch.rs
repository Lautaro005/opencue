//! The notch: a small black window at the top centre of the screen where the agents ask for things
//! (a permission, a question) and show that they are working. On a Mac with a notch it grows out of
//! it. It is an alternative to system notifications (Settings → Notifications → Where).
//!
//! The page decides what to show and when; this side only makes the window, puts it in place and
//! says where the physical notch is.

use serde::Serialize;
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

pub const LABEL: &str = "notch";

/// What the page needs to draw itself around the hardware.
#[derive(Serialize, Default, Clone, Copy)]
pub struct Info {
    pub macos: bool,
    /// Height of the camera housing in points, 0 on a screen without one.
    pub notch_height: f64,
}

/// The window, made (hidden) the first time it is needed.
pub fn ensure(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(w) = app.get_webview_window(LABEL) {
        return Ok(w);
    }
    let w = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("OpenCue")
        .initialization_script("window.__NOTCH__ = true;")
        .inner_size(320.0, 60.0)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .focused(false)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    macos::raise(&w);
    Ok(w)
}

/// Close the window (the user went back to system notifications).
pub fn close(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.close();
    }
}

/// Where the hardware notch is.
pub fn info(app: &AppHandle) -> Info {
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        Info {
            macos: true,
            notch_height: macos::notch_height(),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Info::default()
    }
}

/// Size the window to its content, put it at the top centre of the screen the app is on, show it.
pub fn place(app: &AppHandle, width: f64, height: f64) -> Result<(), String> {
    let w = ensure(app)?;
    let monitor = app
        .get_webview_window("main")
        .and_then(|m| m.current_monitor().ok().flatten())
        .or_else(|| w.primary_monitor().ok().flatten());
    let (left, top, span) = match monitor {
        Some(m) => {
            let s = m.scale_factor();
            (
                m.position().x as f64 / s,
                m.position().y as f64 / s,
                m.size().width as f64 / s,
            )
        }
        None => (0.0, 0.0, 1440.0),
    };
    let width = width.clamp(120.0, 720.0);
    let height = height.clamp(24.0, 560.0);
    w.set_size(LogicalSize::new(width, height))
        .map_err(|e| e.to_string())?;
    w.set_position(LogicalPosition::new(left + (span - width) / 2.0, top))
        .map_err(|e| e.to_string())?;
    if !w.is_visible().unwrap_or(false) {
        // Showing must not take the keyboard from whatever the user is typing in.
        w.show().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn hide(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.hide();
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSEdgeInsets;
    use tauri::WebviewWindow;

    /// Above the menu bar (so the window can cover the notch's strip), on every Space and over
    /// full-screen apps, without a shadow.
    pub fn raise(w: &WebviewWindow) {
        let Ok(ptr) = w.ns_window() else {
            return;
        };
        let ptr = ptr as *mut AnyObject;
        if ptr.is_null() {
            return;
        }
        // SAFETY: the pointer is the live NSWindow of this webview window, used right away.
        unsafe {
            let win: &AnyObject = &*ptr;
            // NSStatusWindowLevel.
            let _: () = msg_send![win, setLevel: 25isize];
            // canJoinAllSpaces | stationary | fullScreenAuxiliary.
            let _: () = msg_send![win, setCollectionBehavior: (1usize | (1 << 4) | (1 << 8))];
            let _: () = msg_send![win, setHasShadow: false];
        }
    }

    /// The camera housing's height on the main screen: the top safe-area inset (macOS 12 and up).
    pub fn notch_height() -> f64 {
        // SAFETY: plain class and property reads on the main screen.
        unsafe {
            let cls = objc2::class!(NSScreen);
            let screen: *mut AnyObject = msg_send![cls, mainScreen];
            if screen.is_null() {
                return 0.0;
            }
            let screen: &AnyObject = &*screen;
            let ok: bool = msg_send![screen, respondsToSelector: objc2::sel!(safeAreaInsets)];
            if !ok {
                return 0.0;
            }
            let insets: NSEdgeInsets = msg_send![screen, safeAreaInsets];
            insets.top
        }
    }
}
