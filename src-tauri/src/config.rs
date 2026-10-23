//! User settings, stored at `~/.config/questionbar/config.json`.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Folder holding the topic `.md` files (and `assets/` for images).
    pub notes_dir: PathBuf,
    /// Panel width in physical pixels, remembered across restarts.
    pub width: u32,
    /// Text size of questions and answers (1.0 = 100%).
    pub zoom: f32,
    /// One line per question in the list.
    pub compact: bool,
    /// The Super+` shortcut and autostart have been set up (see setup.rs).
    pub setup_done: bool,
    /// Shortcut that toggles the panel, in GNOME's accelerator syntax.
    pub hotkey: String,
    /// Start hidden when you log in.
    pub autostart: bool,
    /// Screen edge the panel docks to: "right" or "left".
    pub side: String,
    /// Screen to open on: "primary", "mouse", or a monitor's name.
    pub monitor: String,
    /// "system", "light" or "dark".
    pub theme: String,
    /// The welcome topic was offered (it's only created in an empty notes folder).
    pub welcome_done: bool,
    /// Ask GitHub for a newer version at start and once a day.
    pub check_updates: bool,
    /// Widen the panel when you start writing an answer.
    pub wide_editing: bool,
}

fn documents() -> PathBuf {
    dirs::document_dir().or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from("."))
}

impl Default for Config {
    fn default() -> Self {
        Config {
            notes_dir: documents().join("QuestionBar").join("notes"),
            width: 420,
            zoom: 1.0,
            compact: false,
            setup_done: false,
            hotkey: "<Super>grave".into(),
            autostart: true,
            side: "right".into(),
            monitor: "primary".into(),
            theme: "system".into(),
            welcome_done: false,
            check_updates: true,
            wide_editing: false,
        }
    }
}

/// `~/.config/questionbar/config.json` on Linux, `%APPDATA%\questionbar\config.json` on Windows.
fn path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("questionbar").join("config.json")
}

impl Config {
    /// Loads the config, writing the defaults on first run so the file is easy to find and edit.
    pub fn load() -> Config {
        let path = path();
        match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                eprintln!("questionbar: ignoring invalid {}: {e}", path.display());
                Config::default()
            }),
            Err(_) => {
                let config = Config::default();
                config.save();
                config
            }
        }
    }

    pub fn save(&self) {
        let path = path();
        let result = fs::create_dir_all(path.parent().unwrap())
            .and_then(|_| fs::write(&path, serde_json::to_string_pretty(self).unwrap() + "\n"));
        if let Err(e) = result {
            eprintln!("questionbar: could not save {}: {e}", path.display());
        }
    }
}
