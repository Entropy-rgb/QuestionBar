'use strict';

// Editing helpers shared by the question boxes and the answer editor: suggestion menus
// at the caret, where code and maths are, the `/` blocks, LaTeX command matching, and
// the floating maths preview for the question boxes.

/** Marks where the caret goes in inserted text. */
const CARET = '‸';

/**
 * Viewport position of the caret (mirror-element technique). For a single-line
 * input, y/lineHeight span the whole field so things placed "below" clear it.
 */
function caretCoords(ta) {
  const single = ta.tagName === 'INPUT';
  const style = getComputedStyle(ta);
  const mirror = document.createElement('div');
  for (const prop of ['boxSizing', 'width', 'paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft',
    'borderTopWidth', 'borderRightWidth', 'borderBottomWidth', 'borderLeftWidth', 'fontFamily', 'fontSize',
    'fontWeight', 'lineHeight', 'letterSpacing', 'tabSize']) {
    mirror.style[prop] = style[prop];
  }
  Object.assign(mirror.style, { position: 'absolute', visibility: 'hidden', top: '0', left: '-9999px',
    whiteSpace: single ? 'pre' : 'pre-wrap', overflowWrap: 'break-word', borderStyle: 'solid' });
  if (single) mirror.style.width = 'auto';
  mirror.textContent = ta.value.slice(0, ta.selectionStart);
  const marker = h('span', {}, '​');
  mirror.append(marker);
  document.body.append(mirror);
  const rect = ta.getBoundingClientRect();
  const x = rect.left + marker.offsetLeft - ta.scrollLeft;
  const y = single ? rect.top : rect.top + marker.offsetTop - ta.scrollTop;
  const lineHeight = single ? rect.height : marker.offsetHeight;
  mirror.remove();
  return { x, y, lineHeight };
}

/**
 * Replaces field.value[start, end) with `text`, placing the caret at ‸ (or after
 * the text). Goes through the browser's editing commands so Ctrl+Z undoes it.
 */
function replaceText(field, start, end, text) {
  const at = text.indexOf(CARET);
  const clean = text.replace(CARET, '');
  field.focus();
  field.setSelectionRange(start, end);
  if (!document.execCommand(clean ? 'insertText' : 'delete', false, clean)) {
    field.value = field.value.slice(0, start) + clean + field.value.slice(end);
    field.dispatchEvent(new Event('input'));
  }
  const caret = start + (at >= 0 ? at : clean.length);
  field.setSelectionRange(caret, caret);
  field.dispatchEvent(new Event('selectionmove'));
}

/**
 * A suggestion list attached to a text field. The provider decides when it
 * applies and what it inserts:
 *   match(field) -> { start, end, query } | null   the text being completed
 *   items(token) -> entries; render(entry) -> row content; accept(entry, token, field)
 *   empty: text shown when nothing matches (otherwise the list just hides)
 * Returns { isOpen }.
 */
