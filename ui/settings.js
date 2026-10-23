'use strict';

// The Settings page (⚙): shortcut, notes folder, where the panel opens, theme,
// text size, start on login, the welcome topic, keyboard keys and about.

const REPO_URL = 'https://github.com/Entropy-rgb/QuestionBar';

/** Keys the shortcut can use: [GNOME key name, label]. */
const HOTKEY_KEYS = [
  ['Above_Tab', 'Key above Tab (any layout)'], ['grave', '` (backtick)'], ['space', 'Space'],
  ...'abcdefghijklmnopqrstuvwxyz'.split('').map((c) => [c, c.toUpperCase()]),
  ...'0123456789'.split('').map((c) => [c, c]),
  ...Array.from({ length: 12 }, (_, i) => [`F${i + 1}`, `F${i + 1}`]),
  ['slash', '/'], ['period', '.'], ['comma', ','], ['semicolon', ';'], ['apostrophe', "'"],
  ['backslash', '\\'], ['bracketleft', '['], ['bracketright', ']'], ['Insert', 'Insert'], ['Pause', 'Pause'],
];
/** [GNOME modifier name, label], in the order they're written. */
const HOTKEY_MODS = [['Super', 'Super'], ['Primary', 'Ctrl'], ['Alt', 'Alt'], ['Shift', 'Shift']];

function parseHotkey(accel) {
  const mods = new Set();
  let rest = accel || '';
  let m;
  while ((m = rest.match(/^<([^>]+)>/))) {
    const name = m[1].toLowerCase();
    if (name === 'super') mods.add('Super');
    else if (['primary', 'control', 'ctrl'].includes(name)) mods.add('Primary');
    else if (name === 'alt') mods.add('Alt');
    else if (name === 'shift') mods.add('Shift');
    rest = rest.slice(m[0].length);
  }
  const known = HOTKEY_KEYS.find(([k]) => k.toLowerCase() === rest.toLowerCase());
  return { mods, key: known ? known[0] : rest };
}

function formatHotkey({ mods, key }) {
  return HOTKEY_MODS.filter(([m]) => mods.has(m)).map(([m]) => `<${m}>`).join('') + key;
}

/** "Super" is the Windows key on Windows. */
function modLabel(mod, label) {
  return mod === 'Super' && state.platform === 'windows' ? 'Win' : label;
}

function describeHotkey(accel) {
  const { mods, key } = parseHotkey(accel);
  const keyLabel = (HOTKEY_KEYS.find(([k]) => k === key) || [key, key])[1].replace(/ \(.*\)$/, '');
  return [...HOTKEY_MODS.filter(([m]) => mods.has(m)).map(([m, label]) => modLabel(m, label)), keyLabel].join(' + ');
}

let settings = null;

async function openSettings() {
  settings = await call('get_settings');
  state.settingsOpen = true;
  hidePeek();
  renderSettings();
  $('#settings').hidden = false;
  $('#settings-btn').setAttribute('aria-pressed', 'true');
  $('#settings').focus({ preventScroll: true });
}

function closeSettings() {
  if (!state.settingsOpen) return;
  state.settingsOpen = false;
  $('#settings').hidden = true;
  $('#settings-btn').setAttribute('aria-pressed', 'false');
  $('#new').focus();
}

/** A row of buttons where one is chosen: [[value, label], ...]. */
function segmented(options, current, onPick) {
  return h('div', { class: 'seg-group', role: 'radiogroup' },
    options.map(([value, label]) => h('button', {
      class: 'seg',
      role: 'radio',
      'aria-checked': String(value === current),
      onclick: () => onPick(value),
    }, label)));
}

function toggleSwitch(on, label, onChange) {
  return h('button', {
    class: 'switch',
    role: 'switch',
    'aria-checked': String(on),
    'aria-label': label,
    onclick: () => onChange(!on),
  }, h('span'));
}

function row(label, control, hint) {
  return h('div', { class: 'set-row' },
    h('div', { class: 'set-label' }, label, hint ? h('div', { class: 'set-hint' }, hint) : null),
    control);
}

function section(title, ...children) {
  return h('section', { class: 'set-section' }, h('h3', {}, title), ...children);
}

function renderSettings() {
  $('#settings').replaceChildren(
    h('div', { class: 'settings-head' },
      h('button', { class: 'seg back', onclick: closeSettings, title: 'Back (Esc)' }, '← Back'),
      h('h2', {}, 'Settings')),
    shortcutSection(),
    folderSection(),
    panelSection(),
    startupSection(),
    updatesSection(),
    keysSection(),
    aboutSection(),
  );
}

// ---------- Shortcut ----------

