'use strict';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (sel, root = document) => root.querySelector(sel);

// Surface unexpected errors in the panel instead of failing silently.
window.addEventListener('error', (e) => toast(`Error: ${e.message}`));
window.addEventListener('unhandledrejection', (e) => {
  if (!(typeof e.reason === 'string')) toast(`Error: ${e.reason && e.reason.message || e.reason}`);
});

/** Small DOM builder: h('div', { class: 'x', onclick: fn }, child, 'text', ...) */
function h(tag, attrs = {}, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v == null || v === false) continue;
    if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
    else if (k === 'html') el.innerHTML = v;
    else el.setAttribute(k, v === true ? '' : v);
  }
  for (const c of children.flat()) {
    if (c != null && c !== false) el.append(c);
  }
  return el;
}

const CHECK_SVG = '<svg viewBox="0 0 24 24" width="12" height="12"><path d="M5 12.5l4.5 4.5L19 7.5" stroke="#fff" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round"/></svg>';

const state = {
  questions: [],
  expanded: new Set(),
  /** The one open editor: { id, kind: 'answer' | 'title', el, saved, title, orphan } */
  editing: null,
  filter: 'all',
  pinned: false,
  /** Text from "Use selection", saved as a quote with the next new question. */
  captureQuote: '',
  /** Where to return after following a link: [{ scroll, search, filter }] */
  backStack: [],
  /** Text size of the list (1 = 100%) and one-line-per-question mode; saved in config.json. */
  zoom: 1,
  compact: false,
};

// ---------- Markdown rendering ----------

const md = window.markdownit({
  linkify: true,
  highlight(code, lang) {
    if (lang && hljs.getLanguage(lang)) {
      const html = hljs.highlight(code, { language: lang, ignoreIllegals: true }).value;
      return `<pre class="hljs"><code class="language-${lang}">${html}</code></pre>`;
    }
    return '';
  },
});
md.use(texmath, { engine: katex, delimiters: 'dollars', katexOptions: { throwOnError: false } });
md.use(calloutPlugin);
md.use(taskListPlugin);

