//! Showing, hiding and docking the panel window at the right screen edge.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::config::Config;

const LABEL: &str = "panel";

/// Last known panel width in physical pixels. A hidden window reports a size of
/// zero, so the width is remembered here rather than read back on show.
static WIDTH: AtomicU32 = AtomicU32::new(420);

pub fn init_width(width: u32) {
    WIDTH.store(width.max(1), Ordering::Relaxed);
}

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// Docks the window to the right edge of the monitor's work area (which excludes
/// the GNOME top bar), keeping the remembered width.
fn dock(win: &WebviewWindow) {
    let monitor = win.current_monitor().ok().flatten().or_else(|| win.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let area = monitor.work_area();
    let width = WIDTH.load(Ordering::Relaxed).clamp(1, area.size.width);
    let x = area.position.x + area.size.width as i32 - width as i32;
    let _ = win.set_size(PhysicalSize::new(width, area.size.height));
    let _ = win.set_position(PhysicalPosition::new(x, area.position.y));
}

pub fn show(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    dock(&win);
    let _ = win.show();
    let _ = win.unminimize();
    let _ = win.set_always_on_top(true);
    let _ = win.set_focus();
    let _ = win.emit("panel-shown", ());
}

pub fn hide(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    remember_width(app, &win);
    // The UI saves any open draft when it receives this.
    let _ = win.emit("panel-hiding", ());
    let _ = win.hide();
}

/// Hotkey behaviour: hidden -> show; visible but unfocused (pinned) -> focus; focused -> hide.
pub fn toggle(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    let visible = win.is_visible().unwrap_or(false);
    let focused = win.is_focused().unwrap_or(false);
    if !visible || !focused {
        show(app);
    } else {
        hide(app);
    }
}

/// Records the current width (e.g. after the user dragged the edge) and persists it.
fn remember_width(app: &AppHandle, win: &WebviewWindow) {
    let Ok(size) = win.outer_size() else { return };
    if size.width == 0 || !win.is_visible().unwrap_or(false) {
        return;
    }
    if WIDTH.swap(size.width, Ordering::Relaxed) != size.width {
        let config = app.state::<Mutex<Config>>();
        let mut config = config.lock().unwrap();
        config.width = size.width;
        config.save();
    }
}
