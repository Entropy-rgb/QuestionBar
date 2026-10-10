//! Watches the notes folder and tells the UI when topic files change outside the app.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};

use crate::store::{content_hash, OwnWrites};

/// Burst of events (editors often write several times) collapsed into one reload.
const DEBOUNCE: Duration = Duration::from_millis(250);

pub fn spawn(app: AppHandle, dir: &Path, own_writes: OwnWrites) -> notify::Result<RecommendedWatcher> {
    let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
    let mut watcher = notify::recommended_watcher(tx)?;
    watcher.watch(dir, RecursiveMode::NonRecursive)?;

    std::thread::spawn(move || {
        while let Ok(first) = rx.recv() {
            let mut external = is_external(&first, &own_writes);
            while let Ok(next) = rx.recv_timeout(DEBOUNCE) {
                external |= is_external(&next, &own_writes);
            }
            if external {
                let _ = app.emit("notes-changed", ());
            }
        }
    });
    Ok(watcher)
}

fn is_topic_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.ends_with(".md") && !name.starts_with('.')
}

/// True if the event touched a topic file in a way the app itself didn't cause.
fn is_external(event: &notify::Result<notify::Event>, own_writes: &OwnWrites) -> bool {
    let Ok(event) = event else { return false };
    if event.kind.is_access() {
        return false;
    }
    let writes = own_writes.lock().unwrap();
    event.paths.iter().filter(|p| is_topic_file(p)).any(|path: &PathBuf| {
        let current = fs::read(path).ok().map(|bytes| content_hash(&bytes));
        writes.get(path) != Some(&current)
    })
}
