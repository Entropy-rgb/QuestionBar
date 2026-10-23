'use strict';

// The answer editor: CodeMirror 6 with an Obsidian-style live preview. Markdown marks
// hide except on the line (or the bit of text) your cursor is in; maths, images,
// tables, callouts, checkboxes and dividers render in place. Suggestions: `/` for
// blocks, `\` for LaTeX inside maths, `[[` for links to other questions.

const {
  EditorState, EditorSelection, StateField, Prec,
  EditorView, Decoration, WidgetType, keymap, showTooltip, drawSelection, dropCursor, placeholder,
  defaultKeymap, history, historyKeymap, indentMore, indentLess,
  markdown, commonmarkLanguage,
  syntaxTree, ensureSyntaxTree, syntaxHighlighting, HighlightStyle,
  autocompletion, completionStatus, acceptCompletion, startCompletion,
  tags, GFM,
} = CM;

// ---------- Maths as Markdown syntax ----------

const DOLLAR = 36;
const isSpace = (c) => c === 32 || c === 9 || c === 10 || c === 13;
const isDigit = (c) => c >= 48 && c <= 57;

/**
 * `$x$`, `$ x $` and `$$x$$` inside paragraphs, and `$$` blocks on their own lines, by
 * the same rules as the renderer, so `$5 and $10` stays text and nothing inside maths
 * gets read as emphasis.
 */
const mathSyntax = {
  defineNodes: [
    { name: 'InlineMath' },
    { name: 'DisplayMath' },
    { name: 'BlockMath', block: true },
    { name: 'MathMark', style: tags.processingInstruction },
  ],
  parseInline: [{
    name: 'InlineMath',
    before: 'Emphasis',
    parse(cx, next, pos) {
      if (next !== DOLLAR) return -1;
      const wrap = (name, end, mark) => cx.addElement(cx.elt(name, pos, end,
        [cx.elt('MathMark', pos, pos + mark), cx.elt('MathMark', end - mark, end)]));
      if (cx.char(pos + 1) === DOLLAR) {
        const close = cx.slice(pos + 2, cx.end).indexOf('$$');
        return close > 0 ? wrap('DisplayMath', pos + 2 + close + 2, 2) : -1;
      }
      const after = cx.char(pos + 1);
      if (after < 0) return -1;
      if (isSpace(after)) {
        const m = /^\$[ \t]+[^$\s](?:[^$\n]*?[^$\s])?[ \t]+\$(?!\d)/.exec(cx.slice(pos, cx.end));
        return m ? wrap('InlineMath', pos + m[0].length, 1) : -1;
      }
      for (let i = pos + 1; i < cx.end; i++) {
        const c = cx.char(i);
        if (c === 10) return -1;
        if (c === DOLLAR && !isSpace(cx.char(i - 1)) && cx.char(i - 1) !== 92 && !isDigit(cx.char(i + 1))) {
          return wrap('InlineMath', i + 1, 1);
        }
      }
      return -1;
    },
  }],
  parseBlock: [{
    name: 'BlockMath',
    before: 'FencedCode',
    parse(cx, line) {
      if (line.next !== DOLLAR || line.text.charCodeAt(line.pos + 1) !== DOLLAR) return false;
      const start = cx.lineStart + line.pos;
      const add = (end) => cx.addElement(cx.elt('BlockMath', start, end,
        [cx.elt('MathMark', start, start + 2), cx.elt('MathMark', Math.max(start + 2, end - 2), end)]));
      const rest = line.text.slice(line.pos + 2);
      const sameLine = rest.indexOf('$$');
      if (sameLine >= 0) {
        // $$…$$ followed by more text is inline maths in a paragraph instead.
        if (rest.slice(sameLine + 2).trim()) return false;
        const end = start + 2 + sameLine + 2;
        cx.nextLine();
        add(end);
        return true;
      }
      while (cx.nextLine()) {
        const close = line.text.indexOf('$$');
        if (close >= 0) {
          const end = cx.lineStart + close + 2;
          cx.nextLine();
          add(end);
          return true;
        }
      }
      add(cx.prevLineEnd());
      return true;
    },
  }],
};

const MATH_NODES = new Set(['InlineMath', 'DisplayMath', 'BlockMath']);