// Relative image paths (assets/x.png) are served from the notes folder.
const renderImage = md.renderer.rules.image;
md.renderer.rules.image = (tokens, idx, options, env, self) => {
  const token = tokens[idx];
  const src = token.attrGet('src') || '';
  if (src && !/^[a-z][a-z0-9+.-]*:/i.test(src) && !src.startsWith('//')) {
    token.attrSet('src', 'notes://localhost/' + src.replace(/^\.?\//, ''));
  }
  return renderImage(tokens, idx, options, env, self);
};

// Links to other questions are written [label](#id).
const QLINK = /^#([0-9a-z]{4,16})$/;
const renderLinkOpen = md.renderer.rules.link_open
  || ((tokens, idx, options, env, self) => self.renderToken(tokens, idx, options));
md.renderer.rules.link_open = (tokens, idx, options, env, self) => {
  const token = tokens[idx];
  const m = (token.attrGet('href') || '').match(QLINK);
  if (m) {
    const exists = state.questions.some((q) => q.id === m[1]);
    token.attrJoin('class', exists ? 'qlink' : 'qlink missing');
    token.attrSet('data-qid', m[1]);
    if (!exists) token.attrSet('title', 'The linked question was deleted');
  }
  return renderLinkOpen(tokens, idx, options, env, self);
};

// Question titles can contain the same links; everything else in a title is plain text.
const TITLE_LINK = /\[((?:\\.|[^\]\\])*)\]\(#([0-9a-z]{4,16})\)/g;
const unescapeLabel = (label) => label.replace(/\\(.)/g, '$1');

/** A title with its `[label](#id)` links reduced to their labels. */
function plainTitle(title) {
  return title.replace(TITLE_LINK, (_, label) => unescapeLabel(label));
}

/** A title as DOM nodes, with question links made clickable. */
function renderTitle(title) {
  const nodes = [];
  let last = 0;
  for (const m of title.matchAll(TITLE_LINK)) {
    if (m.index > last) nodes.push(title.slice(last, m.index));
    const exists = state.questions.some((q) => q.id === m[2]);
    nodes.push(h('a', {
      class: exists ? 'qlink' : 'qlink missing',
      href: '#' + m[2],
      'data-qid': m[2],
      title: exists ? null : 'The linked question was deleted',
    }, unescapeLabel(m[1])));
    last = m.index + m[0].length;
  }
  if (last < title.length) nodes.push(title.slice(last));
  return nodes;
}

function renderMarkdown(text) {
  return h('div', { class: 'md', html: md.render(text) });
}

// ---------- Backend calls ----------

let toastTimer;
function toast(msg, kind = 'error') {
  const el = $('#toast');
  el.textContent = msg;
  el.className = kind;
  el.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (el.hidden = true), kind === 'error' ? 6000 : 2500);
}

async function call(cmd, args, options) {
  try {
    return await invoke(cmd, args, options);
  } catch (e) {
    toast(String(e));
    throw e;
  }
}

function replaceQuestion(q) {
  const i = state.questions.findIndex((x) => x.id === q.id);
  if (i >= 0) state.questions[i] = q;
  else state.questions.push(q);
}

async function load() {
  state.questions = await call('list_questions');
  render();
}

// ---------- Filtering ----------

function parseSearch(text) {
  const words = [];
  const tags = [];
  for (const tok of text.toLowerCase().split(/\s+/).filter(Boolean)) {
    if (tok.length > 1 && tok.startsWith('#')) tags.push(tok.slice(1));
    else words.push(tok);
  }
  return { words, tags };
}

function matches(q, { words, tags }) {
  if (!tags.every((t) => q.topic === t || q.tags.includes(t))) return false;
  if (!words.length) return true;
  const hay = [plainTitle(q.title), q.answer, q.quote, q.topic, ...q.tags].join('\n').toLowerCase();
  return words.every((w) => hay.includes(w));
}

/** Open questions first, then newest first. */
function compare(a, b) {
  if (a.status !== b.status) return a.status === 'open' ? -1 : 1;
  return b.asked.localeCompare(a.asked) || b.id.localeCompare(a.id);
}

/** Every topic and tag with how often it's used, topics first, for autocomplete. */
function hashtagIndex() {
  const counts = new Map();
  for (const q of state.questions) {
    for (const [name, isTopic] of [[q.topic, true], ...q.tags.map((t) => [t, false])]) {
      const e = counts.get(name) || { name, count: 0, topic: false };
      e.count += 1;
      e.topic ||= isTopic;
      counts.set(name, e);
    }
  }
  return [...counts.values()].sort((a, b) => b.topic - a.topic || b.count - a.count || a.name.localeCompare(b.name));
}

// ---------- Hashtag autocomplete ----------

function attachAutocomplete(input, onEnter) {
  const box = h('div', { class: 'suggest', hidden: true });
  input.after(box);
  let items = [];
  let sel = 0;

  const token = () => {
    const pos = input.selectionStart;
    const m = input.value.slice(0, pos).match(/(^|\s)#([\p{L}\p{N}_+-]*)$/u);
    return m ? { start: pos - m[2].length - 1, end: pos, prefix: m[2].toLowerCase() } : null;
  };
  const hide = () => { box.hidden = true; items = []; };
  const draw = () => {
    box.replaceChildren(...items.map((t, i) => h('div', {
      'aria-selected': String(i === sel),
      onmousedown: (e) => { e.preventDefault(); sel = i; accept(); },
    }, '#' + t.name, h('small', {}, t.topic ? 'topic' : `${t.count}`))));
    box.hidden = false;
  };
  const update = () => {
    const tok = token();
    const inLink = /\[\[[^\[\]\n]*$/.test(input.value.slice(0, input.selectionStart));
    if (!tok || inLink) return hide();
    items = hashtagIndex().filter((t) => t.name.startsWith(tok.prefix) && t.name !== tok.prefix).slice(0, 6);
    if (!items.length) return hide();
    sel = 0;
    draw();
  };
  const accept = () => {
    const tok = token();
    if (!tok || !items[sel]) return hide();
    const insert = '#' + items[sel].name + ' ';
    const rest = input.value.slice(tok.end).replace(/^\S*\s?/, '');
    input.value = input.value.slice(0, tok.start) + insert + rest;
    const caret = tok.start + insert.length;
    input.setSelectionRange(caret, caret);
    hide();
  };

  input.addEventListener('input', update);
  input.addEventListener('blur', () => setTimeout(hide, 100));
  input.addEventListener('keydown', (e) => {
    if (!box.hidden) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        sel = (sel + (e.key === 'ArrowDown' ? 1 : items.length - 1)) % items.length;
        draw();
      } else if (e.key === 'Tab' || e.key === 'Enter') {
        accept();
      } else if (e.key === 'Escape') {
        hide();
      } else {
        return;
      }
      e.preventDefault();
      e.stopImmediatePropagation();
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      onEnter();
    }
  });
}

// ---------- Finding questions to link to ----------

/** Questions whose titles contain every word of `query`, best matches first. */
function findQuestions(query, excludeId) {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const q0 = query.trim().toLowerCase();
  return state.questions
    .filter((q) => q.id !== excludeId)
    .map((q) => {
      const title = plainTitle(q.title).toLowerCase();
      if (!words.every((w) => title.includes(w))) return null;
      const score = !q0 ? 0 : title.startsWith(q0) ? 0 : title.includes(q0) ? 1 : 2;
      return { q, score };
    })
    .filter(Boolean)
    .sort((a, b) => a.score - b.score || b.q.id.localeCompare(a.q.id))
    .map((x) => x.q);
}

// ---------- Rendering ----------

function formatDate(iso) {
  const d = new Date(iso + 'T00:00:00');
  if (isNaN(d)) return iso;
  const sameYear = d.getFullYear() === new Date().getFullYear();
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: sameYear ? undefined : 'numeric' });
}

