// Prevents an extra console window on Windows in release; harmless on Linux.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod hotkey;
mod panel;
mod settings;
#[cfg(target_os = "linux")]
mod setup;
mod store;
mod watcher;

// What differs between Linux and Windows: shortcuts, autostart, clipboard, dialogs, updates.
#[cfg(target_os = "linux")]
#[path = "platform_linux.rs"]
mod platform;
#[cfg(windows)]
#[path = "platform_windows.rs"]
mod platform;

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::http::{header, Request as HttpRequest, Response as HttpResponse, StatusCode};
use tauri::ipc::{InvokeBody, Request};
use tauri::{Manager, State, WindowEvent};

use config::Config;
use store::{Question, Status, Store};

/// App-wide flags shared between the window-event handler and commands.
#[derive(Default)]
pub(crate) struct PanelState {
    /// When pinned, losing focus does not hide the panel.
    pub(crate) pinned: AtomicBool,
    /// Set by the UI while an answer is being edited; suppresses hide-on-blur.
    pub(crate) busy: AtomicBool,
}

/// The notes folder, for serving images to the webview (changes when Settings switches folders).
pub(crate) struct NotesDir(pub(crate) Mutex<PathBuf>);

/// Messages for the UI to show once, e.g. that the shortcut fell back to another one.
#[derive(Default)]
pub(crate) struct Notices(pub(crate) Mutex<Vec<String>>);

/// A newer release. `can_install` is true where QuestionBar can install it itself (Windows).
#[derive(serde::Serialize)]
pub(crate) struct UpdateInfo {
    pub(crate) version: String,
    pub(crate) notes: String,
    pub(crate) url: String,
    pub(crate) can_install: bool,
}

type StoreState<'a> = State<'a, Mutex<Store>>;

pub(crate) fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
fn list_questions(store: StoreState) -> Result<Vec<Question>, String> {
    store.lock().unwrap().all().map_err(err)
}

#[tauri::command]
fn add_question(store: StoreState, text: String, quote: Option<String>) -> Result<Question, String> {
    store.lock().unwrap().add(&text, quote.as_deref().unwrap_or("")).map_err(err)
}

#[tauri::command]
fn set_answer(store: StoreState, id: String, answer: String) -> Result<Question, String> {
    store.lock().unwrap().set_answer(&id, &answer).map_err(err)
}

#[tauri::command]
fn set_status(store: StoreState, id: String, status: Status) -> Result<Question, String> {
    store.lock().unwrap().set_status(&id, status).map_err(err)
}

#[tauri::command]
fn edit_question(store: StoreState, id: String, text: String, quote: Option<String>) -> Result<Question, String> {
    store.lock().unwrap().edit(&id, &text, quote.as_deref()).map_err(err)
}

#[tauri::command]
fn delete_question(store: StoreState, id: String) -> Result<(), String> {
    store.lock().unwrap().delete(&id).map_err(err)
}

/// Saves a pasted image. The raw bytes are the request body; `x-ext` gives the file type.
#[tauri::command]
fn save_image(store: StoreState, request: Request) -> Result<String, String> {
    let InvokeBody::Raw(bytes) = request.body() else { return Err("expected raw image bytes".into()) };
    let ext = request.headers().get("x-ext").and_then(|v| v.to_str().ok()).unwrap_or("png");
    store.lock().unwrap().save_asset(bytes, ext).map_err(err)
}

pub(crate) const IMAGE_EXTS: [&str; 7] = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"];

/// Copies a dropped image file into the notes `assets/` folder.
#[tauri::command]
fn import_image(store: StoreState, path: String) -> Result<String, String> {
    let path = PathBuf::from(path);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if !IMAGE_EXTS.contains(&ext.as_str()) {
        return Err(format!("{} is not an image", path.display()));
    }
    let bytes = fs::read(&path).map_err(err)?;
    store.lock().unwrap().save_asset(&bytes, &ext).map_err(err)
}