function shortcutSection() {
  if (!settings.can_set_shortcut) {
    return section('Shortcut',
      h('p', { class: 'set-text' },
        'Your desktop handles keyboard shortcuts itself. In its keyboard settings, add a shortcut that runs ',
        h('code', {}, 'questionbar --toggle'), '.'),
      h('button', { class: 'btn', onclick: () => call('open_external', { url: `${REPO_URL}#other-desktops` }) },
        'How to, for KDE, Sway, Hyprland and others'));
  }
  const draft = parseHotkey(settings.hotkey);
  const status = h('div', { class: 'set-hint' });
  const useButton = h('button', { class: 'btn primary', disabled: true }, 'Use this shortcut');
  const changed = () => {
    useButton.disabled = formatHotkey(draft) === settings.hotkey;
    status.textContent = '';
  };
  const modButtons = HOTKEY_MODS.map(([mod, label]) => {
    const b = h('button', { class: 'chip-toggle', 'aria-pressed': String(draft.mods.has(mod)) }, modLabel(mod, label));
    b.onclick = () => {
      if (draft.mods.has(mod)) draft.mods.delete(mod);
      else draft.mods.add(mod);
      b.setAttribute('aria-pressed', String(draft.mods.has(mod)));
      changed();
    };
    return b;
  });
  const keySelect = h('select', { 'aria-label': 'Key' },
    HOTKEY_KEYS.some(([k]) => k === draft.key) ? null : h('option', { value: draft.key }, draft.key),
    HOTKEY_KEYS.map(([k, label]) => h('option', { value: k, selected: k === draft.key }, label)));
  keySelect.onchange = () => {
    draft.key = keySelect.value;
    changed();
  };

  const apply = async (hotkey) => {
    await call('set_hotkey', { hotkey });
    settings.hotkey = hotkey;
    toast(`Shortcut is now ${describeHotkey(hotkey)}`, 'info');
    renderSettings();
  };
  useButton.onclick = async () => {
    const hotkey = formatHotkey(draft);
    const clashes = await call('hotkey_conflicts', { hotkey });
    if (!clashes.length) return apply(hotkey);
    status.replaceChildren(
      `GNOME already uses ${describeHotkey(hotkey)} for: ${clashes.join(', ')}. `,
      h('button', { class: 'btn', onclick: () => apply(hotkey) }, 'Use it anyway'));
  };

  return section('Shortcut',
    row('Open and close the panel', h('div', { class: 'hotkey-now' }, describeHotkey(settings.hotkey))),
    h('div', { class: 'hotkey-edit' }, h('div', { class: 'chips' }, modButtons), keySelect, useButton),
    status);
}

// ---------- Notes folder ----------

function folderSection() {
  const choice = h('div', { class: 'set-choice', hidden: true });
  const change = async () => {
    const path = await withDialog(() => call('pick_folder'));
    if (!path || path === settings.notes_dir) return;
    const count = state.questions.length;
    const switchTo = async (moveNotes) => {
      const moved = await call('set_notes_dir', { path, moveNotes });
      toast(moveNotes ? `Moved ${moved} questions to ${path}` : `Now using ${path}`, 'info');
      settings = await call('get_settings');
      renderSettings();
    };
    if (!count) return switchTo(false);
    choice.replaceChildren(
      h('div', {}, `Move your ${count} question${count === 1 ? '' : 's'} to `, h('code', {}, path), ', or switch and leave them where they are?'),
      h('div', { class: 'actions' },
        h('button', { class: 'btn primary', onclick: () => switchTo(true) }, 'Move them'),
        h('button', { class: 'btn', onclick: () => switchTo(false) }, 'Just switch'),
        h('button', { class: 'btn', onclick: () => { choice.hidden = true; } }, 'Cancel')));
    choice.hidden = false;
  };
  return section('Notes folder',
    h('div', { class: 'folder-path' }, settings.notes_dir),
    h('div', { class: 'actions' }, h('button', { class: 'btn', onclick: change }, 'Change…')),
    choice,
    h('div', { class: 'set-hint' }, 'Point it at your Obsidian vault or a synced folder to keep your notes in one place.'));
}

// ---------- Panel ----------