function addToSearch(tag) {
  const search = $('#search');
  const tokens = search.value.split(/\s+/).filter(Boolean);
  if (!tokens.includes('#' + tag)) tokens.push('#' + tag);
  search.value = tokens.join(' ') + ' ';
  render();
}

function renderQuestion(q) {
  const editing = state.editing && state.editing.id === q.id ? state.editing : null;
  const expanded = state.expanded.has(q.id) || !!editing;
  const el = h('article', { class: `q ${q.status}${expanded ? ' expanded' : ''}`, 'data-id': q.id });

  const dot = h('button', {
    class: 'dot',
    title: q.status === 'open' ? 'Mark answered' : 'Mark open',
    html: CHECK_SVG,
    onclick: (e) => { e.stopPropagation(); toggleStatus(q); },
  });
  const title = editing && editing.kind === 'title'
    ? editing.el
    : h('div', { class: 'q-title', title: state.compact && !expanded ? plainTitle(q.title) : null }, renderTitle(q.title));
  el.append(h('div', { class: 'q-head' }, dot, title));

  // A click anywhere on the card expands it. Once expanded, clicks inside the
  // answer don't collapse it, so text can be selected and double-clicked.
  el.addEventListener('click', (e) => {
    if (editing || e.target.closest('button, a, input, textarea')) return;
    if (expanded && e.target.closest('.q-body')) return;
    if (!window.getSelection().isCollapsed) return;
    toggleExpand(q.id);
  });

  if (!(editing && editing.kind === 'title')) {
    el.append(h('div', { class: 'q-meta' },
      h('button', { class: 'chip topic', title: 'Filter by topic', onclick: () => addToSearch(q.topic) }, q.topic),
      q.tags.map((t) => h('button', { class: 'chip', title: 'Filter by tag', onclick: () => addToSearch(t) }, '#' + t)),
      q.quote && !expanded ? h('span', { class: 'quote-mark', title: 'Has a captured quote' }, '❝') : null,
      h('span', { class: 'date' }, formatDate(q.asked)),
    ));
  }

  if (expanded) {
    const body = h('div', { class: 'q-body' });
    if (q.quote && !(editing && editing.kind === 'title')) body.append(h('blockquote', { class: 'captured' }, q.quote));
    if (editing && editing.kind === 'answer') {
      body.append(editing.el);
    } else {
      const content = q.answer
        ? renderMarkdown(q.answer)
        : h('div', { class: 'no-answer' }, 'No answer yet. Double-click to write one.');
      body.append(content);
      body.addEventListener('dblclick', (e) => {
        if (e.target.closest('.actions, a, summary, input')) return;
        const caret = q.answer ? caretFromSelection(content, q.answer) : null;
        window.getSelection().removeAllRanges();
        editAnswer(q, caret);
      });
      body.append(h('div', { class: 'actions' },
        h('button', { class: 'btn primary', onclick: () => editAnswer(q) }, q.answer ? 'Edit answer' : 'Write answer'),
        h('button', { class: 'btn', onclick: () => editTitle(q) }, 'Edit question'),
        h('span', { class: 'spacer' }),
        h('button', { class: 'btn danger', onclick: (e) => confirmDelete(e.currentTarget, q) }, 'Delete'),
      ));
    }
    el.append(body);
  }
  return el;
}

/**
 * Best guess at where a double-clicked word sits in the raw Markdown: the same
 * occurrence (1st, 2nd, ...) of that word as in the rendered text.
 */
