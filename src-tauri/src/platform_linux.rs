//! Linux: the GNOME shortcut and XDG autostart (setup.rs), the GTK clipboard and file
//! dialogs, and update checks against GitHub (a .deb can't replace itself).

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager, Wry};

use crate::config::Config;
use crate::{err, setup, UpdateInfo, IMAGE_EXTS};

/// Redirects to the newest release's page. Unlike GitHub's API it isn't rate-limited,
/// which matters on shared networks like a campus one.
const LATEST_RELEASE: &str = "https://github.com/Entropy-rgb/QuestionBar/releases/latest";

/// Runs before Tauri starts.
pub fn prepare() {
    // The panel needs to place itself at the screen edge, which Wayland forbids.
    // XWayland allows it, so force the X11 GDK backend before GTK initialises.
    std::env::set_var("GDK_BACKEND", "x11");
    // Avoids blank WebKitGTK windows on some GPU drivers.
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

pub fn plugins(builder: tauri::Builder<Wry>) -> tauri::Builder<Wry> {
    builder
}

/// First start of an installed build (or `--setup`): bind the shortcut and start on
/// login. Later starts only repoint those if QuestionBar moved. Debug builds never do
/// this on their own, so a dev copy can't take over the real shortcut.
pub fn start(app: &AppHandle, force_setup: bool) {
    let setup_done = app.state::<Mutex<Config>>().lock().unwrap().setup_done;
    if force_setup || (!cfg!(debug_assertions) && !setup_done) {
        force_setup_now(app);
    } else if !cfg!(debug_assertions) {
        if let Ok(exe) = std::env::current_exe() {
            std::thread::spawn(move || setup::refresh(&exe));
        }
    }
}

/// Sets up the shortcut (GNOME) and autostart in the background, remembering that it's done.
pub fn force_setup_now(app: &AppHandle) {
    let Ok(exe) = std::env::current_exe() else { return };
    let (hotkey, autostart) = {
        let config = app.state::<Mutex<Config>>();
        let config = config.lock().unwrap();
        (config.hotkey.clone(), config.autostart)
    };
    let app = app.clone();
    std::thread::spawn(move || match setup::run(&exe, &hotkey, autostart) {
        Ok(()) => {
            let config = app.state::<Mutex<Config>>();
            let mut config = config.lock().unwrap();
            config.setup_done = true;
            config.save();
        }
        Err(e) => eprintln!("questionbar: shortcut/autostart setup skipped: {e}"),
    });
}

pub fn can_set_shortcut() -> bool {
    setup::is_gnome()
}

pub fn set_hotkey(_app: &AppHandle, old: &str, new: &str) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(err)?;
    setup::set_hotkey(&exe, old, new)
}

pub fn hotkey_conflicts(hotkey: &str) -> Vec<String> {
    if setup::is_gnome() {
        setup::conflicts(hotkey)
    } else {
        Vec::new()
    }
}

pub fn set_autostart(_app: &AppHandle, on: bool) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(err)?;
    setup::set_autostart(&exe, on).map_err(err)
}

pub fn autostart_enabled(_app: &AppHandle) -> bool {
    setup::autostart_enabled()
}

/// Text last highlighted in any app (the "primary" selection), falling back to
/// the clipboard. GTK clipboards can only be read on the main thread.
pub async fn read_selection(app: &AppHandle) -> Result<String, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let read = |atom: gtk::gdk::Atom| {
            gtk::Clipboard::get(&atom).wait_for_text().map(|t| t.to_string()).filter(|t| !t.trim().is_empty())
        };
        let text = read(gtk::gdk::SELECTION_PRIMARY).or_else(|| read(gtk::gdk::SELECTION_CLIPBOARD));
        let _ = tx.send(text.unwrap_or_default());
    })
    .map_err(err)?;
    tauri::async_runtime::spawn_blocking(move || rx.recv()).await.map_err(err)?.map_err(err)
}