function attachCaretMenu(field, container, provider) {
  // Fixed-position so it can sit at the caret; lives in `container` and goes away with it.
  const box = h('div', { class: `suggest caret-menu ${provider.className || ''}`, hidden: true });
  container.append(box);
  let items = [];
  let sel = 0;
  // Esc dismisses the list for the completion it was showing.
  let dismissedAt = -1;

  const hide = () => { box.hidden = true; items = []; };
  const place = () => {
    // Single-line inputs get the list under the whole field; textareas at the caret.
    const r = field.getBoundingClientRect();
    const c = field.tagName === 'INPUT' ? { x: r.left, y: r.top, lineHeight: r.height } : caretCoords(field);
    const width = Math.min(provider.width || 340, window.innerWidth - 16);
    box.style.width = width + 'px';
    box.style.left = Math.max(8, Math.min(c.x, window.innerWidth - width - 8)) + 'px';
    const below = c.y + c.lineHeight + 4;
    box.style.top = (below + box.offsetHeight > window.innerHeight - 8 ? Math.max(8, c.y - box.offsetHeight - 4) : below) + 'px';
  };
  const draw = () => {
    box.replaceChildren(...(items.length
      ? items.map((item, i) => h('div', {
        'aria-selected': String(i === sel),
        onmousedown: (e) => { e.preventDefault(); sel = i; accept(); },
      }, provider.render(item)))
      : [h('div', { class: 'none' }, provider.empty)]));
    box.hidden = false;
    place();
    const row = box.children[sel];
    if (row) row.scrollIntoView({ block: 'nearest' });
  };
  const update = () => {
    const token = field.hidden ? null : provider.match(field);
    if (!token) {
      dismissedAt = -1;
      return hide();
    }
    if (token.start === dismissedAt) return hide();
    items = provider.items(token);
    if (!items.length && !provider.empty) return hide();
    sel = 0;
    draw();
  };
  const accept = () => {
    const token = provider.match(field);
    const item = items[sel];
    hide();
    if (token && item) provider.accept(item, token, field);
  };

  field.addEventListener('input', update);
  field.addEventListener('click', update);
  field.addEventListener('selectionmove', update);
  field.addEventListener('blur', () => setTimeout(hide, 150));
  field.addEventListener('keyup', (e) => {
    if (['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key)) update();
  });
  field.addEventListener('keydown', (e) => {
    if (box.hidden) return;
    if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && items.length) {
      sel = (sel + (e.key === 'ArrowDown' ? 1 : items.length - 1)) % items.length;
      draw();
    } else if ((e.key === 'Enter' || e.key === 'Tab') && items.length && !e.ctrlKey) {
      accept();
    } else if (e.key === 'Escape') {
      const token = provider.match(field);
      dismissedAt = token ? token.start : -1;
      hide();
    } else {
      return;
    }
    e.preventDefault();
    e.stopImmediatePropagation();
  });
  return { isOpen: () => !box.hidden };
}

// ---------- [[ question links ----------

function attachLinkAutocomplete(field, selfId, container) {
  return attachCaretMenu(field, container, {
    className: 'link-suggest',
    empty: 'No question matches',
    match(f) {
      const pos = f.selectionStart;
      const m = f.value.slice(0, pos).match(/\[\[([^\[\]\n]*)$/);
      return m ? { start: pos - m[0].length, end: pos, query: m[1] } : null;
    },
    items: (t) => findQuestions(t.query, selfId).slice(0, 7),
    render: (q) => [
      h('span', { class: 'link-title' }, h('i', { class: `mini-dot ${q.status}` }), renderTitleText(plainTitle(q.title))),
      h('small', {}, q.topic),
    ],
    accept(q, t, f) {
      // Only brackets need escaping in a link label; LaTeX backslashes stay as they are.
      const label = plainTitle(q.title).replace(/([\[\]])/g, '\\$1');
      // Swallow a closing ]] the user may already have typed.
      const tail = f.value.slice(t.end).match(/^[^\]\n]*\]\]/);
      replaceText(f, t.start, t.end + (tail ? tail[0].length : 0), `[${label}](#${q.id})`);
    },
  });
}

// ---------- Where code and math are in the Markdown ----------

/**
 * Index ranges of fenced/inline code and of math in Markdown text.
 * math: { start, from, to, end, display, closed } where [from, to) is the TeX;
 * an unclosed `$` runs to the end of its line, an unclosed `$$` to the end of the text.
 */