function caretFromSelection(container, raw) {
  const sel = window.getSelection();
  const word = sel.toString().trim();
  if (!word || !sel.rangeCount || !container.contains(sel.anchorNode)) return null;
  const before = document.createRange();
  before.setStart(container, 0);
  before.setEnd(sel.getRangeAt(0).startContainer, sel.getRangeAt(0).startOffset);
  const nth = before.toString().split(word).length - 1;
  let idx = -1;
  for (let i = 0; i <= nth; i++) {
    const next = raw.indexOf(word, idx + 1);
    if (next < 0) break;
    idx = next;
  }
  return idx >= 0 ? idx : null;
}

/** A draft whose question vanished (deleted or rewritten outside the app). */
function renderOrphan(editing) {
  return h('article', { class: 'q open expanded' },
    h('div', { class: 'q-head' }, h('div', { class: 'q-title' }, plainTitle(editing.title))),
    h('div', { class: 'orphan-note' }, 'This question was changed or removed outside QuestionBar. Copy your draft before closing it.'),
    h('div', { class: 'q-body' }, editing.el),
  );
}

function render() {
  const list = $('#list');
  const scroll = list.scrollTop;
  // Moving the editor between renders loses focus and selection; restore them after.
  const active = document.activeElement;
  const keepFocus = state.editing && state.editing.el.contains(active) ? active : null;
  const selection = keepFocus && 'selectionStart' in keepFocus ? [keepFocus.selectionStart, keepFocus.selectionEnd] : null;
  const innerScroll = keepFocus ? keepFocus.scrollTop : 0;

  const search = parseSearch($('#search').value);
  const counts = { all: 0, open: 0, answered: 0 };
  const shown = [];
  for (const q of state.questions) {
    if (!matches(q, search) && !(state.editing && state.editing.id === q.id)) continue;
    counts.all += 1;
    counts[q.status] += 1;
    if (state.filter === 'all' || q.status === state.filter || (state.editing && state.editing.id === q.id)) shown.push(q);
  }
  shown.sort(compare);
  for (const k of Object.keys(counts)) $('#count-' + k).textContent = counts[k] || '';

  const items = shown.map(renderQuestion);
  if (state.editing && !state.questions.some((q) => q.id === state.editing.id)) {
    state.editing.orphan = true;
    items.unshift(renderOrphan(state.editing));
  }
  if (!items.length) {
    items.push(h('div', { class: 'empty' }, state.questions.length
      ? 'Nothing matches.'
      : 'No questions yet. Type one above and press Enter — end it with #topic to file it.'));
  }
  list.replaceChildren(...items);
  list.scrollTop = scroll;

  if (keepFocus) {
    keepFocus.focus({ preventScroll: true });
    if (selection) keepFocus.setSelectionRange(...selection);
    keepFocus.scrollTop = innerScroll;
  }
}

// ---------- Actions ----------

function toggleExpand(id) {
  if (state.expanded.has(id)) state.expanded.delete(id);
  else state.expanded.add(id);
  render();
}

async function toggleStatus(q) {
  const status = q.status === 'open' ? 'answered' : 'open';
  replaceQuestion(await call('set_status', { id: q.id, status }));
  render();
}

async function addQuestion() {
  const input = $('#new');
  const text = input.value.trim();
  if (!text) return;
  const q = await call('add_question', { text, quote: state.captureQuote });
  input.value = '';
  setCapture('');
  replaceQuestion(q);
  render();
  const el = $(`.q[data-id="${q.id}"]`);
  if (el) {
    el.classList.add('flash');
    el.scrollIntoView({ block: 'nearest' });
  }
}

function confirmDelete(button, q) {
  if (!button.classList.contains('confirm')) {
    button.classList.add('confirm');
    button.textContent = 'Click again to delete';
    setTimeout(() => {
      button.classList.remove('confirm');
      button.textContent = 'Delete';
    }, 3000);
    return;
  }
  call('delete_question', { id: q.id }).then(() => {
    state.questions = state.questions.filter((x) => x.id !== q.id);
    state.expanded.delete(q.id);
    render();
  });
}

function setEditing(editing) {
  state.editing = editing;
  invoke('set_busy', { busy: !!editing });
}

// Saves are chained so they reach the backend in order.
let saveChain = Promise.resolve();
let autosaveTimer;

