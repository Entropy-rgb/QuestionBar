#!/usr/bin/env bash
# Builds QuestionBar and installs it for the current user: ~/.local/bin/questionbar
# and an app-menu entry. The app then binds Super+` and adds itself to autostart
# (`questionbar --setup`, see src-tauri/src/setup.rs).
# Run `./install.sh --uninstall` to remove all of that again (your notes are kept).
set -euo pipefail
cd "$(dirname "$0")"

BIN="$HOME/.local/bin/questionbar"
APPS="$HOME/.local/share/applications/questionbar.desktop"
AUTOSTART="$HOME/.config/autostart/questionbar.desktop"
ICON="$HOME/.local/share/icons/hicolor/256x256/apps/questionbar.png"
KEYS=org.gnome.settings-daemon.plugins.media-keys
KEY_PATH=/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/questionbar/
CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}/questionbar/config.json"

# Removes one entry from a gsettings string-list: gs_remove <schema> <key> <value>
gs_remove() {
  local current
  current=$(gsettings get "$1" "$2")
  gsettings set "$1" "$2" "$(python3 -c '
import ast, sys
cur, val = sys.argv[1], sys.argv[2]
items = [] if cur.startswith("@") else ast.literal_eval(cur)
print(repr([i for i in items if i != val]))' "$current" "$3")"
}

if [[ "${1:-}" == "--uninstall" ]]; then
  "$BIN" --quit 2>/dev/null || true
  rm -f "$BIN" "$APPS" "$AUTOSTART" "$ICON"
  gs_remove $KEYS custom-keybindings "$KEY_PATH"
  gsettings reset-recursively "$KEYS.custom-keybinding:$KEY_PATH"
  gsettings reset org.gnome.desktop.wm.keybindings switch-group
  gsettings reset org.gnome.desktop.wm.keybindings switch-group-backward
  # So a later install (or the .deb) sets the shortcut up again on first run.
  if [[ -f "$CONFIG" ]]; then
    python3 -c '
import json, sys
path = sys.argv[1]
config = json.load(open(path))
config["setup_done"] = False
open(path, "w").write(json.dumps(config, indent=2) + "\n")' "$CONFIG"
  fi
  echo "QuestionBar uninstalled. Notes were left where they are."
  exit 0
fi

echo "Building (release)…"
# Marked as a source build: it updates with `git pull && ./install.sh`, not from releases.
(cd src-tauri && QUESTIONBAR_SOURCE_BUILD=1 cargo build --release --features tauri/custom-protocol)

install -Dm755 src-tauri/target/release/questionbar "$BIN"
install -Dm644 src-tauri/icons/icon.png "$ICON"

mkdir -p "$(dirname "$APPS")"
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

# Restart any running copy so the new build is the one in the background.
if pgrep -x questionbar >/dev/null; then
  "$BIN" --quit || true
  for _ in 1 2 3 4 5 6 7 8 9 10; do pgrep -x questionbar >/dev/null || break; sleep 0.3; done
  pkill -x questionbar 2>/dev/null || true
fi
# --setup (re)binds Super+` to this binary and adds it to autostart.
setsid "$BIN" --hidden --setup >/dev/null 2>&1 < /dev/null &

echo "Installed. Press Super+\` to open QuestionBar."
echo "Notes folder and width are set in ~/.config/questionbar/config.json"
