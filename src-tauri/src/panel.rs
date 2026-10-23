//! Showing, hiding and docking the panel window at a screen edge.

#[cfg(target_os = "linux")]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::config::Config;

const LABEL: &str = "panel";

/// Last known panel width in physical pixels. A hidden window reports a size of
/// zero, so the width is remembered here rather than read back on show.
static WIDTH: AtomicU32 = AtomicU32::new(420);

/// Writing wide: the panel takes about half the screen until the answer is done.
static WIDE: AtomicBool = AtomicBool::new(false);

pub fn init_width(width: u32) {
    WIDTH.store(width.max(1), Ordering::Relaxed);
}

/// The panel's X11 window id (0 if unknown), recorded once on the main thread.
#[cfg(target_os = "linux")]
static XID: AtomicU64 = AtomicU64::new(0);

pub fn remember_window_id(app: &AppHandle) {
    #[cfg(not(target_os = "linux"))]
    let _ = app;
    #[cfg(target_os = "linux")]
    if let Some(win) = window(app) {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(handle) = win.window_handle() {
            if let RawWindowHandle::Xlib(h) = handle.as_raw() {
                XID.store(h.window as u64, Ordering::Relaxed);
            }
        }
    }
}

/// Focus-stealing prevention can refuse to focus the panel when it opens over another
/// app, leaving it on screen but deaf to the keyboard. Taskbars activate windows with a
/// "pager" request, which window managers honour; pressing the shortcut is the same kind
/// of explicit request.
fn activate() {
    #[cfg(target_os = "linux")]
    {
        let xid = XID.load(Ordering::Relaxed) as u32;
        if xid != 0 {
            std::thread::spawn(move || {
                if let Err(e) = request_activation(xid) {
                    eprintln!("questionbar: could not ask the window manager for focus: {e}");
                }
            });
        }
    }
}

#[cfg(target_os = "linux")]
fn request_activation(xid: u32) -> Result<(), Box<dyn std::error::Error>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ClientMessageEvent, ConnectionExt, EventMask, MapState};

    const SOURCE_PAGER: u32 = 2;
    let (conn, screen) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen].root;
    let active_window = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW")?.reply()?.atom;
    // The window manager ignores the request until it has mapped the window.
    for _ in 0..50 {
        if conn.get_window_attributes(xid)?.reply()?.map_state == MapState::VIEWABLE {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let event = ClientMessageEvent::new(32, xid, active_window, [SOURCE_PAGER, x11rb::CURRENT_TIME, 0, 0, 0]);
    conn.send_event(false, root, EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY, event)?;
    conn.flush()?;
    Ok(())
}

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// The monitor chosen in Settings: "primary", "mouse" (best effort: on Wayland the
/// pointer position is only fresh if it was last over an X11 window), or a name.
fn pick_monitor(win: &WebviewWindow, choice: &str) -> Option<Monitor> {
    let chosen = match choice {
        "primary" => None,
        "mouse" => win.cursor_position().ok().and_then(|p| win.monitor_from_point(p.x, p.y).ok().flatten()),
        name => win
            .available_monitors()
            .ok()
            .and_then(|all| all.into_iter().find(|m| m.name().is_some_and(|n| n == name))),
    };
    chosen.or_else(|| win.primary_monitor().ok().flatten()).or_else(|| win.current_monitor().ok().flatten())
}

/// Docks the window to the chosen edge of the monitor's work area (which excludes
/// the GNOME top bar), keeping the remembered width.
fn dock(app: &AppHandle, win: &WebviewWindow) {
    let (side, choice) = {
        let config = app.state::<Mutex<Config>>();
        let config = config.lock().unwrap();
        (config.side.clone(), config.monitor.clone())
    };
    let Some(monitor) = pick_monitor(win, &choice) else { return };
    let area = monitor.work_area();
    let normal = WIDTH.load(Ordering::Relaxed).clamp(1, area.size.width);
    let width = if WIDE.load(Ordering::Relaxed) { (area.size.width / 2).max(normal) } else { normal };
    let x = if side == "left" { area.position.x } else { area.position.x + area.size.width as i32 - width as i32 };
    let position = PhysicalPosition::new(x, area.position.y);
    let _ = win.set_size(PhysicalSize::new(width, area.size.height));
    let _ = win.set_position(position);
    // When a move arrives together with a shrink (leaving wide mode), the window manager
    // applies the new size but can drop the move; repeat it once the resize is through.
    let win = win.clone();
    std::thread::spawn(move || {
        for _ in 0..25 {
            if win.outer_size().is_ok_and(|size| size.width == width) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let _ = win.set_position(position);
    });
}

pub fn show(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    dock(app, &win);
    let _ = win.show();
    let _ = win.unminimize();
    let _ = win.set_always_on_top(true);
    let _ = win.set_focus();
    activate();
    let _ = win.emit("panel-shown", ());
}

pub fn hide(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    remember_width(app, &win);
    // The UI saves any open draft when it receives this.
    let _ = win.emit("panel-hiding", ());
    let _ = win.hide();
}

pub fn set_wide(app: &AppHandle, on: bool) {
    let Some(win) = window(app) else { return };
    if on && !WIDE.load(Ordering::Relaxed) {
        // Keep the normal width (perhaps just resized) to come back to.
        remember_width(app, &win);
    }
    WIDE.store(on, Ordering::Relaxed);
    if win.is_visible().unwrap_or(false) {
        dock(app, &win);
    }
}

/// Moves the visible panel after its side or monitor changed in Settings.
pub fn redock(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    if win.is_visible().unwrap_or(false) {
        remember_width(app, &win);
        dock(app, &win);
    }
}

/// Hotkey behaviour: hidden -> show; focused -> hide. Visible but unfocused means
/// either you pinned it (or are editing) and clicked away, so it gets focus back, or
/// the window manager never gave it focus, so the shortcut closes it.
pub fn toggle(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    let visible = win.is_visible().unwrap_or(false);
    let focused = win.is_focused().unwrap_or(false);
    let state = app.state::<crate::PanelState>();
    let kept_open = state.pinned.load(Ordering::Relaxed) || state.busy.load(Ordering::Relaxed);
    if visible && (focused || !kept_open) {
        hide(app);
    } else {
        show(app);
    }
}

/// Records the current width (e.g. after the user dragged the edge) and persists it.
fn remember_width(app: &AppHandle, win: &WebviewWindow) {
    // The wide width is temporary; the normal one is what gets remembered.
    if WIDE.load(Ordering::Relaxed) {
        return;
    }
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