function saveAnswer() {
  clearTimeout(autosaveTimer);
  const editing = state.editing;
  if (!editing || editing.kind !== 'answer' || editing.orphan) return saveChain;
  const answer = editing.textarea.value;
  if (answer === editing.saved) return saveChain;
  editing.saved = answer;
  saveChain = saveChain
    .then(() => call('set_answer', { id: editing.id, answer }))
    .then((q) => {
      const before = state.questions.find((x) => x.id === q.id);
      const statusChanged = before && before.status !== q.status;
      replaceQuestion(q);
      // The first answer flips the status; re-render only then, to avoid disturbing the editor.
      if (statusChanged && state.editing === editing) render();
    })
    .catch(() => { editing.saved = null; });
  return saveChain;
}

async function closeEditor() {
  if (!state.editing) return;
  await saveAnswer();
  setEditing(null);
  render();
}

async function editAnswer(q, caret = null) {
  await closeEditor();
  const textarea = h('textarea', { spellcheck: 'true', placeholder: 'Write your answer in Markdown…  / for blocks, $math$ (\\ for symbols), [[ to link a question, paste or drop images' });
  textarea.value = q.answer;
  const preview = h('div', { class: 'preview', hidden: true });
  const writeTab = h('button', { class: 'seg', 'aria-selected': 'true' }, 'Write');
  const previewTab = h('button', { class: 'seg', 'aria-selected': 'false' }, 'Preview');
  const showPreview = (on) => {
    writeTab.setAttribute('aria-selected', String(!on));
    previewTab.setAttribute('aria-selected', String(on));
    textarea.hidden = on;
    preview.hidden = !on;
    if (on) preview.replaceChildren(renderMarkdown(textarea.value || '*Nothing yet.*'));
    else textarea.focus();
  };
  writeTab.onclick = () => showPreview(false);
  previewTab.onclick = () => showPreview(true);

  const el = h('div', { class: 'editor' },
    h('div', { class: 'tabs' }, writeTab, previewTab),
    textarea,
    preview,
    h('div', { class: 'actions' },
      h('button', { class: 'btn primary', onclick: closeEditor }, 'Done'),
      h('span', { class: 'hint' }, 'Ctrl+Enter done · Ctrl+P preview · / blocks · [[ link · autosaves'),
    ),
  );

  // Menus first: their Enter/Tab/Esc handling must win over the editor's own keys.
  const menus = [
    attachLinkAutocomplete(textarea, q.id, el),
    attachSlashMenu(textarea, el, insertImagesFromPicker),
    attachLatexAutocomplete(textarea, el),
  ];
  attachListContinuation(textarea);
  attachMathPreview(textarea, el, menus);
  textarea.addEventListener('input', () => {
    autosize(textarea);
    clearTimeout(autosaveTimer);
    autosaveTimer = setTimeout(saveAnswer, 700);
  });
  textarea.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && e.ctrlKey) {
      e.preventDefault();
      closeEditor();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      closeEditor();
    } else if (e.key === 'Tab' && !e.shiftKey) {
      e.preventDefault();
      insertAtCursor(textarea, '  ');
    }
  });
  el.addEventListener('keydown', (e) => {
    if (e.key.toLowerCase() === 'p' && e.ctrlKey) {
      e.preventDefault();
      showPreview(textarea.hidden === false);
    } else if (e.key === 'Escape' && textarea.hidden) {
      e.stopPropagation();
      closeEditor();
    }
  });
  textarea.addEventListener('paste', onPaste);

  setEditing({ id: q.id, kind: 'answer', el, textarea, saved: q.answer, title: q.title });
  state.expanded.add(q.id);
  render();
  autosize(textarea);
  textarea.focus();
  const pos = caret ?? textarea.value.length;
  textarea.setSelectionRange(pos, pos);
  // Scroll the textarea so the caret's line is visible.
  const line = textarea.value.slice(0, pos).split('\n').length - 1;
  const lineHeight = parseFloat(getComputedStyle(textarea).lineHeight) || 20;
  textarea.scrollTop = Math.max(0, line * lineHeight - textarea.clientHeight / 3);
  textarea.closest('.q').scrollIntoView({ block: 'nearest' });
}