/** The TeX inside a maths node, without its dollar signs. */
function mathSource(state, node) {
  const marks = node.node.getChildren('MathMark');
  const from = marks.length ? marks[0].to : node.from;
  const to = marks.length > 1 ? marks[marks.length - 1].from : node.to;
  return state.sliceDoc(from, to);
}

// ---------- Widgets shown in place of Markdown ----------

/**
 * A click on an inline widget would land the cursor beside it, leaving the source
 * hidden; put it inside the Markdown instead. `inside(node)` picks the spot.
 */
function revealOnClick(view, el, nodeNames, inside) {
  el.addEventListener('mousedown', (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    const start = view.posAtDOM(el);
    let node = syntaxTree(view.state).resolveInner(start + 1, 1);
    while (node && !nodeNames.includes(node.name)) node = node.parent;
    view.dispatch({ selection: { anchor: node ? inside(node) : start } });
    view.focus();
  });
}

class MathWidget extends WidgetType {
  constructor(tex, display) {
    super();
    this.tex = tex;
    this.display = display;
  }

  eq(other) {
    return other.tex === this.tex && other.display === this.display;
  }

  toDOM(view) {
    const el = document.createElement(this.display ? 'div' : 'span');
    el.className = this.display ? 'cm-lp-math-block' : 'cm-lp-math';
    el.innerHTML = katex.renderToString(this.tex, { displayMode: this.display, throwOnError: false });
    // Just before the closing dollar(s), so you can keep typing the formula.
    revealOnClick(view, el, ['InlineMath', 'DisplayMath', 'BlockMath'], (node) => node.to - (node.name === 'InlineMath' ? 1 : 2));
    return el;
  }

  // A click lands the cursor here, which shows the source again.
  ignoreEvent() {
    return false;
  }
}

class ImageWidget extends WidgetType {
  constructor(src, alt) {
    super();
    this.src = src;
    this.alt = alt;
  }

  eq(other) {
    return other.src === this.src && other.alt === this.alt;
  }

