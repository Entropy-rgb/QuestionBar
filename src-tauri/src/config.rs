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
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

impl Default for Config {
    fn default() -> Self {
        Config { notes_dir: home().join("Documents/QuestionBar/notes"), width: 420 }
    }
}

fn path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".config"));
    base.join("questionbar/config.json")
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