async function editTitle(q) {
  await closeEditor();
  const input = h('input', { type: 'text', spellcheck: 'true' });
  input.value = [q.title, '#' + q.topic, ...q.tags.map((t) => '#' + t)].join(' ');
  const quoteBox = h('textarea', { class: 'quote-edit', rows: '2', placeholder: 'Quote (optional)', spellcheck: 'true' });
  quoteBox.value = q.quote;
  const wrap = h('div', { class: 'title-edit', onclick: (e) => e.stopPropagation() },
    h('div', { class: 'field' }, input), quoteBox,
    h('div', { class: 'hint' }, 'Enter save · Esc cancel · first #tag is the topic · [[ link'));
  const save = async () => {
    const updated = await call('edit_question', { id: q.id, text: input.value, quote: quoteBox.value });
    replaceQuestion(updated);
    setEditing(null);
    render();
  };
  const cancel = (e) => {
    if (e.key === 'Escape') {
      e.stopPropagation();
      setEditing(null);
      render();
    }
  };
  attachLinkAutocomplete(input, q.id, wrap);
  attachAutocomplete(input, save);
  input.addEventListener('keydown', cancel);
  quoteBox.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && e.ctrlKey) {
      e.preventDefault();
      save();
    } else {
      cancel(e);
    }
  });
  quoteBox.addEventListener('input', () => autosize(quoteBox));
  setEditing({ id: q.id, kind: 'title', el: wrap, title: q.title });
  render();
  autosize(quoteBox);
  input.focus();
}

function autosize(textarea) {
  textarea.style.height = 'auto';
  textarea.style.height = Math.min(textarea.scrollHeight + 2, window.innerHeight * 0.6) + 'px';
}

function insertAtCursor(textarea, text) {
  const { selectionStart: s, selectionEnd: e, value } = textarea;
  textarea.value = value.slice(0, s) + text + value.slice(e);
  textarea.setSelectionRange(s + text.length, s + text.length);
  textarea.dispatchEvent(new Event('input'));
}

// ---------- Images ----------

function insertImage(path) {
  const editing = state.editing;
  if (!editing || editing.kind !== 'answer') return;
  const ta = editing.textarea;
  const before = ta.value.slice(0, ta.selectionStart);
  const pad = before && !before.endsWith('\n') ? '\n' : '';
  insertAtCursor(ta, `${pad}![](${path})\n`);
}

async function insertImagesFromPicker() {
  const editing = state.editing;
  const paths = await call('pick_images');
  for (const path of paths) insertImage(await call('import_image', { path }));
  if (editing && state.editing === editing) editing.textarea.focus();
}

async function onPaste(e) {
  const item = [...e.clipboardData.items].find((i) => i.kind === 'file' && i.type.startsWith('image/'));
  if (!item) return;
  e.preventDefault();
  const file = item.getAsFile();
  const ext = (file.type.split('/')[1] || 'png').replace('jpeg', 'jpg').replace('svg+xml', 'svg');
  const bytes = new Uint8Array(await file.arrayBuffer());
  insertImage(await call('save_image', bytes, { headers: { 'x-ext': ext } }));
}

window.__TAURI__.webview.getCurrentWebview().onDragDropEvent(async (event) => {
  const { type } = event.payload;
  const editing = state.editing && state.editing.kind === 'answer' ? state.editing : null;
  if (editing) editing.el.classList.toggle('dropping', type === 'enter' || type === 'over');
  if (type !== 'drop') return;
  if (!editing) {
    toast('Open an answer for editing, then drop images onto it.', 'info');
    return;
  }
  for (const path of event.payload.paths) {
    insertImage(await call('import_image', { path }));
  }
});

// ---------- Capture from selection ----------

function setCapture(text) {
  state.captureQuote = text;
  $('#capture').hidden = !text;
  $('#capture-text').textContent = text;
}

async function useSelection() {
  const text = (await call('read_selection')).trim();
  if (!text) {
    toast('Nothing is highlighted or copied right now.', 'info');
    return;
  }
  setCapture(text);
  $('#new').focus();
}

// ---------- Following question links ----------

function setFilter(filter) {
  state.filter = filter;
  for (const s of document.querySelectorAll('.filters .seg')) s.setAttribute('aria-selected', String(s.dataset.filter === filter));
}

function jumpTo(id) {
  const target = state.questions.find((q) => q.id === id);
  if (!target) return;
  hidePeek();
  const search = $('#search');
  state.backStack.push({ scroll: $('#list').scrollTop, search: search.value, filter: state.filter });
  // Clear whatever would hide the target.
  if (!matches(target, parseSearch(search.value))) search.value = '';
  if (state.filter !== 'all' && target.status !== state.filter) setFilter('all');
  state.expanded.add(id);
  render();
  const el = $(`.q[data-id="${id}"]`);
  if (el) {
    el.scrollIntoView({ block: 'start' });
    el.classList.add('flash');
  }
  $('#back').hidden = false;
}

