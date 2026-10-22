# QuestionBar

A side panel for the questions you have while learning. Press **Super+`**, type the question, press Enter, and get back to what you were doing. Later, open it again and write the answers yourself, with Markdown, code, math and images.

Everything is stored as plain Markdown, one file per topic, so your notes stay readable in any editor (and in Obsidian).

## Install

**From a release:** download the `.deb` from [Releases](https://github.com/Entropy-rgb/QuestionBar/releases) and install it (`sudo apt install ./QuestionBar_*.deb`). Start QuestionBar once from the app menu.

**From source** (Ubuntu): install Rust and Tauri's system libraries, then run the installer:

```sh
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
./install.sh
```

`install.sh` builds a release binary into `~/.local/bin/questionbar` and adds it to the app menu. `./install.sh --uninstall` removes it again and leaves your notes alone.

**What the first start sets up (GNOME):** the **Super+`** shortcut to toggle the panel, and starting hidden when you log in. GNOME normally uses Super+` to switch between windows of the same app; that keeps working on **Alt+`**. If you remove the shortcut it stays removed; `questionbar --setup` adds it back.

## Using it

| | |
|---|---|
| **Super+`** | Show the panel (cursor is in "New question"); press again to hide |
| `Why does X happen? #rust #ownership` + Enter | Adds a question. The first trailing hashtag is the **topic** (the file it goes into, here `rust.md`); the rest are **tags**. No hashtag puts it in `inbox.md`. Type `#` for suggestions. |
| Click anywhere on a question | Expand it to read the answer; click its title area again to collapse |
| Circle on the left | Toggle open / answered (writing a first answer marks it answered automatically) |
| Double-click an answer | Opens the editor with the cursor at the word you clicked |
| **Write answer** | Markdown editor. Autosaves; **Ctrl+P** toggles preview, **Ctrl+Enter** or **Esc** closes it |
| `/` at the start of a line (or after a space) | Block menu: heading, lists, checklist, code block (`/python`, `/rust`… for a language), math, table, divider, quote, question link, Note / Tip / Warning / Important callouts, fold-away block, image from file |
| Typing inside `$…$` or `$$…$$` | Live preview of the formula under the line; `\` suggests LaTeX commands (`\al` → α) with templates like `\frac{}{}` |
| Enter in a list, checklist or quote | Continues it; Enter on an empty item ends it |
| Checkbox in an answer | Ticks the item and saves it |
| Paste a screenshot or drop an image file into the editor | Saved to `notes/assets/` and linked |
| **Edit question** | Edit the text and hashtags; changing the first hashtag moves the question to that topic |
| ❝ button (or **Ctrl+Shift+V** in the new-question box) | Quotes the text you last highlighted in any app (or copied, if nothing is highlighted). Type your question and press Enter; the quote is saved with it. ✕ discards it. |
| `[[` in an answer, the new-question box, or Edit question | Link to another question: pick it from the suggestions and `[title](#id)` is inserted. Hover a link to preview that question, click to jump to it; **← Back** or **Alt+Left** returns. |
| Search box | Words search questions and answers; `#tag` filters by topic or tag. **Ctrl+F** or **/** to focus it |
| 📌 | Pin: the panel stays open when you click elsewhere (it also stays open while you're editing an answer) |
| **Ctrl+=** / **Ctrl+-** / **Ctrl+0**, or Ctrl+scroll | Text size of questions and answers (70–250%) |
| ☰ (next to All / Open / Answered) | Compact view: one line per question |
| **Esc** | Hide the panel |

Math uses `$inline$` and `$$display$$` (KaTeX). Code blocks take a language: ```` ```rust ````. Callouts use Obsidian's syntax, `> [!note] Title`; add `-` (`> [!tip]- Title`) to make one fold away until clicked.

`questionbar --quit` stops the background process.

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

Links to other questions are ordinary Markdown links to the question's id, `[Why does a String move?](#mh2k3l9x4f)`, so renaming or moving a question never breaks them.

A `##` heading only counts as a question when the `<!-- id: … -->` line is directly below it, so you can use headings freely inside answers. You can edit these files in any editor while the app is running; it reloads them automatically.

## Settings

`~/.config/questionbar/config.json`:

```json
{ "notes_dir": "/home/you/Documents/QuestionBar/notes", "width": 420, "zoom": 1.0, "compact": false, "setup_done": true }
```

Point `notes_dir` at another folder (an Obsidian vault, a synced folder…) and restart with `questionbar --quit && questionbar --hidden &`.

## Development

```sh
cd src-tauri
cargo test           # parser and storage tests
cargo run            # debug build; UI is in ../ui (plain HTML/JS, no bundler)
```

The window runs through XWayland (`GDK_BACKEND=x11`) because Wayland doesn't let apps dock themselves to a screen edge.

Debug builds never set up the shortcut or autostart on their own. With `QB_DEV_EVAL=script.js`, a debug build runs that script in the panel after it loads (for scripted UI checks).

## Releases

GitHub → **Actions** → **Release** → **Run workflow**. Type a version (`0.2.0`), or leave it empty to bump the last release (`0.1.0` → `0.1.1`). The workflow runs the tests, builds the `.deb` on Ubuntu 22.04 and publishes the release with notes generated from the commits. The version only goes into the build; nothing is committed back.

Bundled libraries in `ui/vendor/`: markdown-it, markdown-it-texmath, KaTeX, highlight.js (licenses alongside).