function panelSection() {
  const monitor = h('select', { 'aria-label': 'Screen' },
    h('option', { value: 'primary', selected: settings.monitor === 'primary' }, 'Primary screen'),
    settings.monitors.map((m) => h('option', { value: m.name, selected: settings.monitor === m.name },
      m.label + (m.primary ? ', primary' : ''))),
    h('option', { value: 'mouse', selected: settings.monitor === 'mouse' }, 'The screen with the mouse'));
  monitor.onchange = async () => {
    await call('update_settings', { monitor: monitor.value });
    settings.monitor = monitor.value;
    renderSettings();
  };
  const zoomLabel = h('span', { class: 'zoom-now' }, `${Math.round(state.zoom * 100)}%`);
  const zoomBy = (step) => {
    changeZoom(step);
    zoomLabel.textContent = `${Math.round(state.zoom * 100)}%`;
  };

  return section('Panel',
    row('Side of the screen', segmented([['left', 'Left'], ['right', 'Right']], settings.side, async (side) => {
      await call('update_settings', { side });
      settings.side = side;
      applySide(side);
      renderSettings();
    })),
    row('Open on', monitor, settings.monitor === 'mouse'
      ? 'On Wayland this can lag behind: it uses where the mouse last was over an X11 window.' : null),
    row('Theme', segmented([['system', 'System'], ['light', 'Light'], ['dark', 'Dark']], settings.theme, async (theme) => {
      await call('update_settings', { theme });
      settings.theme = theme;
      state.themePref = theme;
      applyTheme();
      renderSettings();
    })),
    row('Text size', h('div', { class: 'zoom-ctl' },
      h('button', { class: 'btn', onclick: () => zoomBy(-0.1), 'aria-label': 'Smaller' }, '−'),
      zoomLabel,
      h('button', { class: 'btn', onclick: () => zoomBy(0.1), 'aria-label': 'Larger' }, '+'),
      h('button', { class: 'btn', onclick: () => zoomBy(0) }, 'Reset')), 'Ctrl+= and Ctrl+- work anywhere in the panel.'),
    row('Compact list', toggleSwitch(state.compact, 'Compact list', (on) => {
      applyCompact(on);
      saveUiSettings();
      render();
      renderSettings();
    }), 'One line per question.'));
}

// ---------- Start on login, welcome topic ----------

function startupSection() {
  return section('Start',
    row('Start QuestionBar when I log in', toggleSwitch(settings.autostart, 'Start on login', async (on) => {
      await call('set_autostart', { on });
      settings.autostart = on;
      renderSettings();
    }), 'It starts hidden and waits for your shortcut.'),
    row('Welcome questions', h('button', {
      class: 'btn',
      onclick: async () => {
        await call('add_welcome');
        await load();
        toast('Added the welcome topic to your list', 'info');
      },
    }, 'Add them'), 'A few questions that show what QuestionBar can do.'));
}

// ---------- Keys and about ----------

function keysSection() {
  const keys = [
    [describeHotkey(settings.hotkey), 'open or close the panel'],
    ['↓ in the top boxes', 'jump into the list'],
    ['↑ ↓ or j k', 'move through questions'],
    ['Enter', 'open or close a question'],
    ['e / t', 'edit the answer / the question'],
    ['x', 'mark answered or open'],
    ['d d', 'delete'],
    ['/ or Ctrl+F', 'search'],
    ['n or Ctrl+N', 'new question'],
    ['Ctrl+Shift+V', state.platform === 'windows' ? 'quote the text you copied' : 'quote the text you highlighted'],
    ['Ctrl+Shift+W', 'write wide while editing an answer'],
    ['Alt+Left', 'back after following a link'],
    ['Esc', 'close the panel'],
  ];
  return section('Keyboard',
    h('table', { class: 'keys' }, keys.map(([k, what]) => h('tr', {}, h('td', {}, h('kbd', {}, k)), h('td', {}, what)))));
}

function updatesSection() {
  if (settings.source_build) {
    return section('Updates',
      h('p', { class: 'set-text' }, 'You built this copy from source. To update, run ',
        h('code', {}, 'git pull'), ' and ', h('code', {}, './install.sh'), ' again.'));
  }
  const status = h('div', { class: 'set-hint' });
  const checkNow = async () => {
    status.textContent = 'Checking…';
    try {
      const info = await invoke('check_update');
      status.textContent = info ? `QuestionBar ${info.version} is out.` : `You have the latest version (${settings.version}).`;
      if (info) {
        try { localStorage.removeItem('qb-dismissed-update'); } catch (e) { /* fine */ }
        showUpdate(info);
      }
    } catch (e) {
      status.textContent = `Couldn't check: ${e}`;
    }
  };
  const how = state.platform === 'windows'
    ? 'It downloads the new version when you click Restart to update.'
    : 'It tells you when a new .deb is out; you install it yourself.';
  return section('Updates',
    row('Check for updates', toggleSwitch(settings.check_updates, 'Check for updates', async (on) => {
      await call('set_check_updates', { on });
      settings.check_updates = on;
      scheduleUpdateChecks(on);
      renderSettings();
    }), `Asks GitHub for the latest version when QuestionBar starts and once a day. ${how}`),
    h('div', { class: 'actions' }, h('button', { class: 'btn', onclick: checkNow }, 'Check now')),
    status);
}

function aboutSection() {
  return section('About',
    h('p', { class: 'set-text' },
      `QuestionBar ${settings.version}. Your notes stay in plain Markdown files on this computer. `,
      'QuestionBar has no account and sends nothing anywhere.'),
    h('div', { class: 'actions' },
      h('button', { class: 'btn', onclick: () => call('open_external', { url: REPO_URL }) }, 'GitHub'),
      h('button', { class: 'btn', onclick: () => call('open_external', { url: `${REPO_URL}/issues/new` }) }, 'Report a problem')));
}

$('#settings-btn').addEventListener('click', () => (state.settingsOpen ? closeSettings() : openSettings()));