function scanMarkup(text) {
  const math = [];
  const code = [];
  const escaped = (i) => {
    let n = 0;
    while (text[i - 1 - n] === '\\') n++;
    return n % 2 === 1;
  };
  let fence = null;
  let display = null;
  let lineStart = 0;
  for (const line of text.split('\n')) {
    const lineEnd = lineStart + line.length;
    if (display === null) {
      const f = line.match(/^ {0,3}(`{3,}|~{3,})/);
      if (fence) {
        if (f && f[1][0] === fence.ch && f[1].length >= fence.len && !line.slice(f[0].length).trim()) {
          code.push({ start: fence.start, end: lineEnd, closed: true });
          fence = null;
        }
        lineStart = lineEnd + 1;
        continue;
      }
      if (f) {
        fence = { ch: f[1][0], len: f[1].length, start: lineStart };
        lineStart = lineEnd + 1;
        continue;
      }
    }
    let inline = null;
    for (let i = lineStart; i < lineEnd; i++) {
      const c = text[i];
      if (display !== null) {
        if (c === '$' && text[i + 1] === '$' && !escaped(i)) {
          math.push({ start: display, from: display + 2, to: i, end: i + 2, display: true, closed: true });
          display = null;
          i++;
        }
        continue;
      }
      if (inline !== null) {
        // Same rules as the renderer: no space before the closing $, no digit after it.
        if (c === '$' && !escaped(i) && !/\s/.test(text[i - 1]) && !/\d/.test(text[i + 1] || '')) {
          math.push({ start: inline, from: inline + 1, to: i, end: i + 1, display: false, closed: true });
          inline = null;
        }
        continue;
      }
      if (c === '`') {
        let run = 1;
        while (text[i + run] === '`') run++;
        let j = i + run;
        let close = -1;
        while (j < lineEnd) {
          if (text[j] !== '`') { j++; continue; }
          let r = 1;
          while (text[j + r] === '`') r++;
          if (r === run) { close = j; break; }
          j += r;
        }
        if (close >= 0) {
          code.push({ start: i, end: close + run, closed: true });
          i = close + run - 1;
        } else {
          i += run - 1;
        }
        continue;
      }
      if (c === '$' && !escaped(i)) {
        if (text[i + 1] === '$') {
          display = i;
          i++;
        } else if (i + 1 < lineEnd && !/\s/.test(text[i + 1])) {
          inline = i;
        } else {
          // `$ x $`: padded with spaces on both sides (the renderer accepts this too).
          const m = /^\$[ \t]+[^$\s](?:[^$\n]*?[^$\s])?[ \t]+\$(?!\d)/.exec(text.slice(i, lineEnd));
          if (m) {
            math.push({ start: i, from: i + 1, to: i + m[0].length - 1, end: i + m[0].length, display: false, closed: true });
            i += m[0].length - 1;
          }
        }
      }
    }
    if (inline !== null) math.push({ start: inline, from: inline + 1, to: lineEnd, end: lineEnd, display: false, closed: false });
    lineStart = lineEnd + 1;
  }
  if (display !== null) math.push({ start: display, from: display + 2, to: text.length, end: text.length, display: true, closed: false });
  if (fence) code.push({ start: fence.start, end: text.length, closed: false });
  return { math, code };
}

/** The math the caret at `pos` is in: { tex, display, start, ... } or null. */
function mathAt(text, pos) {
  const m = scanMarkup(text).math.find((r) => pos >= r.from && pos <= r.to);
  return m ? { ...m, tex: text.slice(m.from, m.to) } : null;
}

function inCode(scan, pos) {
  return scan.code.some((c) => pos > c.start && (pos < c.end || !c.closed));
}

// ---------- / slash menu ----------

const SLASH_BLOCKS = [
  { label: 'Heading', icon: 'H', keys: 'title h3', snippet: `### ${CARET}`, block: true },
  { label: 'Bullet list', icon: '•', keys: 'ul unordered', snippet: `- ${CARET}`, block: true },
  { label: 'Numbered list', icon: '1.', keys: 'ol ordered', snippet: `1. ${CARET}`, block: true },
  { label: 'Checklist', icon: '☐', keys: 'todo task checkbox', snippet: `- [ ] ${CARET}`, block: true },
  { label: 'Code block', icon: '{ }', keys: 'snippet program', snippet: `\`\`\`\n${CARET}\n\`\`\``, block: true, spaced: true },
  { label: 'Math block', icon: '∑', keys: 'equation latex formula display', snippet: `$$\n${CARET}\n$$`, block: true, spaced: true },
  { label: 'Inline math', icon: '$', keys: 'equation latex formula', snippet: `$${CARET}$` },
  { label: 'Table', icon: '▦', keys: 'grid columns rows', snippet: `| ${CARET} |  |  |\n| --- | --- | --- |\n|  |  |  |`, block: true, spaced: true },
  { label: 'Divider', icon: '—', keys: 'hr line rule separator', snippet: `---\n${CARET}`, block: true, spaced: true },
  { label: 'Quote', icon: '❝', keys: 'blockquote', snippet: `> ${CARET}`, block: true },
  { label: 'Link to question', icon: '↗', keys: 'question reference [[', snippet: '[[' },
  { label: 'Note', icon: 'ⓘ', keys: 'callout info', snippet: `> [!note]\n> ${CARET}`, block: true, spaced: true },
  { label: 'Tip', icon: '✦', keys: 'callout hint', snippet: `> [!tip]\n> ${CARET}`, block: true, spaced: true },
  { label: 'Warning', icon: '⚠', keys: 'callout caution', snippet: `> [!warning]\n> ${CARET}`, block: true, spaced: true },
  { label: 'Important', icon: '!', keys: 'callout key', snippet: `> [!important]\n> ${CARET}`, block: true, spaced: true },
  { label: 'Fold-away block', icon: '▸', keys: 'collapse details spoiler hide toggle', snippet: `> [!note]- Details\n> ${CARET}`, block: true, spaced: true },
  { label: 'Image from file', icon: '▣', keys: 'picture photo img upload', action: 'image' },
];

/** [name, highlight.js language, other names] for `/python`, `/rust`, ... */
const CODE_LANGS = [
  ['Python', 'python', 'py'], ['Rust', 'rust', 'rs'], ['C', 'c'], ['C++', 'cpp', 'cc cxx'], ['Java', 'java'],
  ['JavaScript', 'javascript', 'js'], ['TypeScript', 'typescript', 'ts'], ['Go', 'go', 'golang'],
  ['Bash', 'bash', 'sh shell'], ['SQL', 'sql'], ['Haskell', 'haskell', 'hs'], ['OCaml', 'ocaml', 'ml'],
  ['Kotlin', 'kotlin', 'kt'], ['Swift', 'swift'], ['C#', 'csharp', 'cs'], ['Ruby', 'ruby', 'rb'], ['PHP', 'php'],
  ['R', 'r'], ['Julia', 'julia', 'jl'], ['Scala', 'scala'], ['Lua', 'lua'], ['MATLAB', 'matlab'],
  ['LaTeX', 'latex', 'tex'], ['JSON', 'json'], ['YAML', 'yaml', 'yml'], ['HTML', 'xml', 'html'], ['CSS', 'css'],
  ['Makefile', 'makefile', 'make'], ['Dockerfile', 'dockerfile', 'docker'], ['Assembly (x86)', 'x86asm', 'asm'],
  ['Scheme', 'scheme'], ['Lisp', 'lisp'], ['Elixir', 'elixir', 'ex'], ['Nix', 'nix'], ['Diff', 'diff', 'patch'],
  ['Plain text', 'plaintext', 'text txt'],
];

function slashItems(query) {
  const q = query.toLowerCase();
  const score = (keys) => {
    const words = keys.toLowerCase().split(/\s+/);
    if (words.some((w) => w.startsWith(q))) return 0;
    return words.some((w) => w.includes(q)) ? 1 : -1;
  };
  const scored = [];
  SLASH_BLOCKS.forEach((b, i) => {
    const s = q ? score(`${b.label} ${b.keys}`) : 0;
    if (s >= 0) scored.push({ b, s, i });
  });
  if (q) {
    CODE_LANGS.forEach(([name, lang, aliases = ''], i) => {
      const s = score(`${name} ${lang} ${aliases}`);
      const b = { label: `Code · ${name}`, icon: '{ }', snippet: `\`\`\`${lang}\n${CARET}\n\`\`\``, block: true, spaced: true };
      if (s >= 0) scored.push({ b, s: s + 0.5, i: 100 + i });
    });
  }
  return scored.sort((a, b) => a.s - b.s || a.i - b.i).slice(0, q ? 10 : SLASH_BLOCKS.length).map((x) => x.b);
}

/**
 * Where and what to insert for a block snippet typed as `/query` at [start, end): on its
 * own line (moving any text after it below), with blank lines where Markdown needs them
 * to keep the block apart from the paragraphs around it. ‸ in `text` marks the cursor.
 */
function blockInsertion(value, start, end, block) {
  let text = block.snippet;
  if (block.block) {
    const lineStart = value.lastIndexOf('\n', start - 1) + 1;
    const lead = value.slice(lineStart, start);
    if (lead.trim()) {
      start = lineStart + lead.trimEnd().length;
      text = (block.spaced ? '\n\n' : '\n') + text;
    } else {
      if (lead) text = text.replace(/\n/g, '\n' + lead);
      const prevLine = value.slice(value.lastIndexOf('\n', lineStart - 2) + 1, Math.max(0, lineStart - 1));
      if (block.spaced && lineStart > 0 && prevLine.trim()) text = '\n' + text;
    }
    const rest = value.slice(end).match(/^[^\n]*/)[0];
    if (rest.trim()) {
      text += '\n\n' + rest.trim();
      end += rest.length;
    } else if (/^[ \t]*\n[ \t]*\S/.test(value.slice(end))) {
      text += '\n';
    }
  }
  return { start, end, text };
}

// ---------- \ LaTeX commands ----------

const texCache = new Map();
function texPreview(tex) {
  if (!texCache.has(tex)) {
    let html = '';
    try {
      html = katex.renderToString(tex, { throwOnError: false });
    } catch (e) {
      html = '';
    }
    texCache.set(tex, html);
  }
  return texCache.get(tex);
}

function latexMatches(query) {
  if (!query) return LATEX_COMMANDS.slice(0, 8);
  const ql = query.toLowerCase();
  const scored = [];
  LATEX_COMMANDS.forEach((c, i) => {
    let s = -1;
    if (c.name === query) s = 0;
    else if (c.name.startsWith(query)) s = 1;
    else if (c.name.toLowerCase().startsWith(ql)) s = 2;
    else if (`${c.desc} ${c.keys || ''}`.toLowerCase().split(/[^a-z0-9-]+/).some((w) => w.startsWith(ql))) s = 3;
    else if (c.name.toLowerCase().includes(ql)) s = 4;
    if (s >= 0) scored.push({ c, s, i });
  });
  return scored.sort((a, b) => a.s - b.s || a.i - b.i).map((x) => x.c);
}

// ---------- Live math preview ----------

/** Shows the formula under the caret, rendered, below the line (above it while a menu is open). */
function attachMathPreview(field, container, menus) {
  const box = h('div', { class: 'math-preview', hidden: true });
  container.append(box);
  let lastGood = '';
  let lastStart = -1;
  let frame = 0;

  const place = () => {
    const c = caretCoords(field);
    box.style.left = Math.max(8, Math.min(c.x - 16, window.innerWidth - box.offsetWidth - 8)) + 'px';
    const below = c.y + c.lineHeight + 6;
    const above = c.y - box.offsetHeight - 6;
    const fitsBelow = below + box.offsetHeight <= window.innerHeight - 8;
    const menuOpen = menus.some((m) => m.isOpen());
    // A suggestion list sits below; with no room above either, step aside until it closes.
    if (menuOpen && above < 8) box.hidden = true;
    box.style.top = Math.max(8, menuOpen || !fitsBelow ? above : below) + 'px';
  };
  const update = () => {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => {
      if (field.hidden || document.activeElement !== field || field.selectionStart !== field.selectionEnd) {
        box.hidden = true;
        return;
      }
      const region = mathAt(field.value, field.selectionStart);
      if (!region || !region.tex.trim()) {
        box.hidden = true;
        return;
      }
      if (region.start !== lastStart) {
        lastGood = '';
        lastStart = region.start;
      }
      let error = '';
      let failed = false;
      try {
        lastGood = katex.renderToString(region.tex, { displayMode: region.display, throwOnError: true });
      } catch (e) {
        failed = true;
        // A half-typed command (`\al` while picking from the list) isn't worth an error.
        error = menus.some((m) => m.isOpen()) ? '' : String(e.message || e).replace(/^KaTeX parse error: /, '');
        if (!error && !lastGood) {
          box.hidden = true;
          return;
        }
      }
      // While a formula is half-typed, keep showing the last version that rendered.
      box.replaceChildren(
        ...(lastGood ? [h('div', { class: `math-render${failed ? ' stale' : ''}`, html: lastGood })] : []),
        ...(error ? [h('div', { class: 'math-error' }, error)] : []),
      );
      box.hidden = false;
      place();
    });
  };
  for (const ev of ['input', 'click', 'keyup', 'focus', 'selectionmove']) field.addEventListener(ev, update);
  field.addEventListener('blur', () => setTimeout(() => {
    if (document.activeElement !== field) box.hidden = true;
  }, 150));
}