/// Runs a GTK file chooser on the main thread and returns the chosen paths.
async fn choose(
    app: &AppHandle,
    title: &'static str,
    action: gtk::FileChooserAction,
    accept: &'static str,
    start: Option<PathBuf>,
    images: bool,
) -> Result<Vec<String>, String> {
    let window = app.get_webview_window("panel").ok_or("panel window missing")?;
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        use gtk::prelude::*;
        let parent = window.gtk_window().ok();
        let dialog = gtk::FileChooserDialog::with_buttons(
            Some(title),
            parent.as_ref(),
            action,
            &[("Cancel", gtk::ResponseType::Cancel), (accept, gtk::ResponseType::Accept)],
        );
        if images {
            dialog.set_select_multiple(true);
            let filter = gtk::FileFilter::new();
            filter.set_name(Some("Images"));
            for ext in IMAGE_EXTS {
                filter.add_pattern(&format!("*.{ext}"));
                filter.add_pattern(&format!("*.{}", ext.to_uppercase()));
            }
            dialog.add_filter(filter);
        } else {
            dialog.set_create_folders(true);
        }
        if let Some(start) = &start {
            let _ = dialog.set_current_folder(start);
        }
        let paths = if dialog.run() == gtk::ResponseType::Accept {
            dialog.filenames().into_iter().filter_map(|p| p.to_str().map(String::from)).collect()
        } else {
            Vec::new()
        };
        // SAFETY: the dialog is ours and not used after this.
        unsafe { dialog.destroy() };
        let _ = tx.send(paths);
    })
    .map_err(err)?;
    tauri::async_runtime::spawn_blocking(move || rx.recv()).await.map_err(err)?.map_err(err)
}

pub async fn pick_images(app: &AppHandle) -> Result<Vec<String>, String> {
    choose(app, "Insert image", gtk::FileChooserAction::Open, "Insert", None, true).await
}

pub async fn pick_folder(app: &AppHandle, start: PathBuf) -> Result<Option<String>, String> {
    let start = start.parent().map(PathBuf::from).unwrap_or(start);
    let picked = choose(app, "Choose your notes folder", gtk::FileChooserAction::SelectFolder, "Use this folder", Some(start), false);
    Ok(picked.await?.into_iter().next())
}

/// "dark" or "light", following GNOME's appearance setting.
pub fn system_theme(_app: &AppHandle) -> &'static str {
    let out = std::process::Command::new("gsettings").args(["get", "org.gnome.desktop.interface", "color-scheme"]).output();
    match out {
        Ok(o) if String::from_utf8_lossy(&o.stdout).contains("dark") => "dark",
        _ => "light",
    }
}

pub fn open_url(url: &str) -> Result<(), String> {
    let mut child = std::process::Command::new("xdg-open").arg(url).spawn().map_err(err)?;
    // Reap it so it doesn't linger as a zombie.
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// Asks GitHub for the latest release. Linux only shows a notice: a .deb can't replace itself.
pub async fn check_update(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    let current = app.package_info().version.clone();
    tauri::async_runtime::spawn_blocking(move || latest_release(&current)).await.map_err(err)?
}

fn latest_release(current: &semver::Version) -> Result<Option<UpdateInfo>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .max_redirects(0)
        .max_redirects_will_error(false)
        .http_status_as_error(false)
        .build()
        .into();
    let response = agent.get(LATEST_RELEASE).header("User-Agent", "QuestionBar").call().map_err(err)?;
    let location = response
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| format!("GitHub answered {} instead of pointing at a release", response.status()))?;
    let tag = location.rsplit('/').next().unwrap_or_default();
    let latest = semver::Version::parse(tag.trim_start_matches('v')).map_err(err)?;
    Ok((latest > *current).then(|| UpdateInfo {
        version: latest.to_string(),
        notes: String::new(),
        url: location.to_string(),
        can_install: false,
    }))
}

pub async fn install_update(_app: &AppHandle) -> Result<(), String> {
    Err("Download the new .deb from the release page and install it.".into())
}
