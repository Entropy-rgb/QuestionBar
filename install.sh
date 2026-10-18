#!/usr/bin/env bash
# Builds QuestionBar and installs it for the current user:
#   ~/.local/bin/questionbar, an app-menu entry, autostart on login (hidden),
#   and the GNOME shortcut Super+` -> `questionbar --toggle`.
# Run `./install.sh --uninstall` to remove all of that again (your notes are kept).
set -euo pipefail
cd "$(dirname "$0")"

BIN="$HOME/.local/bin/questionbar"
APPS="$HOME/.local/share/applications/questionbar.desktop"
AUTOSTART="$HOME/.config/autostart/questionbar.desktop"
ICON="$HOME/.local/share/icons/hicolor/256x256/apps/questionbar.png"
KEYS=org.gnome.settings-daemon.plugins.media-keys
KEY_PATH=/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/questionbar/
HOTKEY='<Super>grave'

# Adds or removes one entry of a gsettings string-list: gs_list <schema> <key> add|remove <value>
gs_list() {
  local current
  current=$(gsettings get "$1" "$2")
  gsettings set "$1" "$2" "$(python3 -c '
import ast, sys
cur, op, val = sys.argv[1], sys.argv[2], sys.argv[3]
items = [] if cur.startswith("@") else ast.literal_eval(cur)
items = [i for i in items if i != val] + ([val] if op == "add" else [])
print(repr(items))' "$current" "$3" "$4")"
}

if [[ "${1:-}" == "--uninstall" ]]; then
  "$BIN" --quit 2>/dev/null || true
  rm -f "$BIN" "$APPS" "$AUTOSTART" "$ICON"
  gs_list $KEYS custom-keybindings remove "$KEY_PATH"
  gsettings reset-recursively "$KEYS.custom-keybinding:$KEY_PATH"
  gsettings reset org.gnome.desktop.wm.keybindings switch-group
  gsettings reset org.gnome.desktop.wm.keybindings switch-group-backward
  echo "QuestionBar uninstalled. Notes were left where they are."
  exit 0
fi

echo "Building (release)…"
(cd src-tauri && cargo build --release --features tauri/custom-protocol)

install -Dm755 src-tauri/target/release/questionbar "$BIN"
install -Dm644 src-tauri/icons/icon.png "$ICON"

mkdir -p "$(dirname "$APPS")" "$(dirname "$AUTOSTART")"
cat > "$APPS" <<EOF
[Desktop Entry]
Type=Application
Name=QuestionBar
Comment=Capture and answer the questions you have while learning
Exec=$BIN --toggle
Icon=questionbar
Terminal=false
Categories=Education;Utility;
StartupWMClass=questionbar
EOF
cat > "$AUTOSTART" <<EOF
[Desktop Entry]
Type=Application
Name=QuestionBar
Exec=$BIN --hidden
Icon=questionbar
Terminal=false
X-GNOME-Autostart-enabled=true
NoDisplay=true
EOF

# Super+` is GNOME's "switch windows of an app" by default; Alt+` keeps doing that.
gs_list org.gnome.desktop.wm.keybindings switch-group remove '<Super>Above_Tab'
gs_list org.gnome.desktop.wm.keybindings switch-group-backward remove '<Shift><Super>Above_Tab'
gsettings set "$KEYS.custom-keybinding:$KEY_PATH" name 'QuestionBar toggle'
gsettings set "$KEYS.custom-keybinding:$KEY_PATH" command "$BIN --toggle"
gsettings set "$KEYS.custom-keybinding:$KEY_PATH" binding "$HOTKEY"
gs_list $KEYS custom-keybindings add "$KEY_PATH"

# Restart any running copy so the new build is the one in the background.
if pgrep -x questionbar >/dev/null; then
  "$BIN" --quit || true
  for _ in 1 2 3 4 5 6 7 8 9 10; do pgrep -x questionbar >/dev/null || break; sleep 0.3; done
  pkill -x questionbar 2>/dev/null || true
fi
setsid "$BIN" --hidden >/dev/null 2>&1 < /dev/null &

echo "Installed. Press Super+\` to open QuestionBar."
echo "Notes folder and width are set in ~/.config/questionbar/config.json"
