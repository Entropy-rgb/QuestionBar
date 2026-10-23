// The parts of CodeMirror 6 the answer editor (ui/live-editor.js) uses, exposed as
// the global `CM` by scripts/build-editor.mjs.
export {
  EditorState, EditorSelection, StateField, StateEffect, Prec,
} from '@codemirror/state';
export {
  EditorView, Decoration, WidgetType, keymap, showTooltip, drawSelection, dropCursor, placeholder,
} from '@codemirror/view';
export { defaultKeymap, history, historyKeymap, indentMore, indentLess } from '@codemirror/commands';
export { markdown, commonmarkLanguage } from '@codemirror/lang-markdown';
export { syntaxTree, ensureSyntaxTree, syntaxHighlighting, HighlightStyle } from '@codemirror/language';
export {
  autocompletion, completionStatus, acceptCompletion, startCompletion, closeCompletion,
} from '@codemirror/autocomplete';
export { tags } from '@lezer/highlight';
export { GFM } from '@lezer/markdown';