  toDOM(view) {
    const img = document.createElement('img');
    img.className = 'cm-lp-image';
    img.alt = this.alt;
    const relative = !/^[a-z][a-z0-9+.-]*:/i.test(this.src) && !this.src.startsWith('//');
    img.src = relative ? window.__TAURI__.core.convertFileSrc(this.src.replace(/^\.?\//, ''), 'notes') : this.src;
    revealOnClick(view, img, ['Image'], (node) => node.to);
    return img;
  }

  ignoreEvent() {
    return false;
  }
}

class TableWidget extends WidgetType {
  constructor(source) {
    super();
    this.source = source;
  }

  eq(other) {
    return other.source === this.source;
  }

  toDOM() {
    const el = document.createElement('div');
    el.className = 'cm-lp-table md';
    el.innerHTML = md.render(this.source);
    return el;
  }

  ignoreEvent() {
    return false;
  }
}

class CheckboxWidget extends WidgetType {
  constructor(checked) {
    super();
    this.checked = checked;
  }

  eq(other) {
    return other.checked === this.checked;
  }

  toDOM(view) {
    const box = document.createElement('input');
    box.type = 'checkbox';
    box.className = 'cm-lp-task';
    box.checked = this.checked;
    box.addEventListener('mousedown', (e) => {
      e.preventDefault();
      // The widget stands for "[ ]"; flip the character between the brackets.
      const pos = view.posAtDOM(box) + 1;
      view.dispatch({ changes: { from: pos, to: pos + 1, insert: this.checked ? ' ' : 'x' } });
    });
    return box;
  }

  ignoreEvent() {
    return true;
  }
}

class TextWidget extends WidgetType {
  constructor(text, className) {
    super();
    this.text = text;
    this.className = className;
  }

  eq(other) {
    return other.text === this.text && other.className === this.className;
  }

  toDOM() {
    const el = document.createElement('span');
    el.className = this.className;
    el.textContent = this.text;
    return el;
  }
}

// ---------- The live preview ----------

/** True when a selection touches [from, to]: then the Markdown shows as typed. */
function touched(state, from, to) {
  return state.selection.ranges.some((r) => r.from <= to && r.to >= from);
}

/** True when a selection touches any line of [from, to]. */
function touchedLines(state, from, to) {
  return touched(state, state.doc.lineAt(from).from, state.doc.lineAt(to).to);
}

const CALLOUT = /^(\s*>\s?)\[!([a-z]+)\]([+-]?)[ \t]*/i;
const QLINK_URL = /^#[0-9a-z]{4,16}$/;

function buildDecorations(state) {
  const doc = state.doc;
  const decos = [];
  const replace = (from, to, spec = {}) => {
    if (from < to) decos.push(Decoration.replace(spec).range(from, to));
  };
  const lineClass = (pos, cls) => decos.push(Decoration.line({ class: cls }).range(doc.lineAt(pos).from));
  const eachLine = (from, to, fn) => {
    for (let pos = from; pos <= to;) {
      const line = doc.lineAt(pos);
      fn(line);
      pos = line.to + 1;
    }
  };
  // Lines whose `> [!type]` prefix the callout handling already hides.
  const calloutHeads = new Set();

  const tree = ensureSyntaxTree(state, doc.length, 200) || syntaxTree(state);
  tree.iterate({
    enter(node) {
      const { name, from, to } = node;
      const heading = /^ATXHeading(\d)$/.exec(name);
      if (heading) {
        lineClass(from, `cm-lp-h cm-lp-h${heading[1]}`);
        const mark = node.node.getChild('HeaderMark');
        if (mark && !touchedLines(state, from, to)) replace(mark.from, Math.min(mark.to + 1, to));
        return;
      }
      switch (name) {
        case 'EmphasisMark':
        case 'StrikethroughMark':
        case 'CodeMark': {
          const parent = node.node.parent;
          if (parent && parent.name !== 'FencedCode' && !touched(state, parent.from, parent.to)) replace(from, to);
          return;
        }
        case 'InlineCode':
          decos.push(Decoration.mark({ class: 'cm-lp-code' }).range(from, to));
          return;
        case 'Link': {
          if (touched(state, from, to)) return;
          const marks = node.node.getChildren('LinkMark');
          const url = node.node.getChild('URL');
          // `[text]` without an address isn't a link when rendered either (e.g. `[!note]`).
          if (marks.length < 2 || !url) return;
          const labelFrom = marks[0].to;
          const labelTo = marks[1].from;
          const isQuestion = url && QLINK_URL.test(state.sliceDoc(url.from, url.to));
          replace(marks[0].from, marks[0].to);
          replace(labelTo, to);
          if (labelFrom < labelTo) decos.push(Decoration.mark({ class: isQuestion ? 'cm-lp-qlink' : 'cm-lp-link' }).range(labelFrom, labelTo));
          return false;
        }
        case 'Image': {
          if (touched(state, from, to)) return false;
          const url = node.node.getChild('URL');
          const marks = node.node.getChildren('LinkMark');
          const alt = marks.length >= 2 ? state.sliceDoc(marks[0].to, marks[1].from) : '';
          if (url) replace(from, to, { widget: new ImageWidget(state.sliceDoc(url.from, url.to), alt) });
          return false;
        }
        case 'InlineMath':
        case 'DisplayMath':
          if (touched(state, from, to)) decos.push(Decoration.mark({ class: 'cm-lp-math-src' }).range(from, to));
          else replace(from, to, { widget: new MathWidget(mathSource(state, node), name === 'DisplayMath') });
          return false;
        case 'BlockMath': {
          const first = doc.lineAt(from);
          const last = doc.lineAt(to);
          // Only whole lines can be swapped for a block.
          if (touchedLines(state, from, to) || first.from !== from || last.to !== to) {
            eachLine(from, to, (line) => lineClass(line.from, 'cm-lp-math-src-line'));
          } else {
            replace(from, to, { widget: new MathWidget(mathSource(state, node), true), block: true });
          }
          return false;
        }
        case 'Table':
          if (touchedLines(state, from, to)) {
            eachLine(from, to, (line) => lineClass(line.from, 'cm-lp-table-src'));
          } else {
            const first = doc.lineAt(from);
            const last = doc.lineAt(to);
            replace(first.from, last.to, { widget: new TableWidget(state.sliceDoc(first.from, last.to)), block: true });
          }
          return false;
        case 'Blockquote': {
          const head = doc.lineAt(from);
          const callout = CALLOUT.exec(head.text);
          const type = callout ? callout[2].toLowerCase() : null;
          eachLine(from, to, (line) => {
            const edges = `${line.from === head.from ? ' cm-lp-first' : ''}${line.to >= to ? ' cm-lp-last' : ''}`;
            lineClass(line.from, (type ? `cm-lp-callout cm-lp-callout-${type}` : 'cm-lp-quote') + edges);
          });
          if (callout) {
            calloutHeads.add(head.from);
            if (!touchedLines(state, head.from, head.from)) {
              const title = head.text.slice(callout[0].length).trim();
              const label = (callout[3] ? (callout[3] === '-' ? '▸ ' : '▾ ') : '') + (title ? '' : type[0].toUpperCase() + type.slice(1));
              replace(head.from, head.from + callout[0].length,
                label ? { widget: new TextWidget(label, 'cm-lp-callout-title') } : {});
              if (title) decos.push(Decoration.mark({ class: 'cm-lp-callout-title' }).range(head.from + callout[0].length, head.to));
            }
          }
          return;
        }
        case 'QuoteMark': {
          const line = doc.lineAt(from);
          if (calloutHeads.has(line.from) || touchedLines(state, from, from)) return;
          replace(from, Math.min(to + (doc.sliceString(to, to + 1) === ' ' ? 1 : 0), line.to));
          return;
        }
        case 'ListMark': {
          const item = node.node.parent;
          const task = item && item.getChild('Task');
          const bullet = item && item.parent && item.parent.name === 'BulletList';
          if (touchedLines(state, from, from)) return;
          const end = Math.min(to + 1, doc.lineAt(from).to);
          if (task) replace(from, end);
          else if (bullet) replace(from, end, { widget: new TextWidget('•', 'cm-lp-bullet') });
          return;
        }
        case 'TaskMarker':
          if (!touchedLines(state, from, to)) {
            replace(from, to, { widget: new CheckboxWidget(/\[[xX]\]/.test(state.sliceDoc(from, to))) });
          }
          return;
        case 'HorizontalRule':
          if (!touchedLines(state, from, to)) replace(from, to, { widget: new TextWidget('', 'cm-lp-hr') });
          return;
        case 'FencedCode':
          eachLine(from, to, (line) => {
            const fence = line.from === doc.lineAt(from).from || line.to === doc.lineAt(to).to;
            lineClass(line.from, fence ? 'cm-lp-code-line cm-lp-fence' : 'cm-lp-code-line');
          });
          return false;
        default:
          return undefined;
      }
    },
  });
  return Decoration.set(decos, true);
}

const livePreview = StateField.define({
  create: buildDecorations,
  update(decorations, tr) {
    if (tr.docChanged || tr.selection) return buildDecorations(tr.state);
    return decorations;
  },
  provide: (field) => EditorView.decorations.from(field),
});

// ---------- Formula preview while the cursor is inside maths ----------

let lastMath = { start: -1, html: '' };

function mathTooltip(state) {
  const sel = state.selection.main;
  if (!sel.empty) return null;
  const region = mathAt(state.doc.toString(), sel.head);
  if (!region || !region.tex.trim()) return null;
  const menuOpen = completionStatus(state) === 'active';
  let html = '';
  let error = '';
  try {
    html = katex.renderToString(region.tex, { displayMode: region.display, throwOnError: true });
    lastMath = { start: region.start, html };
  } catch (e) {
    // A half-typed command (`\al` while picking from the list) isn't worth an error.
    if (menuOpen) return null;
    error = String(e.message || e).replace(/^KaTeX parse error: /, '');
    html = lastMath.start === region.start ? lastMath.html : '';
  }
  return {
    pos: region.start,
    above: menuOpen,
    arrow: false,
    create: () => {
      const dom = document.createElement('div');
      dom.className = 'cm-lp-math-preview';
      dom.innerHTML = html ? `<div class="math-render${error ? ' stale' : ''}">${html}</div>` : '';
      if (error) dom.append(h('div', { class: 'math-error' }, error));
      return { dom };
    },
  };
}

const mathPreview = StateField.define({
  create: mathTooltip,
  update: (tooltip, tr) => mathTooltip(tr.state),
  provide: (field) => showTooltip.from(field),
});

// ---------- Suggestions ----------

function insideCodeOrMath(state, pos) {
  const scan = scanMarkup(state.doc.toString());
  return inCode(scan, pos) || scan.math.some((r) => pos >= r.from && pos <= r.to);
}

/** Applies a snippet where ‸ marks the cursor. */
function applySnippet(view, from, to, text) {
  const at = text.indexOf(CARET);
  const insert = text.replace(CARET, '');
  view.dispatch({
    changes: { from, to, insert },
    selection: { anchor: from + (at >= 0 ? at : insert.length) },
    scrollIntoView: true,
  });
}

function linkSuggestions(selfId) {
  return (context) => {
    const m = context.matchBefore(/\[\[[^\[\]\n]*$/);
    if (!m) return null;
    return {
      from: m.from,
      filter: false,
      options: findQuestions(m.text.slice(2), selfId).slice(0, 8).map((q, i) => ({
        label: plainTitle(q.title),
        detail: q.topic,
        boost: 99 - i,
        apply: (view, completion, from, to) => {
          const label = plainTitle(q.title).replace(/([\[\]])/g, '\\$1');
          // Swallow a closing ]] you may already have typed.
          const tail = view.state.sliceDoc(to, to + 300).match(/^[^\]\n]*\]\]/);
          applySnippet(view, from, to + (tail ? tail[0].length : 0), `[${label}](#${q.id})`);
        },
      })),
    };
  };
}

function slashSuggestions(onImage) {
  return (context) => {
    const line = context.state.doc.lineAt(context.pos);
    const m = line.text.slice(0, context.pos - line.from).match(/(?:^|\s)\/([^\s/]*)$/);
    if (!m) return null;
    const from = context.pos - m[1].length - 1;
    if (insideCodeOrMath(context.state, from)) return null;
    const items = slashItems(m[1]);
    if (!items.length) return null;
    return {
      from,
      filter: false,
      options: items.map((b, i) => ({
        label: b.label,
        icon: b.icon,
        boost: 99 - i,
        apply: (view, completion, start, end) => {
          if (b.action === 'image') {
            view.dispatch({ changes: { from: start, to: end, insert: '' } });
            onImage();
            return;
          }
          const edit = blockInsertion(view.state.doc.toString(), start, end, b);
          applySnippet(view, edit.start, edit.end, edit.text);
          // "Link to question" leaves `[[` behind: open the question list right away.
          if (edit.text.endsWith('[[')) startCompletion(view);
        },
      })),
    };
  };
}

function latexSuggestions(context) {
  const m = context.matchBefore(/\\[a-zA-Z]*$/);
  if (!m) return null;
  // `\\` is a line break in matrices, not the start of a command.
  if (m.from > 0 && context.state.sliceDoc(m.from - 1, m.from) === '\\') return null;
  const region = mathAt(context.state.doc.toString(), context.pos);
  if (!region) return null;
  return {
    from: m.from,
    filter: false,
    options: latexMatches(m.text.slice(1)).slice(0, 10).map((c, i) => ({
      label: '\\' + c.name,
      detail: c.desc,
      tex: c.preview,
      boost: 99 - i,
      apply: (view, completion, from, to) => {
        // Inline maths can't span lines, so multi-line templates are flattened there.
        let text = region.display ? c.insert : c.insert.replace(/\n\s*/g, ' ');
        const tail = view.state.sliceDoc(to, to + 40).match(/^[a-zA-Z]*/)[0];
        const next = view.state.sliceDoc(to + tail.length, to + tail.length + 1);
        if (!text.includes(CARET) && /[a-zA-Z]/.test(next)) text += ' ';
        applySnippet(view, from, to + tail.length, text);
      },
    })),
  };
}

/** The icon or rendered symbol in front of each suggestion. */
function suggestionExtra(completion) {
  if (completion.tex) {
    const el = document.createElement('span');
    el.className = 'tex-sym';
    el.innerHTML = texPreview(completion.tex);
    return el;
  }
  if (completion.icon) {
    const el = document.createElement('span');
    el.className = 'slash-icon';
    el.textContent = completion.icon;
    return el;
  }
  return null;
}

// ---------- Looks ----------

const markdownHighlight = HighlightStyle.define([
  { tag: tags.processingInstruction, color: 'var(--muted)' },
  { tag: tags.url, color: 'var(--muted)' },
  { tag: tags.strong, fontWeight: '700' },
  { tag: tags.emphasis, fontStyle: 'italic' },
  { tag: tags.strikethrough, textDecoration: 'line-through' },
  { tag: tags.heading, fontWeight: '650' },
  { tag: tags.monospace, fontFamily: 'ui-monospace, "Ubuntu Mono", monospace', fontSize: '0.9em' },
  { tag: tags.link, color: 'var(--accent)' },
]);

const editorTheme = EditorView.theme({
  '&': {
    background: 'var(--surface)', color: 'var(--text)',
    border: '1px solid var(--border)', borderRadius: 'var(--radius)',
  },
  '&.cm-focused': { outline: 'none', borderColor: 'var(--accent)', boxShadow: '0 0 0 3px var(--accent-soft)' },
  '.cm-scroller': { fontFamily: 'inherit', lineHeight: '1.55', maxHeight: '62vh', overflow: 'auto' },
  '.cm-content': { padding: '8px 6px', minHeight: '150px', caretColor: 'var(--text)' },
  '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--text)' },
  '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground': {
    background: 'color-mix(in srgb, var(--accent) 30%, transparent)',
  },
  '.cm-placeholder': { color: 'var(--muted)' },
  // The formula preview while the cursor is inside maths.
  '.cm-tooltip.cm-lp-math-preview': {
    background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 'var(--radius)',
    boxShadow: '0 8px 24px rgba(0, 0, 0, 0.35)', padding: '6px 10px', maxWidth: 'calc(100vw - 16px)',
  },
  '.cm-tooltip.cm-tooltip-autocomplete': {
    background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 'var(--radius)',
    boxShadow: '0 8px 24px rgba(0, 0, 0, 0.35)', padding: '4px',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul': { fontFamily: 'inherit', maxHeight: '260px' },
  '.cm-tooltip.cm-tooltip-autocomplete > ul > li': {
    padding: '4px 8px', borderRadius: '5px', display: 'flex', alignItems: 'center', gap: '8px',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]': { background: 'var(--accent-soft)', color: 'var(--text)' },
  '.cm-completionDetail': { marginLeft: 'auto', fontStyle: 'normal', color: 'var(--muted)', fontSize: '12px' },
});

// ---------- The editor ----------

/**
 * Creates the answer editor in `parent`. Callbacks: onChange(text), onDone(),
 * onImage() for "Image from file", onPaste(event) returning true if it took the paste.
 */
function createAnswerEditor({ parent, doc, caret, selfId, onChange, onDone, onImage, onPaste }) {
  const view = new EditorView({
    parent,
    state: EditorState.create({
      doc,
      selection: EditorSelection.cursor(Math.min(caret ?? doc.length, doc.length)),
      extensions: [
        history(),
        drawSelection(),
        dropCursor(),
        EditorView.lineWrapping,
        markdown({ base: commonmarkLanguage, extensions: [GFM, mathSyntax] }),
        syntaxHighlighting(markdownHighlight),
        livePreview,
        mathPreview,
        autocompletion({
          override: [linkSuggestions(selfId), slashSuggestions(onImage), latexSuggestions],
          icons: false,
          addToOptions: [{ render: suggestionExtra, position: 20 }],
        }),
        Prec.highest(keymap.of([
          { key: 'Mod-Enter', run: () => { onDone(); return true; } },
          { key: 'Escape', run: (v) => { if (completionStatus(v.state)) return false; onDone(); return true; } },
          { key: 'Tab', run: (v) => acceptCompletion(v) || indentMore(v), shift: indentLess },
        ])),
        keymap.of([...historyKeymap, ...defaultKeymap]),
        EditorView.contentAttributes.of({ spellcheck: 'true', 'aria-label': 'Answer' }),
        placeholder('Write your answer…  / for blocks, $maths$ (\\ for symbols), [[ to link a question'),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) onChange(update.state.doc.toString());
        }),
        EditorView.domEventHandlers({ paste: (event) => onPaste(event) }),
        editorTheme,
      ],
    }),
  });
  return {
    view,
    getText: () => view.state.doc.toString(),
    focus() {
      view.focus();
      view.dispatch({ scrollIntoView: true });
    },
    /** Inserts text at the cursor, on a line of its own when it's a block like an image. */
    insertBlockText(text) {
      const { from, to } = view.state.selection.main;
      const before = view.state.sliceDoc(Math.max(0, from - 1), from);
      const pad = from > 0 && before !== '\n' ? '\n' : '';
      const insert = pad + text + '\n';
      view.dispatch({ changes: { from, to, insert }, selection: { anchor: from + insert.length }, scrollIntoView: true });
    },
    destroy: () => view.destroy(),
  };
}
