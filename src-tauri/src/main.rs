// Prevents an extra console window on Windows in release; harmless on Linux.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod panel;
mod setup;
mod store;
mod watcher;

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
struct PanelState {
    /// When pinned, losing focus does not hide the panel.
    pinned: AtomicBool,
    /// Set by the UI while an answer is being edited; suppresses hide-on-blur.
    busy: AtomicBool,
}

/// The notes folder, for serving images to the webview.
struct NotesDir(PathBuf);

type StoreState<'a> = State<'a, Mutex<Store>>;

fn err(e: impl std::fmt::Display) -> String {
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

const IMAGE_EXTS: [&str; 7] = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"];

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

/// Text last highlighted in any app (the "primary" selection), falling back to
/// the clipboard. GTK clipboards can only be read on the main thread.
#[tauri::command]
async fn read_selection(app: tauri::AppHandle) -> Result<String, String> {
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

/// Lets the user pick image files to insert; returns their paths (empty if cancelled).
#[tauri::command]
async fn pick_images(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let window = app.get_webview_window("panel").ok_or("panel window missing")?;
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        use gtk::prelude::*;
        let parent = window.gtk_window().ok();
        let dialog = gtk::FileChooserDialog::with_buttons(
            Some("Insert image"),
            parent.as_ref(),
            gtk::FileChooserAction::Open,
            &[("Cancel", gtk::ResponseType::Cancel), ("Insert", gtk::ResponseType::Accept)],
        );
        dialog.set_select_multiple(true);
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Images"));
        for ext in IMAGE_EXTS {
            filter.add_pattern(&format!("*.{ext}"));
            filter.add_pattern(&format!("*.{}", ext.to_uppercase()));
        }
        dialog.add_filter(filter);
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

#[derive(serde::Serialize)]
struct UiSettings {
    zoom: f32,
    compact: bool,
}

#[tauri::command]
fn ui_settings(config: State<Mutex<Config>>) -> UiSettings {
    let config = config.lock().unwrap();
    UiSettings { zoom: config.zoom, compact: config.compact }
}

#[tauri::command]
fn save_ui_settings(config: State<Mutex<Config>>, zoom: f32, compact: bool) {
    let mut config = config.lock().unwrap();
    if config.zoom != zoom || config.compact != compact {
        config.zoom = zoom;
        config.compact = compact;
        config.save();
    }
}

/// Lets scripted UI tests (see `QB_DEV_EVAL`) report results on stderr.
#[tauri::command]
fn dev_log(msg: String) {
    eprintln!("[ui] {msg}");
}

/// "dark" or "light", following GNOME's appearance setting.
#[tauri::command]
fn system_theme() -> &'static str {
    let out = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output();
    match out {
        Ok(o) if String::from_utf8_lossy(&o.stdout).contains("dark") => "dark",
        _ => "light",
    }
}

/// Opens a link from an answer in the default browser instead of inside the panel.
#[tauri::command]
fn open_external(url: String) -> Result<(), String> {
    let allowed = ["http://", "https://", "mailto:"].iter().any(|p| url.starts_with(p));
    if !allowed {
        return Err(format!("not opening {url}"));
    }
    let mut child = std::process::Command::new("xdg-open").arg(&url).spawn().map_err(err)?;
    // Reap it so it doesn't linger as a zombie.
    std::thread::spawn(move || child.wait());
    Ok(())
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

/// Sets up the GNOME shortcut and autostart in the background, remembering that it's done.
fn run_setup(app: &tauri::AppHandle) {
    let Ok(exe) = std::env::current_exe() else { return };
    let app = app.clone();
    std::thread::spawn(move || match setup::run(&exe) {
        Ok(()) => {
            let config = app.state::<Mutex<Config>>();
            let mut config = config.lock().unwrap();
            config.setup_done = true;
            config.save();
        }
        Err(e) => eprintln!("questionbar: shortcut/autostart setup skipped: {e}"),
    });
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
    // The panel needs to place itself at the screen edge, which Wayland forbids.
    // XWayland allows it, so force the X11 GDK backend before GTK initialises.
    std::env::set_var("GDK_BACKEND", "x11");
    // Avoids blank WebKitGTK windows on some GPU drivers.
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

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

    tauri::Builder::default()
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
                run_setup(app);
            } else if !argv.iter().any(|a| a == "--hidden") {
                panel::show(app);
            }
        }))
        .manage(PanelState::default())
        .manage(Mutex::new(config))
        .manage(Mutex::new(store))
        .manage(NotesDir(notes_dir.clone()))
        .register_uri_scheme_protocol("notes", |ctx, request| {
            serve_note_file(&ctx.app_handle().state::<NotesDir>().0, &request)
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
            ui_settings,
            save_ui_settings,
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
            // First run of an installed build: bind Super+` and start on login. Debug builds
            // never do this on their own, so a dev copy can't take over the real shortcut.
            let setup_done = app.state::<Mutex<Config>>().lock().unwrap().setup_done;
            if force_setup || (!cfg!(debug_assertions) && !setup_done) {
                run_setup(app.handle());
            } else if !cfg!(debug_assertions) {
                if let Ok(exe) = std::env::current_exe() {
                    std::thread::spawn(move || setup::refresh(&exe));
                }
            }
            match watcher::spawn(app.handle().clone(), &notes_dir, own_writes) {
                Ok(w) => {
                    app.manage(Mutex::new(w));
                }
                Err(e) => eprintln!("questionbar: not watching {} for changes: {e}", notes_dir.display()),
            }
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
