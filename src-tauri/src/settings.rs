//! Commands behind the Settings page. Debug builds save the choices but never touch
//! the real desktop shortcut or autostart, so a dev copy can't take them over.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use notify::RecommendedWatcher;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::config::Config;
use crate::store::Store;
use crate::{err, hotkey, panel, platform, watcher, NotesDir};

#[derive(Serialize)]
pub struct MonitorView {
    name: String,
    label: String,
    primary: bool,
}

#[derive(Serialize)]
pub struct SettingsView {
    notes_dir: String,
    hotkey: String,
    autostart: bool,
    side: String,
    monitor: String,
    theme: String,
    zoom: f32,
    compact: bool,
    /// Whether QuestionBar can set the shortcut itself (GNOME and Windows).
    can_set_shortcut: bool,
    /// "linux" or "windows", for labels like Super vs Win.
    platform: &'static str,
    check_updates: bool,
    /// Built with install.sh: updates come from `git pull`, not from releases.
    source_build: bool,
    monitors: Vec<MonitorView>,
    version: String,
}

fn monitors(app: &AppHandle) -> Vec<MonitorView> {
    let Some(win) = app.get_webview_window("panel") else { return Vec::new() };
    let primary = win.primary_monitor().ok().flatten().and_then(|m| m.name().cloned());
    win.available_monitors()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| {
            let name = m.name()?.clone();
            let size = m.size();
            Some(MonitorView {
                label: format!("{name} ({}×{})", size.width, size.height),
                primary: primary.as_deref() == Some(name.as_str()),
                name,
            })
        })
        .collect()
}

#[tauri::command]
pub fn get_settings(app: AppHandle, config: State<Mutex<Config>>) -> SettingsView {
    let c = config.lock().unwrap().clone();
    SettingsView {
        notes_dir: c.notes_dir.display().to_string(),
        autostart: if cfg!(debug_assertions) { c.autostart } else { platform::autostart_enabled(&app) },
        hotkey: c.hotkey,
        side: c.side,
        monitor: c.monitor,
        theme: c.theme,
        zoom: c.zoom,
        compact: c.compact,
        can_set_shortcut: platform::can_set_shortcut(),
        platform: if cfg!(windows) { "windows" } else { "linux" },
        check_updates: c.check_updates,
        source_build: crate::SOURCE_BUILD,
        monitors: monitors(&app),
        version: app.package_info().version.to_string(),
    }
}

/// Changes any of side ("left"/"right"), monitor and theme ("system"/"light"/"dark").
#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    config: State<Mutex<Config>>,
    side: Option<String>,
    monitor: Option<String>,
    theme: Option<String>,
) -> Result<(), String> {
    {
        let mut c = config.lock().unwrap();
        if let Some(side) = side {
            if side != "left" && side != "right" {
                return Err(format!("unknown side {side}"));
            }
            c.side = side;
        }
        if let Some(monitor) = monitor {
            c.monitor = monitor;
        }
        if let Some(theme) = theme {
            if !["system", "light", "dark"].contains(&theme.as_str()) {
                return Err(format!("unknown theme {theme}"));
            }
            c.theme = theme;
        }
        c.save();
    }
    panel::redock(&app);
    Ok(())
}

#[tauri::command]
pub fn save_ui_settings(config: State<Mutex<Config>>, zoom: f32, compact: bool) {
    let mut config = config.lock().unwrap();
    if config.zoom != zoom || config.compact != compact {
        config.zoom = zoom;
        config.compact = compact;
        config.save();
    }
}

/// Names of desktop shortcuts that already use `hotkey` (GNOME can tell; Windows can't).
#[tauri::command]
pub async fn hotkey_conflicts(hotkey: String) -> Vec<String> {
    platform::hotkey_conflicts(&hotkey)
}

#[tauri::command]
pub async fn set_hotkey(app: AppHandle, hotkey: String) -> Result<(), String> {
    if !hotkey::valid(&hotkey) {
        return Err("Pick at least one modifier key, or use a function key.".into());
    }
    let old = app.state::<Mutex<Config>>().lock().unwrap().hotkey.clone();
    platform::set_hotkey(&app, &old, &hotkey)?;
    let config = app.state::<Mutex<Config>>();
    let mut config = config.lock().unwrap();
    config.hotkey = hotkey;
    config.save();
    Ok(())
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, config: State<Mutex<Config>>, on: bool) -> Result<(), String> {
    platform::set_autostart(&app, on)?;
    let mut config = config.lock().unwrap();
    config.autostart = on;
    config.save();
    Ok(())
}

/// Lets the user pick a folder; None if they cancel.
#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    let start = app.state::<Mutex<Config>>().lock().unwrap().notes_dir.clone();
    platform::pick_folder(&app, start).await
}

#[tauri::command]
pub fn set_check_updates(config: State<Mutex<Config>>, on: bool) {
    let mut config = config.lock().unwrap();
    config.check_updates = on;
    config.save();
}

/// Switches to another notes folder, first moving the questions there if asked.
/// Returns how many questions moved.
#[tauri::command]
pub async fn set_notes_dir(app: AppHandle, path: String, move_notes: bool) -> Result<usize, String> {
    let new_dir = PathBuf::from(&path);
    if !new_dir.is_absolute() {
        return Err(format!("{path} is not a full folder path"));
    }
    let store = app.state::<Mutex<Store>>();
    let old_dir = store.lock().unwrap().dir().to_path_buf();
    if fs::canonicalize(&old_dir).ok() == fs::canonicalize(&new_dir).ok() {
        return Ok(0);
    }
    let moved = if move_notes { store.lock().unwrap().move_all_to(&new_dir).map_err(err)? } else { 0 };

    let new_store = Store::new(new_dir.clone()).map_err(err)?;
    let own_writes = new_store.own_writes();
    *store.lock().unwrap() = new_store;
    *app.state::<NotesDir>().0.lock().unwrap() = new_dir.clone();
    // Replacing the watcher drops the old one, which stops it.
    let watcher = watcher::spawn(app.clone(), &new_dir, own_writes).ok();
    *app.state::<Mutex<Option<RecommendedWatcher>>>().lock().unwrap() = watcher;
    {
        let config = app.state::<Mutex<Config>>();
        let mut config = config.lock().unwrap();
        config.notes_dir = new_dir;
        config.save();
    }
    let _ = app.emit("notes-changed", ());
    Ok(moved)
}

#[tauri::command]
pub fn add_welcome(store: State<Mutex<Store>>) -> Result<(), String> {
    store.lock().unwrap().add_welcome().map_err(err)
}
