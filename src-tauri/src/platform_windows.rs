//! Windows: a global hotkey and a "run at login" entry that the app registers itself,
//! the clipboard (Windows has no highlighted-text selection), native file dialogs, and
//! updates that install themselves.

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as _};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_updater::UpdaterExt;

use crate::config::Config;
use crate::{err, hotkey, panel, Notices, UpdateInfo, IMAGE_EXTS};

const RELEASES: &str = "https://github.com/Entropy-rgb/QuestionBar/releases";
/// Used when the chosen shortcut is taken; Windows Terminal holds Win+` while it runs.
const FALLBACK_HOTKEY: &str = "<Primary><Alt>q";

/// The shortcut registered right now (it can differ from config.json after a fallback).
static ACTIVE: Mutex<Option<String>> = Mutex::new(None);

pub fn prepare() {}

pub fn plugins(builder: tauri::Builder<Wry>) -> tauri::Builder<Wry> {
    builder
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        panel::toggle(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
}

fn register(app: &AppHandle, accel: &str) -> Result<(), String> {
    let shortcut = hotkey::to_global(accel)
        .ok_or_else(|| format!("{} can't be a shortcut on Windows.", hotkey::describe(accel)))?;
    app.global_shortcut().register(shortcut.as_str()).map_err(err)?;
    *ACTIVE.lock().unwrap() = Some(accel.to_string());
    Ok(())
}

fn unregister(app: &AppHandle, accel: &str) {
    if let Some(shortcut) = hotkey::to_global(accel) {
        let _ = app.global_shortcut().unregister(shortcut.as_str());
    }
}

/// The shortcut only lasts while QuestionBar runs, so it's registered on every start.
/// The first start of an installed build also turns on start-at-login.
pub fn start(app: &AppHandle, force_setup: bool) {
    let (chosen, setup_done) = {
        let config = app.state::<Mutex<Config>>();
        let config = config.lock().unwrap();
        (config.hotkey.clone(), config.setup_done)
    };
    if register(app, &chosen).is_err() {
        let notice = if register(app, FALLBACK_HOTKEY).is_ok() {
            if !setup_done {
                // First start: keep the shortcut that works.
                let config = app.state::<Mutex<Config>>();
                let mut config = config.lock().unwrap();
                config.hotkey = FALLBACK_HOTKEY.to_string();
                config.save();
            }
            format!(
                "Another app is using {} (Windows Terminal does while it runs), so Ctrl + Alt + Q opens QuestionBar. You can change it in Settings.",
                hotkey::describe(&chosen)
            )
        } else {
            "QuestionBar couldn't register a shortcut. Pick one in Settings.".to_string()
        };
        app.state::<Notices>().0.lock().unwrap().push(notice);
    }
    if !cfg!(debug_assertions) && (force_setup || !setup_done) {
        force_setup_now(app);
    }
}

pub fn force_setup_now(app: &AppHandle) {
    let autostart = app.state::<Mutex<Config>>().lock().unwrap().autostart;
    if autostart {
        let _ = app.autolaunch().enable();
    }
    let config = app.state::<Mutex<Config>>();
    let mut config = config.lock().unwrap();
    config.setup_done = true;
    config.save();
}

pub fn can_set_shortcut() -> bool {
    true
}

pub fn set_hotkey(app: &AppHandle, _old: &str, new: &str) -> Result<(), String> {
    let previous = ACTIVE.lock().unwrap().take();
    if let Some(previous) = &previous {
        unregister(app, previous);
    }
    register(app, new).map_err(|_| {
        if let Some(previous) = &previous {
            let _ = register(app, previous);
        }
        format!("Another app already uses {}. Pick a different shortcut.", hotkey::describe(new))
    })
}

/// Windows only says a shortcut is taken when you try to register it.
pub fn hotkey_conflicts(_hotkey: &str) -> Vec<String> {
    Vec::new()
}

pub fn set_autostart(app: &AppHandle, on: bool) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Ok(());
    }
    let launcher = app.autolaunch();
    if on { launcher.enable() } else { launcher.disable() }.map_err(err)
}

pub fn autostart_enabled(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// Windows has no "highlighted text" selection, so the quote button uses what you copied.
pub async fn read_selection(_app: &AppHandle) -> Result<String, String> {
    let text = tauri::async_runtime::spawn_blocking(|| arboard::Clipboard::new().and_then(|mut c| c.get_text()))
        .await
        .map_err(err)?;
    Ok(text.unwrap_or_default())
}

pub async fn pick_images(app: &AppHandle) -> Result<Vec<String>, String> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = app.dialog().file().set_title("Insert image").add_filter("Images", &IMAGE_EXTS);
        if let Some(win) = app.get_webview_window("panel") {
            dialog = dialog.set_parent(&win);
        }
        dialog
            .blocking_pick_files()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| p.into_path().ok())
            .filter_map(|p| p.to_str().map(String::from))
            .collect()
    })
    .await
    .map_err(err)
}

pub async fn pick_folder(app: &AppHandle, start: PathBuf) -> Result<Option<String>, String> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = app.dialog().file().set_title("Choose your notes folder").set_directory(&start);
        if let Some(win) = app.get_webview_window("panel") {
            dialog = dialog.set_parent(&win);
        }
        dialog.blocking_pick_folder().and_then(|p| p.into_path().ok()).and_then(|p| p.to_str().map(String::from))
    })
    .await
    .map_err(err)
}

pub fn system_theme(app: &AppHandle) -> &'static str {
    match app.get_webview_window("panel").and_then(|w| w.theme().ok()) {
        Some(tauri::Theme::Dark) => "dark",
        _ => "light",
    }
}

pub fn open_url(url: &str) -> Result<(), String> {
    // explorer.exe hands the URL to the default browser without a shell parsing it.
    let mut child = std::process::Command::new("explorer").arg(url).spawn().map_err(err)?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

pub async fn check_update(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    let update = app.updater().map_err(err)?.check().await.map_err(err)?;
    Ok(update.map(|u| UpdateInfo {
        notes: u.body.clone().unwrap_or_default(),
        url: format!("{RELEASES}/tag/v{}", u.version),
        version: u.version,
        can_install: true,
    }))
}

/// Downloads the update, reporting progress as `update-progress` (0–1), then installs
/// it. The installer closes QuestionBar and starts the new version.
pub async fn install_update(app: &AppHandle) -> Result<(), String> {
    let update = app.updater().map_err(err)?.check().await.map_err(err)?;
    let Some(update) = update else { return Err("QuestionBar is already up to date.".into()) };
    let progress = app.clone();
    let mut received = 0usize;
    update
        .download_and_install(
            move |chunk, total| {
                received += chunk;
                if let Some(total) = total.filter(|t| *t > 0) {
                    let _ = progress.emit("update-progress", received as f64 / total as f64);
                }
            },
            || {},
        )
        .await
        .map_err(err)?;
    app.restart();
}