/// Text you last highlighted (Linux) or copied, for the quote button.
#[tauri::command]
async fn read_selection(app: tauri::AppHandle) -> Result<String, String> {
    platform::read_selection(&app).await
}

/// Lets you pick image files to insert; returns their paths (empty if cancelled).
#[tauri::command]
async fn pick_images(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    platform::pick_images(&app).await
}

/// Copies built with install.sh keep the version from the repo, so they'd always look
/// out of date; they update with `git pull` instead.
pub(crate) const SOURCE_BUILD: bool = option_env!("QUESTIONBAR_SOURCE_BUILD").is_some();

#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
    if SOURCE_BUILD {
        return Ok(None);
    }
    platform::check_update(&app).await
}

#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    platform::install_update(&app).await
}

#[tauri::command]
fn take_notices(notices: State<Notices>) -> Vec<String> {
    std::mem::take(&mut *notices.0.lock().unwrap())
}

/// Lets scripted UI tests (see `QB_DEV_EVAL`) report results on stderr.
#[tauri::command]
fn dev_log(msg: String) {
    eprintln!("[ui] {msg}");
}

/// "dark" or "light", following the desktop's appearance setting.
#[tauri::command]
fn system_theme(app: tauri::AppHandle) -> &'static str {
    platform::system_theme(&app)
}

/// Opens a link from an answer in the default browser instead of inside the panel.
#[tauri::command]
fn open_external(url: String) -> Result<(), String> {
    let allowed = ["http://", "https://", "mailto:"].iter().any(|p| url.starts_with(p));
    if !allowed {
        return Err(format!("not opening {url}"));
    }
    platform::open_url(&url)
}

#[tauri::command]
fn set_pinned(state: State<PanelState>, pinned: bool) {
    state.pinned.store(pinned, Ordering::Relaxed);
}

#[tauri::command]
fn set_busy(state: State<PanelState>, busy: bool) {
    state.busy.store(busy, Ordering::Relaxed);
}

#[tauri::command]
fn hide_panel(app: tauri::AppHandle) {
    panel::hide(&app);
}

fn percent_decode(s: &str) -> String {
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(hi << 4 | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Serves `notes://localhost/assets/x.png` from the notes folder, so answers can show images.
fn serve_note_file(dir: &Path, request: &HttpRequest<Vec<u8>>) -> HttpResponse<Vec<u8>> {
    let rel = percent_decode(request.uri().path().trim_start_matches('/'));
    let rel = Path::new(&rel);
    let respond = |status: StatusCode, mime: &str, body: Vec<u8>| {
        HttpResponse::builder().status(status).header(header::CONTENT_TYPE, mime).body(body).unwrap()
    };
    if rel.as_os_str().is_empty() || rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return respond(StatusCode::FORBIDDEN, "text/plain", Vec::new());
    }
    let mime = match rel.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    };
    match fs::read(dir.join(rel)) {
        Ok(bytes) => respond(StatusCode::OK, mime, bytes),
        Err(_) => respond(StatusCode::NOT_FOUND, "text/plain", Vec::new()),
    }
}