function goBack() {
  const prev = state.backStack.pop();
  if (!prev) return;
  hidePeek();
  $('#search').value = prev.search;
  setFilter(prev.filter);
  render();
  $('#list').scrollTop = prev.scroll;
  $('#back').hidden = state.backStack.length === 0;
}

// Hovering a question link shows a preview of that question and its answer.
let peekShowTimer;
let peekHideTimer;

function hidePeek() {
  clearTimeout(peekShowTimer);
  $('#peek').hidden = true;
  $('#peek').dataset.qid = '';
}

function showPeek(link) {
  const q = state.questions.find((x) => x.id === link.dataset.qid);
  if (!q || !document.body.contains(link)) return;
  const peek = $('#peek');
  peek.dataset.qid = q.id;
  peek.replaceChildren(
    h('div', { class: 'peek-head' }, h('i', { class: `mini-dot ${q.status}` }), h('strong', {}, renderTitle(q.title))),
    h('div', { class: 'peek-meta' }, [q.topic, ...q.tags.map((t) => '#' + t)].join('  ')),
    q.answer ? renderMarkdown(q.answer) : h('div', { class: 'no-answer' }, 'No answer yet.'),
    h('div', { class: 'peek-foot' }, 'Click the link to jump there'),
  );
  peek.hidden = false;
  const r = link.getBoundingClientRect();
  const width = Math.min(360, window.innerWidth - 16);
  peek.style.width = width + 'px';
  peek.style.left = Math.max(8, Math.min(r.left, window.innerWidth - width - 8)) + 'px';
  const below = r.bottom + 6;
  const top = below + peek.offsetHeight > window.innerHeight - 8 ? r.top - 6 - peek.offsetHeight : below;
  peek.style.top = Math.max(8, top) + 'px';
}

document.addEventListener('mouseover', (e) => {
  const link = e.target.closest('a.qlink:not(.missing)');
  if (link && !link.closest('#peek')) {
    clearTimeout(peekHideTimer);
    if ($('#peek').dataset.qid === link.dataset.qid && !$('#peek').hidden) return;
    clearTimeout(peekShowTimer);
    peekShowTimer = setTimeout(() => showPeek(link), 350);
  } else if (e.target.closest('#peek')) {
    clearTimeout(peekHideTimer);
  } else {
    clearTimeout(peekShowTimer);
    if (!$('#peek').hidden) {
      clearTimeout(peekHideTimer);
      peekHideTimer = setTimeout(hidePeek, 250);
    }
  }
});

// ---------- Text size and compact mode ----------

let saveUiTimer;
function saveUiSettings() {
  clearTimeout(saveUiTimer);
  saveUiTimer = setTimeout(() => invoke('save_ui_settings', { zoom: state.zoom, compact: state.compact }), 400);
}

function applyZoom(zoom) {
  state.zoom = Math.round(Math.min(2.5, Math.max(0.7, zoom)) * 10) / 10;
  document.documentElement.style.setProperty('--zoom', state.zoom);
  if (state.editing && state.editing.textarea) autosize(state.editing.textarea);
}

/** step: +0.1 / -0.1, or 0 to reset to 100%. */
function changeZoom(step) {
  const before = state.zoom;
  applyZoom(step === 0 ? 1 : state.zoom + step);
  if (state.zoom !== before) saveUiSettings();
}

function applyCompact(on) {
  state.compact = on;
  document.body.classList.toggle('compact', on);
  $('#compact').setAttribute('aria-pressed', String(on));
}

// ---------- Checklists ----------

/** Ticks or unticks the checklist item on `line` of a question's answer. */
async function toggleTask(id, line) {
  const q = state.questions.find((x) => x.id === id);
  if (!q || line < 0) return;
  const lines = q.answer.split('\n');
  const item = /^([ \t]*(?:>[ \t]?)*[ \t]*(?:[-*+]|\d+[.)])[ \t]+)\[([ xX])\]/;
  if (!item.test(lines[line] || '')) return;
  lines[line] = lines[line].replace(item, (_, prefix, mark) => `${prefix}[${mark === ' ' ? 'x' : ' '}]`);
  replaceQuestion(await call('set_answer', { id, answer: lines.join('\n') }));
  render();
}

