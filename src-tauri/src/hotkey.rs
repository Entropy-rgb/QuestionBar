//! Shortcuts in GNOME's accelerator syntax (`<Super>grave`), which is how config.json
//! stores them on every platform, plus the conversion the Windows hotkey API needs.

pub const SUPER: u8 = 1;
pub const CTRL: u8 = 2;
pub const ALT: u8 = 4;
pub const SHIFT: u8 = 8;

/// Splits an accelerator like `<Super><Primary>t` into modifier bits and a key name,
/// so different spellings of the same shortcut compare equal.
pub fn parse(accel: &str) -> Option<(u8, String)> {
    let mut mods = 0;
    let mut rest = accel.trim();
    while let Some(inner) = rest.strip_prefix('<') {
        let end = inner.find('>')?;
        mods |= match inner[..end].to_ascii_lowercase().as_str() {
            "super" | "mod4" => SUPER,
            "primary" | "control" | "ctrl" | "ctl" => CTRL,
            "alt" | "mod1" => ALT,
            "shift" => SHIFT,
            _ => 0,
        };
        rest = &inner[end + 1..];
    }
    if rest.is_empty() || rest.contains(|c: char| c.is_whitespace() || c == '/') {
        return None;
    }
    // GNOME names the key above Tab `Above_Tab` whatever the layout; on most it's `.
    let key = match rest.to_ascii_lowercase().as_str() {
        "above_tab" => "grave".to_string(),
        other => other.to_string(),
    };
    Some((mods, key))
}

fn function_key(key: &str) -> bool {
    key.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()).is_some_and(|n| (1..=12).contains(&n))
}

/// A shortcut someone can press: at least one modifier, or a function key on its own.
pub fn valid(accel: &str) -> bool {
    match parse(accel) {
        Some((mods, key)) => mods != 0 || function_key(&key),
        None => false,
    }
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn is_super_backtick(accel: &str) -> bool {
    parse(accel) == Some((SUPER, "grave".to_string()))
}

/// The shortcut as the global-shortcut plugin spells it, e.g. `Super+Backquote`.
/// Key names are physical keys, so "the key above Tab" stays that key on any layout.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn to_global(accel: &str) -> Option<String> {
    let (mods, key) = parse(accel)?;
    let key = match key.as_str() {
        "grave" => "Backquote".to_string(),
        "space" => "Space".into(),
        "slash" => "Slash".into(),
        "period" => "Period".into(),
        "comma" => "Comma".into(),
        "semicolon" => "Semicolon".into(),
        "apostrophe" => "Quote".into(),
        "backslash" => "Backslash".into(),
        "bracketleft" => "BracketLeft".into(),
        "bracketright" => "BracketRight".into(),
        "insert" => "Insert".into(),
        "pause" => "Pause".into(),
        k if k.len() == 1 && k.chars().all(|c| c.is_ascii_lowercase()) => format!("Key{}", k.to_ascii_uppercase()),
        k if k.len() == 1 && k.chars().all(|c| c.is_ascii_digit()) => format!("Digit{k}"),
        k if function_key(k) => k.to_ascii_uppercase(),
        _ => return None,
    };
    let mut parts: Vec<&str> = [(SUPER, "Super"), (CTRL, "Control"), (ALT, "Alt"), (SHIFT, "Shift")]
        .into_iter()
        .filter(|(bit, _)| mods & bit != 0)
        .map(|(_, name)| name)
        .collect();
    parts.push(&key);
    Some(parts.join("+"))
}

/// A readable label for messages, e.g. `Win + ``.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn describe(accel: &str) -> String {
    let Some((mods, key)) = parse(accel) else { return accel.to_string() };
    let mut parts: Vec<String> = [(SUPER, "Win"), (CTRL, "Ctrl"), (ALT, "Alt"), (SHIFT, "Shift")]
        .into_iter()
        .filter(|(bit, _)| mods & bit != 0)
        .map(|(_, name)| name.to_string())
        .collect();
    parts.push(match key.as_str() {
        "grave" => "`".to_string(),
        "space" => "Space".to_string(),
        k => k.to_uppercase(),
    });
    parts.join(" + ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spellings_compare_equal() {
        assert_eq!(parse("<Super>grave"), parse("<super>Above_Tab"));
        assert_eq!(parse("<Primary><Alt>t"), parse("<Alt><Control>T"));
        assert_ne!(parse("<Super>t"), parse("<Super><Shift>t"));
        assert!(is_super_backtick("<Super>Above_Tab"));
    }

    #[test]
    fn validity() {
        assert!(valid("<Super>grave") && valid("F9") && valid("<Primary>space"));
        assert!(!valid("t") && !valid("<Super>") && !valid("F13") && !valid("/org/x/"));
    }

    #[test]
    fn windows_spelling() {
        assert_eq!(to_global("<Super>grave").as_deref(), Some("Super+Backquote"));
        assert_eq!(to_global("<Super>Above_Tab").as_deref(), Some("Super+Backquote"));
        assert_eq!(to_global("<Primary><Alt>q").as_deref(), Some("Control+Alt+KeyQ"));
        assert_eq!(to_global("<Shift>F5").as_deref(), Some("Shift+F5"));
        assert_eq!(to_global("<Alt>7").as_deref(), Some("Alt+Digit7"));
        assert_eq!(to_global("<Super>bracketleft").as_deref(), Some("Super+BracketLeft"));
        assert_eq!(to_global("<Super>Print"), None);
        assert_eq!(describe("<Primary><Alt>q"), "Ctrl + Alt + Q");
        assert_eq!(describe("<Super>grave"), "Win + `");
    }
}
