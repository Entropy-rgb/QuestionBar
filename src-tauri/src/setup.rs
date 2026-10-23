//! Desktop integration: the shortcut that toggles the panel (set up on GNOME; other
//! desktops bind `questionbar --toggle` themselves) and starting hidden on login.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::hotkey;

const MEDIA_KEYS: &str = "org.gnome.settings-daemon.plugins.media-keys";
const KEY_PATH: &str = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/questionbar/";
const WM_KEYS: &str = "org.gnome.desktop.wm.keybindings";
/// Schemas whose keys hold GNOME's own shortcuts, checked for clashes.
const SHORTCUT_SCHEMAS: [&str; 5] = [
    "org.gnome.desktop.wm.keybindings",
    "org.gnome.shell.keybindings",
    "org.gnome.mutter.keybindings",
    "org.gnome.mutter.wayland.keybindings",
    MEDIA_KEYS,
];

/// True on GNOME (Ubuntu reports `ubuntu:GNOME`), the only desktop whose shortcuts we set.
pub fn is_gnome() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| d.split(':').any(|part| part.eq_ignore_ascii_case("gnome")))
        .unwrap_or(false)
}

fn gsettings(args: &[&str]) -> Result<String, String> {
    let out = Command::new("gsettings").args(args).output().map_err(|e| format!("gsettings: {e}"))?;
    if !out.status.success() {
        return Err(format!("gsettings {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Parses gsettings' text form of a string list: `['a', 'b']` or `@as []`.
fn parse_list(text: &str) -> Vec<String> {
    let text = text.trim().trim_start_matches("@as").trim();
    let inner = text.strip_prefix('[').and_then(|t| t.strip_suffix(']')).unwrap_or("");
    inner.split(',').map(|item| item.trim().trim_matches('\'').to_string()).filter(|item| !item.is_empty()).collect()
}

fn format_list(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|i| gvariant_string(i)).collect();
    format!("[{}]", quoted.join(", "))
}

/// A string in GVariant text format, as `gsettings set` expects.
fn gvariant_string(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn edit_list(schema: &str, key: &str, edit: impl FnOnce(&mut Vec<String>)) -> Result<(), String> {
    let mut items = parse_list(&gsettings(&["get", schema, key])?);
    let before = items.clone();
    edit(&mut items);
    if items != before {
        gsettings(&["set", schema, key, &format_list(&items)])?;
    }
    Ok(())
}

/// The program path as it goes into a command line (quoted only if it has to be).
fn command_path(exe: &Path) -> String {
    let path = exe.to_string_lossy();
    if path.contains(|c: char| c.is_whitespace() || c == '"' || c == '\\') {
        format!("\"{}\"", path.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        path.into_owned()
    }
}

/// The program a command line runs (first word, honouring double quotes).
fn command_program(command: &str) -> String {
    let command = command.trim();
    match command.strip_prefix('"') {
        Some(rest) => rest.split('"').next().unwrap_or("").to_string(),
        None => command.split_whitespace().next().unwrap_or("").to_string(),
    }
}

fn shortcut_schema() -> String {
    format!("{MEDIA_KEYS}.custom-keybinding:{KEY_PATH}")
}

/// GNOME uses Super+` to switch between windows of one app; Alt+` keeps doing that.
fn free_switch_group() -> Result<(), String> {
    edit_list(WM_KEYS, "switch-group", |keys| keys.retain(|k| k != "<Super>Above_Tab"))?;
    edit_list(WM_KEYS, "switch-group-backward", |keys| keys.retain(|k| k != "<Shift><Super>Above_Tab"))
}

/// Gives Super+` back to GNOME's window switcher.
fn restore_switch_group() -> Result<(), String> {
    edit_list(WM_KEYS, "switch-group", |keys| {
        if !keys.iter().any(|k| k == "<Super>Above_Tab") {
            keys.insert(0, "<Super>Above_Tab".to_string());
        }
    })?;
    edit_list(WM_KEYS, "switch-group-backward", |keys| {
        if !keys.iter().any(|k| k == "<Shift><Super>Above_Tab") {
            keys.insert(0, "<Shift><Super>Above_Tab".to_string());
        }
    })
}

fn ensure_shortcut(exe: &Path, hotkey: &str) -> Result<(), String> {
    if hotkey::is_super_backtick(hotkey) {
        free_switch_group()?;
    }
    let schema = shortcut_schema();
    gsettings(&["set", &schema, "name", &gvariant_string("QuestionBar toggle")])?;
    gsettings(&["set", &schema, "command", &gvariant_string(&format!("{} --toggle", command_path(exe)))])?;
    gsettings(&["set", &schema, "binding", &gvariant_string(hotkey)])?;
    edit_list(MEDIA_KEYS, "custom-keybindings", |paths| {
        if !paths.iter().any(|p| p == KEY_PATH) {
            paths.push(KEY_PATH.to_string());
        }
    })
}

/// First run (or `--setup`): autostart if wanted, and the shortcut on GNOME.
pub fn run(exe: &Path, hotkey: &str, autostart: bool) -> Result<(), String> {
    if autostart {
        write_autostart(exe).map_err(|e| format!("autostart: {e}"))?;
    }
    if is_gnome() {
        ensure_shortcut(exe, hotkey)?;
    }
    Ok(())
}

/// Switches the GNOME shortcut from `old` to `new`.
pub fn set_hotkey(exe: &Path, old: &str, new: &str) -> Result<(), String> {
    if !is_gnome() {
        return Err("QuestionBar can only set the shortcut on GNOME. Bind `questionbar --toggle` in your desktop's keyboard settings.".into());
    }
    ensure_shortcut(exe, new)?;
    if hotkey::is_super_backtick(old) && !hotkey::is_super_backtick(new) {
        restore_switch_group()?;
    }
    Ok(())
}

/// Whatever else on GNOME already uses `hotkey`, by name. GNOME's Super+` window
/// switcher isn't listed: QuestionBar moves that one to Alt+` itself.
pub fn conflicts(hotkey: &str) -> Vec<String> {
    let Some(target) = hotkey::parse(hotkey) else { return Vec::new() };
    let same = |accel: &str| hotkey::parse(accel).as_ref() == Some(&target);
    let mut found = Vec::new();
    for schema in SHORTCUT_SCHEMAS {
        let Ok(listing) = gsettings(&["list-recursively", schema]) else { continue };
        for line in listing.lines() {
            let mut parts = line.splitn(3, ' ');
            let (Some(_), Some(key), Some(value)) = (parts.next(), parts.next(), parts.next()) else { continue };
            let accels = if value.starts_with('\'') { vec![value.trim_matches('\'').to_string()] } else { parse_list(value) };
            if key == "switch-group" && hotkey::is_super_backtick(hotkey) {
                continue;
            }
            if accels.iter().any(|a| same(a)) {
                found.push(key.replace('-', " "));
            }
        }
    }
    if let Ok(paths) = gsettings(&["get", MEDIA_KEYS, "custom-keybindings"]) {
        for path in parse_list(&paths).into_iter().filter(|p| p != KEY_PATH) {
            let schema = format!("{MEDIA_KEYS}.custom-keybinding:{path}");
            let binding = gsettings(&["get", &schema, "binding"]).unwrap_or_default();
            if same(binding.trim_matches('\'')) {
                let name = gsettings(&["get", &schema, "name"]).unwrap_or_default();
                found.push(name.trim_matches('\'').to_string());
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

fn autostart_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("autostart/questionbar.desktop")
}

fn write_autostart(exe: &Path) -> io::Result<()> {
    let path = autostart_path();
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(
        path,
        format!(
            "[Desktop Entry]\nType=Application\nName=QuestionBar\nExec={} --hidden\nIcon=questionbar\n\
             Terminal=false\nX-GNOME-Autostart-enabled=true\nNoDisplay=true\n",
            command_path(exe)
        ),
    )
}

pub fn autostart_enabled() -> bool {
    autostart_path().exists()
}

pub fn set_autostart(exe: &Path, on: bool) -> io::Result<()> {
    if on {
        return write_autostart(exe);
    }
    match fs::remove_file(autostart_path()) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// If QuestionBar was reinstalled somewhere else, points the existing shortcut and
/// autostart entry at the new program. Never re-adds anything the user removed.
pub fn refresh(exe: &Path) {
    let schema = shortcut_schema();
    let registered = gsettings(&["get", MEDIA_KEYS, "custom-keybindings"])
        .map(|list| parse_list(&list).iter().any(|p| p == KEY_PATH))
        .unwrap_or(false);
    if registered {
        if let Ok(current) = gsettings(&["get", &schema, "command"]) {
            let current = current.trim_matches('\'');
            if !Path::new(&command_program(current)).exists() {
                let command = format!("{} --toggle", command_path(exe));
                let _ = gsettings(&["set", &schema, "command", &gvariant_string(&command)]);
            }
        }
    }
    if let Ok(text) = fs::read_to_string(autostart_path()) {
        let program = text.lines().find_map(|l| l.strip_prefix("Exec=")).map(command_program).unwrap_or_default();
        if !Path::new(&program).exists() {
            let _ = write_autostart(exe);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gsettings_lists() {
        assert_eq!(parse_list("@as []"), Vec::<String>::new());
        assert_eq!(parse_list("['<Super>Above_Tab', '<Alt>Above_Tab']"), vec!["<Super>Above_Tab", "<Alt>Above_Tab"]);
        let items = vec!["/a/".to_string(), "/b/".to_string()];
        assert_eq!(parse_list(&format_list(&items)), items);
        assert_eq!(format_list(&[]), "[]");
    }

    #[test]
    fn command_lines() {
        assert_eq!(command_path(Path::new("/usr/bin/questionbar")), "/usr/bin/questionbar");
        assert_eq!(command_path(Path::new("/home/a b/questionbar")), "\"/home/a b/questionbar\"");
        assert_eq!(command_program("/usr/bin/questionbar --toggle"), "/usr/bin/questionbar");
        assert_eq!(command_program("\"/home/a b/questionbar\" --toggle"), "/home/a b/questionbar");
        assert_eq!(gvariant_string("it's"), "'it\\'s'");
    }
}
