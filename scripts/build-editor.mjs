// Bundles CodeMirror into ui/vendor/codemirror.min.js (global `CM`).
// Run `npm install` once, then `npm run build:editor` after changing versions.
import { build } from 'esbuild';
import { copyFileSync } from 'node:fs';

await build({
  entryPoints: ['scripts/codemirror-entry.mjs'],
  bundle: true,
  format: 'iife',
  globalName: 'CM',
  minify: true,
  legalComments: 'none',
  target: 'es2021',
  outfile: 'ui/vendor/codemirror.min.js',
});
copyFileSync('node_modules/@codemirror/view/LICENSE', 'ui/vendor/LICENSE.codemirror');
console.log('wrote ui/vendor/codemirror.min.js');