fn main() {
    platform::prepare();

    let start_hidden = std::env::args().any(|a| a == "--hidden");
    // `--quit` with nothing running: nothing to do (and don't open a window).
    let quit_only = std::env::args().any(|a| a == "--quit");
    let force_setup = std::env::args().any(|a| a == "--setup");
    let config = Config::load();
    panel::init_width(config.width);
    let store = Store::new(config.notes_dir.clone())
        .unwrap_or_else(|e| panic!("cannot create notes folder {}: {e}", config.notes_dir.display()));
    let notes_dir = store.dir().to_path_buf();
    let own_writes = store.own_writes();

    let builder = tauri::Builder::default()
        // A second launch (e.g. from the GNOME shortcut) forwards its args here and exits.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if argv.iter().any(|a| a == "--quit") {
                // Hiding makes the UI save an open draft; give it a moment before exiting.
                panel::hide(app);
                let app = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(400));
                    app.exit(0);
                });
            } else if argv.iter().any(|a| a == "--toggle") {
                panel::toggle(app);
            } else if argv.iter().any(|a| a == "--setup") {
                platform::force_setup_now(app);
            } else if !argv.iter().any(|a| a == "--hidden") {
                panel::show(app);
            }
        }));
    platform::plugins(builder)
        .manage(PanelState::default())
        .manage(Notices::default())
        .manage(Mutex::new(config))
        .manage(Mutex::new(store))
        .manage(NotesDir(Mutex::new(notes_dir.clone())))
        .register_uri_scheme_protocol("notes", |ctx, request| {
            let dir = ctx.app_handle().state::<NotesDir>().0.lock().unwrap().clone();
            serve_note_file(&dir, &request)
        })
        .invoke_handler(tauri::generate_handler![
            list_questions,
            add_question,
            set_answer,
            set_status,
            edit_question,
            delete_question,
            save_image,
            import_image,
            read_selection,
            pick_images,
            settings::get_settings,
            settings::update_settings,
            settings::save_ui_settings,
            settings::hotkey_conflicts,
            settings::set_hotkey,
            settings::set_autostart,
            settings::pick_folder,
            settings::set_notes_dir,
            settings::add_welcome,
            settings::set_check_updates,
            check_update,
            install_update,
            take_notices,
            dev_log,
            system_theme,
            open_external,
            set_pinned,
            set_busy,
            hide_panel,
        ])
        .setup(move |app| {
            if quit_only {
                app.handle().exit(0);
                return Ok(());
            }
            // The shortcut and start-on-login (set up once; on Windows the shortcut every start).
            platform::start(app.handle(), force_setup);
            // A brand-new notes folder gets the welcome topic, once.
            {
                let config = app.state::<Mutex<Config>>();
                let mut config = config.lock().unwrap();
                if !config.welcome_done {
                    let store = app.state::<Mutex<Store>>();
                    let store = store.lock().unwrap();
                    if store.all().is_ok_and(|questions| questions.is_empty()) {
                        if let Err(e) = store.add_welcome() {
                            eprintln!("questionbar: could not add the welcome topic: {e}");
                        }
                    }
                    config.welcome_done = true;
                    config.save();
                }
            }
            let watcher = watcher::spawn(app.handle().clone(), &notes_dir, own_writes)
                .map_err(|e| eprintln!("questionbar: not watching {} for changes: {e}", notes_dir.display()))
                .ok();
            app.manage(Mutex::new(watcher));
            panel::remember_window_id(app.handle());
            if !start_hidden {
                panel::show(app.handle());
            }
            // Debug builds only: run a JS file in the panel once it has loaded, for scripted UI tests.
            #[cfg(debug_assertions)]
            if let Some(path) = std::env::var_os("QB_DEV_EVAL") {
                let script = fs::read_to_string(&path)?;
                let win = app.get_webview_window("panel").expect("panel window");
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    let _ = win.eval(script);
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::Focused(false) => {
                let state = window.state::<PanelState>();
                if !state.pinned.load(Ordering::Relaxed) && !state.busy.load(Ordering::Relaxed) {
                    panel::hide(window.app_handle());
                }
            }
            // Closing (e.g. Alt+F4) only hides; `questionbar --quit` exits.
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                panel::hide(window.app_handle());
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running QuestionBar");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decoding() {
        assert_eq!(percent_decode("assets/a%20b.png"), "assets/a b.png");
        assert_eq!(percent_decode("bad%zz%2"), "bad%zz%2");
    }

    #[test]
    fn note_files_cannot_escape_the_notes_folder() {
        let req = HttpRequest::builder().uri("notes://localhost/../secret").body(Vec::new()).unwrap();
        assert_eq!(serve_note_file(Path::new("/tmp"), &req).status(), StatusCode::FORBIDDEN);
        let req = HttpRequest::builder().uri("notes://localhost/assets%2F..%2F..%2Fetc/passwd").body(Vec::new()).unwrap();
        assert_eq!(serve_note_file(Path::new("/tmp"), &req).status(), StatusCode::FORBIDDEN);
    }
}