document.addEventListener('click', (e) => {
  const box = e.target.closest('input.task');
  if (!box) return;
  // The saved answer decides the state; ticking only works on a card's answer, not in previews.
  e.preventDefault();
  const card = box.closest('.q');
  if (card && card.dataset.id && !box.closest('.editor, #peek')) toggleTask(card.dataset.id, Number(box.dataset.line));
});

// ---------- Panel wiring ----------

async function applyTheme() {
  const theme = await invoke('system_theme');
  document.documentElement.dataset.theme = theme;
  $('#hljs-theme').href = `vendor/hljs/${theme === 'dark' ? 'github-dark' : 'github'}.min.css`;
}

// Link suggestions first, so Enter picks a link instead of submitting the question.
attachLinkAutocomplete($('#new'), null, $('#new').parentElement);
attachAutocomplete($('#new'), addQuestion);

$('#search').addEventListener('input', render);
$('#search').addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && e.target.value) {
    e.stopPropagation();
    e.target.value = '';
    render();
  }
});

for (const seg of document.querySelectorAll('.filters .seg')) {
  seg.addEventListener('click', () => {
    setFilter(seg.dataset.filter);
    render();
  });
}

$('#use-sel').addEventListener('click', useSelection);
$('#capture-clear').addEventListener('click', () => {
  setCapture('');
  $('#new').focus();
});
$('#new').addEventListener('keydown', (e) => {
  if (e.code === 'KeyV' && e.ctrlKey && e.shiftKey) {
    e.preventDefault();
    useSelection();
  }
});
$('#back').addEventListener('click', goBack);
$('#compact').addEventListener('click', () => {
  applyCompact(!state.compact);
  saveUiSettings();
  render();
});

// Ctrl+scroll changes the text size, one step per notch (touchpads send many small events).
let lastWheelZoom = 0;
document.addEventListener('wheel', (e) => {
  if (!e.ctrlKey) return;
  e.preventDefault();
  if (e.timeStamp - lastWheelZoom < 90) return;
  lastWheelZoom = e.timeStamp;
  changeZoom(e.deltaY < 0 ? 0.1 : -0.1);
}, { passive: false });
$('#list').addEventListener('scroll', hidePeek, { passive: true });

$('#pin').addEventListener('click', () => {
  state.pinned = !state.pinned;
  $('#pin').setAttribute('aria-pressed', String(state.pinned));
  invoke('set_pinned', { pinned: state.pinned });
});

$('#grip').addEventListener('mousedown', (e) => {
  e.preventDefault();
  window.__TAURI__.window.getCurrentWindow().startResizeDragging('West');
});

// Question links jump inside the panel; other links open in the browser.
document.addEventListener('click', (e) => {
  const a = e.target.closest('a.qlink, .md a[href]');
  if (!a) return;
  e.preventDefault();
  if (a.classList.contains('qlink')) {
    if (!a.classList.contains('missing')) jumpTo(a.dataset.qid);
  } else if (!a.getAttribute('href').startsWith('#')) {
    call('open_external', { url: a.href });
  }
});

document.addEventListener('keydown', (e) => {
  const inField = e.target instanceof Element && e.target.matches('input, textarea');
  if (e.key === 'Escape') {
    if (!$('#peek').hidden) hidePeek();
    else if (state.editing && state.editing.kind === 'answer') closeEditor();
    else invoke('hide_panel');
  } else if (e.key === 'ArrowLeft' && e.altKey) {
    e.preventDefault();
    goBack();
  } else if (e.ctrlKey && !e.altKey && (e.key === '=' || e.key === '+')) {
    e.preventDefault();
    changeZoom(0.1);
  } else if (e.ctrlKey && !e.altKey && e.key === '-') {
    e.preventDefault();
    changeZoom(-0.1);
  } else if (e.ctrlKey && !e.altKey && e.key === '0') {
    e.preventDefault();
    changeZoom(0);
  } else if ((e.key === 'f' && e.ctrlKey) || (e.key === '/' && !inField)) {
    e.preventDefault();
    $('#search').focus();
    $('#search').select();
  } else if (e.key === 'n' && e.ctrlKey) {
    e.preventDefault();
    $('#new').focus();
  }
});

listen('panel-shown', () => {
  applyTheme();
  load();
  $('#new').focus();
});
listen('panel-hiding', () => {
  hidePeek();
  saveAnswer();
});
listen('notes-changed', () => load());

applyTheme();
invoke('ui_settings').then((s) => {
  applyZoom(s.zoom || 1);
  applyCompact(!!s.compact);
  render();
});
load();
$('#new').focus();
