# QuestionBar

A side panel for the questions you have while learning. Press **Super+`**, type the question, press Enter, and get back to what you were doing. Later, open it again and write the answers yourself, with Markdown, code, math and images.

Everything is stored as plain Markdown, one file per topic, so your notes stay readable in any editor (and in Obsidian).

## Install

Requirements (Ubuntu): Rust, plus Tauri's system libraries:

```sh
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
./install.sh
```

`install.sh` builds a release binary into `~/.local/bin/questionbar`, adds it to the app menu and to autostart (it starts hidden), and binds **Super+`** to toggle it. GNOME normally uses Super+` to switch between windows of the same app; that keeps working on **Alt+`**.

`./install.sh --uninstall` removes all of that and leaves your notes alone.

## Using it

| | |
|---|---|
| **Super+`** | Show the panel (cursor is in "New question"); press again to hide |
| `Why does X happen? #rust #ownership` + Enter | Adds a question. The first trailing hashtag is the **topic** (the file it goes into, here `rust.md`); the rest are **tags**. No hashtag puts it in `inbox.md`. Type `#` for suggestions. |
| Click anywhere on a question | Expand it to read the answer; click its title area again to collapse |
| Circle on the left | Toggle open / answered (writing a first answer marks it answered automatically) |
| Double-click an answer | Opens the editor with the cursor at the word you clicked |
| **Write answer** | Markdown editor. Autosaves; **Ctrl+P** toggles preview, **Ctrl+Enter** or **Esc** closes it |
| Paste a screenshot or drop an image file into the editor | Saved to `notes/assets/` and linked |
| **Edit question** | Edit the text and hashtags; changing the first hashtag moves the question to that topic |
| Search box | Words search questions and answers; `#tag` filters by topic or tag. **Ctrl+F** or **/** to focus it |
| 📌 | Pin: the panel stays open when you click elsewhere (it also stays open while you're editing an answer) |
| **Esc** | Hide the panel |

Math uses `$inline$` and `$$display$$` (KaTeX). Code blocks take a language: ```` ```rust ````.

`questionbar --quit` stops the background process.

## File format

`notes/rust.md`:

```markdown
# Rust

## Why does assigning a String move it?
<!-- id: mh2k3l9x4f  status: answered  tags: ownership, memory  asked: 2026-10-22 -->

Because `String` owns heap data...
```

A `##` heading only counts as a question when the `<!-- id: … -->` line is directly below it, so you can use headings freely inside answers. You can edit these files in any editor while the app is running; it reloads them automatically.

## Settings

`~/.config/questionbar/config.json`:

```json
{ "notes_dir": "/home/you/Documents/QuestionBar/notes", "width": 420 }
```

Point `notes_dir` at another folder (an Obsidian vault, a synced folder…) and restart with `questionbar --quit && questionbar --hidden &`.

## Development

```sh
cd src-tauri
cargo test           # parser and storage tests
cargo run            # debug build; UI is in ../ui (plain HTML/JS, no bundler)
```

The window runs through XWayland (`GDK_BACKEND=x11`) because Wayland doesn't let apps dock themselves to a screen edge.

Bundled libraries in `ui/vendor/`: markdown-it, markdown-it-texmath, KaTeX, highlight.js (licenses alongside).
