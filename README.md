# QuestionBar

QuestionBar is a panel that slides in from the edge of your screen while you study. When a question comes up in a lecture, a paper or a tutorial, press **Super+`**, type it and press Enter. The panel hides and you keep going.

Later you open it again, pick a question and write the answer yourself, in Markdown with maths, code and images. You write every answer in your own words, because that's the part that makes it stick.

Your notes are plain Markdown files, one per topic, in a folder you choose. Open them in any editor or in Obsidian. There's no account and no cloud.

What you get:

- **Fast capture.** One shortcut from any app. Hashtags at the end file the question (`Why do magnets attract? #physics`). The ❝ button quotes the sentence you highlighted, so you remember what confused you.
- **Answers worth rereading.** An editor that shows your answer formatted as you write it: maths, tables, images, callouts and checklists render in place, and the Markdown appears only where your cursor is. `\` suggests LaTeX symbols, `/` inserts blocks, and you can paste screenshots.
- **Questions that link up.** Type `[[` to link one question to another. Hover a link to peek, click to jump.
- **Easy to find later.** Search, filters, a compact list and full keyboard control.

QuestionBar runs on Linux and Windows. On Linux it sets itself up on GNOME, and works on KDE, Xfce, Sway, Hyprland and others once you bind a key ([see below](#other-desktops)).

## Install

**Windows:** download `QuestionBar_…_x64-setup.exe` from [Releases](https://github.com/Entropy-rgb/QuestionBar/releases) and run it. It installs for your user only, so you don't need admin rights. The installer isn't code-signed yet, so Windows shows "Windows protected your PC" the first time: click **More info**, then **Run anyway**. QuestionBar then opens with **Win+`**. Windows Terminal claims Win+` while it runs; if it got there first, QuestionBar tells you and uses **Ctrl+Alt+Q** instead. Change it any time in Settings.

**Ubuntu and Debian:** download the `.deb` from [Releases](https://github.com/Entropy-rgb/QuestionBar/releases) and install it with `sudo apt install ./QuestionBar_*.deb`. Then start QuestionBar once from the app menu.

**From source** (Ubuntu): install Rust and Tauri's system libraries, then run the installer:

```sh
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
./install.sh
```

`install.sh` builds `~/.local/bin/questionbar` and adds it to the app menu. `./install.sh --uninstall` removes it again and leaves your notes alone.

**On first start**, QuestionBar adds itself to the apps that start when you log in (hidden, waiting for the shortcut). On GNOME it also binds **Super+`**; on Windows it registers **Win+`** each time it starts. GNOME uses Super+` to switch between windows of the same app, and that keeps working on **Alt+`**. Change the shortcut in Settings, or run `questionbar --setup` to put it back after removing it.

## Getting started

An empty notes folder starts with a **welcome** topic: six answered questions that show each feature by using it, and one open question for you to try. Delete them whenever you like. Settings → **Welcome questions** brings them back.

## Using it

| | |
|---|---|
| **Super+`** | Show the panel with the cursor in the question box; press again to hide |
| `Why does X happen? #rust #ownership` + Enter | Adds a question. The first trailing hashtag is the **topic** (the file it goes into, here `rust.md`), the rest are **tags**. Without a hashtag it goes to `inbox.md`. Type `#` for suggestions. |
| Click a question | Opens it; click its title again to close it |
| Circle on the left | Marks it answered or open (your first answer marks it answered for you) |
| Double-click an answer | Edits it, with the cursor on the word you clicked |
| **Write answer** | Opens the editor. It shows your answer formatted as you type (Obsidian-style live preview): the Markdown symbols show only on the line your cursor is on, and maths, tables, images, callouts and checkboxes render in place. It saves as you type; **Ctrl+Enter** or **Esc** closes it |
| **⤢ Wide**, or **Ctrl+Shift+W** while writing | Widens the panel to half the screen for long answers; it narrows again when you click Done. QuestionBar remembers if you like writing wide |
| `/` at the start of a line or after a space | Block menu: heading, lists, checklist, code block (`/python`, `/rust`… for a language), maths, table, divider, quote, question link, Note / Tip / Warning / Important callouts, fold-away block, image from file |
| Cursor inside `$…$` or `$$…$$` | Shows the source with a preview of the formula; `\` suggests LaTeX commands (`\al` → α) and templates like `\frac{}{}` |
| Enter in a list, checklist or quote | Continues it; Enter on an empty item ends it |
| Checkbox in an answer | Ticks the item and saves it |
| Paste or drop an image while editing | Saved in `notes/assets/` and linked |
| **Edit question** | Change the text and hashtags; a new first hashtag moves the question to that topic |
| ❝ button, or **Ctrl+Shift+V** in the question box | Quotes the text you last highlighted in any app, or copied if nothing is highlighted. Windows has no highlighted-text selection, so there it quotes what you copied with Ctrl+C. |
| `[[` in an answer or a question | Links another question. Hover the link to preview it, click to jump, **← Back** or **Alt+Left** to return |
| Search box | Searches questions, answers and quotes; `#tag` limits it to a topic or tag |
| 📌 | Keeps the panel open when you click elsewhere (it also stays open while you edit) |
| ⚙ | Settings |

Maths uses `$inline$` and `$$display$$` (KaTeX); `$ x $` with spaces inside works too. Code blocks take a language: ```` ```rust ````. Callouts use Obsidian's syntax, `> [!note] Title`; add `-` (`> [!tip]- Title`) to make one fold away until clicked.

`questionbar --quit` stops the background process.

## Keyboard

The list works without a mouse whenever you aren't typing in a box:

| Key | What it does |
|---|---|
| ↓ in the question or search box | jump into the list |
| ↑ ↓ or j k | move between questions (↑ on the first one goes back to the question box) |
| Home / End | first / last question |
| Enter, → / ← | open or close the question |
| e | edit the answer |
| t | edit the question |
| x | mark answered or open |
| d, then d again | delete |
| n or Ctrl+N | new question |
| / or Ctrl+F | search |
| Ctrl+= / Ctrl+- / Ctrl+0, or Ctrl+scroll | text size (70–250%) |
| Ctrl+Shift+W | write wide (while editing an answer) |
| Esc | close the panel |

## Settings

Open Settings with the ⚙ button. You can:

- change the **shortcut** (on GNOME; it warns you if GNOME already uses the combination). Pick "Key above Tab" if your keyboard isn't US-English: it means the same physical key on any layout.
- change the **notes folder**, and either move your questions there or leave them where they are
- dock the panel on the **left or right**, and choose the **screen** it opens on
- pick a **theme**, the **text size** and the **compact list**
- turn **start on login** on or off
- add the **welcome questions** again
- turn the **update check** on or off, or check now

QuestionBar saves these in `~/.config/questionbar/config.json` on Linux and `%APPDATA%\questionbar\config.json` on Windows.

## Updates

When a new release is out, a bar at the top of the panel says so.

- **Windows:** click **Restart to update**. QuestionBar downloads the new version, checks its signature, installs it and starts again.
- **Linux (.deb):** click **Download** and install the new `.deb` the same way as the first one. A `.deb` can't replace itself.
- **Built from source:** `git pull` and run `./install.sh` again. These copies don't check for updates.

To find out, QuestionBar asks GitHub which release is newest when it starts and once a day. Turn that off in Settings → Updates.

## Other desktops

QuestionBar binds its shortcut itself only on GNOME. On other desktops it still adds itself to `~/.config/autostart`, which KDE, Xfce, Cinnamon and MATE read. Bind a key to this command:

```sh
questionbar --toggle
```

- **KDE Plasma:** System Settings → Keyboard → Shortcuts → Add New → Command or Script. Enter `questionbar --toggle` and choose a key.
- **Xfce:** Settings → Keyboard → Application Shortcuts → Add.
- **Cinnamon and MATE:** Keyboard settings → Shortcuts → Custom Shortcuts.
- **i3 and Sway** (in your config file):

  ```
  bindsym $mod+grave exec questionbar --toggle
  for_window [instance="questionbar"] floating enable, sticky enable, border none
  exec questionbar --hidden
  ```

  On i3, write `exec --no-startup-id` instead of `exec`.
- **Hyprland** (in `hyprland.conf`):

  ```
  bind = SUPER, grave, exec, questionbar --toggle
  exec-once = questionbar --hidden
  windowrule = float, class:^([Qq]uestionbar)$
  windowrule = pin, class:^([Qq]uestionbar)$
  ```

  Hyprland changes its window-rule syntax between releases, so check the wiki if these rules don't load.

The window's class is `questionbar`. On tiling compositors, make it float; otherwise the compositor tiles it instead of letting it dock at the screen edge.

## Obsidian and synced folders

Settings → Notes folder → **Change…**, then pick your vault (or a folder inside it). QuestionBar writes one `.md` file per topic and leaves files without questions alone. Obsidian shows the quotes, callouts and maths the same way. Links between questions point at ids (`[title](#id)`), so in Obsidian they show up as plain links.

To use your notes on more than one computer, keep the notes folder in Syncthing, Dropbox or Nextcloud. QuestionBar reloads when a file changes underneath it.

## Privacy

QuestionBar has no account and no server, and it doesn't collect analytics. On its own it makes one kind of request: the update check, which asks GitHub for the newest release when the app starts and once a day (turn it off in Settings → Updates). Links you click in an answer open in your browser. Your notes live in your notes folder and nowhere else.

## Troubleshooting

- **The shortcut does nothing.** Another app may own that combination. Pick a different one in Settings. On GNOME you can see your shortcuts in Settings → Keyboard → Keyboard Shortcuts → Custom Shortcuts. On Windows, QuestionBar tells you when another app (often Windows Terminal) holds the shortcut, and switches to Ctrl+Alt+Q.
- **The panel appears, but your typing goes to the other window.** GNOME sometimes keeps focus on the app you were using. Click the panel once.
- **Blurry text with 125% or 150% scaling.** The panel runs through XWayland. On GNOME 47 or newer, run the command below, then log out and back in:

  ```sh
  gsettings set org.gnome.mutter experimental-features "['scale-monitor-framebuffer', 'xwayland-native-scaling']"
  ```
- **Something about setup looks wrong.** `questionbar --setup` adds the shortcut and the autostart entry again.

## File format

`notes/rust.md`:

```markdown
# Rust

## Why does assigning a String move it?
<!-- id: mh2k3l9x4f  status: answered  tags: ownership, memory  asked: 2026-10-22 -->

Because `String` owns heap data...
```

A captured quote sits directly under the metadata line as an Obsidian-style callout:

```markdown
## Why does the borrow end here?
<!-- id: mh3x8k2p1a  status: open  asked: 2026-10-22 -->
> [!quote]
> A reference's scope starts from where it is introduced...

Your answer...
```

Links to other questions are ordinary Markdown links to the question's id, `[Why does a String move?](#mh2k3l9x4f)`, so renaming or moving a question keeps them working.

A `##` heading only counts as a question when the `<!-- id: … -->` line sits directly below it, so you can use headings inside answers. Edit the files in any editor while the app runs; it reloads them.

## Development

```sh
cd src-tauri
cargo test           # parser, storage and setup tests
cargo run            # debug build; the UI is in ../ui (plain HTML/JS, no bundler)
```

The answer editor uses CodeMirror 6, bundled once into `ui/vendor/codemirror.min.js`. To change its version or the parts it exports (`scripts/codemirror-entry.mjs`), run `npm install` and `npm run build:editor`. The app itself needs no Node at build or run time.

The window runs through XWayland (`GDK_BACKEND=x11`) because Wayland doesn't let apps dock themselves to a screen edge.

Debug builds never change your real shortcut or autostart entry. With `QB_DEV_EVAL=script.js`, a debug build runs that script in the panel after it loads, for scripted UI checks.

Platform-specific code lives in `src-tauri/src/platform_linux.rs` and `platform_windows.rs`. Every push runs the tests and builds both the `.deb` and the Windows installer (`.github/workflows/ci.yml`).

## Releases

GitHub → **Actions** → **Release** → **Run workflow**. Type a version (`0.2.0`), or leave it empty to bump the last release (`0.1.0` → `0.1.1`). The workflow runs the tests, builds the `.deb` on Ubuntu 22.04 and the Windows installer, and publishes one release with both, plus the `latest.json` file the Windows updater reads. The version only goes into the build; nothing is committed back.

Windows updates are signed. The workflow signs them with the private key in the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, and the app checks them against the public key in `tauri.conf.json`. Keep a backup of the private key: without it, installed copies can't receive updates.

## License

[MIT](LICENSE). The libraries bundled in `ui/vendor/` (markdown-it, markdown-it-texmath, KaTeX, highlight.js, CodeMirror) keep their own licenses, which sit next to them.
