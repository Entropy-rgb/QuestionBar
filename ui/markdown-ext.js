'use strict';

// markdown-it plugins for answers: Obsidian-style callouts and checklists.

/**
 * `> [!note] Title` blockquotes become coloured callout boxes. A `-` after the
 * type (`> [!tip]- Title`) makes it fold away until clicked; `+` makes it
 * foldable but open.
 */
function calloutPlugin(md) {
  const HEADER = /^\[!([a-z]+)\]([+-]?)[ \t]*(.*)$/i;

  md.core.ruler.after('block', 'callouts', (state) => {
    const tokens = state.tokens;
    for (let i = 0; i < tokens.length; i++) {
      if (tokens[i].type !== 'blockquote_open') continue;
      const para = tokens[i + 1];
      const inline = tokens[i + 2];
      if (!para || para.type !== 'paragraph_open' || !inline || inline.type !== 'inline') continue;
      const firstLine = inline.content.split('\n')[0];
      const m = firstLine.match(HEADER);
      if (!m) continue;

      let depth = 0;
      let close = i;
      for (; close < tokens.length; close++) {
        if (tokens[close].type === 'blockquote_open') depth++;
        else if (tokens[close].type === 'blockquote_close' && --depth === 0) break;
      }
      const type = m[1].toLowerCase();
      const callout = { type, fold: m[2], title: m[3].trim() || type[0].toUpperCase() + type.slice(1) };
      tokens[i].meta = { callout };
      if (tokens[close]) tokens[close].meta = { callout };

      // The header line is the title, not body text.
      inline.content = inline.content.slice(firstLine.length + 1);
      if (!inline.content.trim()) {
        para.hidden = true;
        tokens[i + 3].hidden = true;
      }
    }
  });

  const fallback = (tokens, idx, options, env, self) => self.renderToken(tokens, idx, options);
  const open = md.renderer.rules.blockquote_open || fallback;
  const close = md.renderer.rules.blockquote_close || fallback;
  md.renderer.rules.blockquote_open = (tokens, idx, options, env, self) => {
    const c = tokens[idx].meta && tokens[idx].meta.callout;
    if (!c) return open(tokens, idx, options, env, self);
    const cls = `callout callout-${md.utils.escapeHtml(c.type)}`;
    const title = md.utils.escapeHtml(c.title);
    return c.fold
      ? `<details class="${cls}"${c.fold === '+' ? ' open' : ''}><summary class="callout-title">${title}</summary><div class="callout-body">\n`
      : `<div class="${cls}"><div class="callout-title">${title}</div><div class="callout-body">\n`;
  };
  md.renderer.rules.blockquote_close = (tokens, idx, options, env, self) => {
    const c = tokens[idx].meta && tokens[idx].meta.callout;
    if (!c) return close(tokens, idx, options, env, self);
    return c.fold ? '</div></details>\n' : '</div></div>\n';
  };
}

/**
 * `- [ ] item` / `- [x] item` list items get a checkbox. `data-line` is the
 * item's line in the source, so a click can tick it in the Markdown.
 */
function taskListPlugin(md) {
  md.core.ruler.push('task_lists', (state) => {
    const tokens = state.tokens;
    for (let i = 2; i < tokens.length; i++) {
      const inline = tokens[i];
      const item = tokens[i - 2];
      if (inline.type !== 'inline' || tokens[i - 1].type !== 'paragraph_open' || item.type !== 'list_item_open') continue;
      const first = inline.children && inline.children[0];
      const m = first && first.type === 'text' && first.content.match(/^\[([ xX])\](?:[ \t]+|$)/);
      if (!m) continue;
      first.content = first.content.slice(m[0].length);
      const box = new state.Token('html_inline', '', 0);
      const line = item.map ? item.map[0] : -1;
      box.content = `<input type="checkbox" class="task" data-line="${line}"${m[1] === ' ' ? '' : ' checked'}>`;
      inline.children.unshift(box);
      item.attrJoin('class', 'task-item');
    }
  });
}
