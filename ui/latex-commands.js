'use strict';

/**
 * Commands offered by the `\` autocomplete inside math, most useful first.
 * name: what you type after `\`; insert: the text added (‸ = where the cursor
 * goes); preview: LaTeX rendered in the list; desc: shown and searched;
 * keys: extra search words.
 */
const LATEX_COMMANDS = (() => {
  const C = '\u2038';
  const list = [];
  const sym = (names, desc = '') => {
    for (const name of names.split(/\s+/).filter(Boolean)) list.push({ name, insert: '\\' + name, preview: '\\' + name, desc });
  };
  const tpl = (name, insert, preview, desc, keys = '') => list.push({ name, insert, preview, desc, keys });
  const env = (name, preview, desc) => tpl(name, `\\begin{${name}}\n  ${C} &  \\\\\n   & \n\\end{${name}}`, preview, desc, 'begin matrix');

  tpl('frac', `\\frac{${C}}{}`, '\\frac{a}{b}', 'fraction');
  tpl('sqrt', `\\sqrt{${C}}`, '\\sqrt{x}', 'square root');
  tpl('sqrt', `\\sqrt[${C}]{}`, '\\sqrt[n]{x}', 'n-th root', 'root');
  tpl('sum', `\\sum_{${C}}^{}`, '\\sum_{i=1}^{n}', 'sum with limits');
  tpl('int', `\\int_{${C}}^{}`, '\\int_a^b', 'integral with limits');
  tpl('lim', `\\lim_{${C} \\to }`, '\\lim_{x \\to 0}', 'limit');
  tpl('prod', `\\prod_{${C}}^{}`, '\\prod_{i=1}^{n}', 'product with limits');
  tpl('binom', `\\binom{${C}}{}`, '\\binom{n}{k}', 'binomial coefficient', 'choose');
  tpl('dfrac', `\\dfrac{${C}}{}`, '\\dfrac{a}{b}', 'display-size fraction');
  tpl('tfrac', `\\tfrac{${C}}{}`, '\\tfrac{a}{b}', 'small fraction');

  sym('alpha beta gamma delta epsilon varepsilon zeta eta theta vartheta iota kappa lambda mu nu xi pi varpi rho varrho sigma varsigma tau upsilon phi varphi chi psi omega', 'Greek');
  sym('Gamma Delta Theta Lambda Xi Pi Sigma Upsilon Phi Psi Omega', 'Greek capital');

  sym('infty', 'infinity');
  sym('partial', 'partial derivative');
  sym('nabla', 'gradient, nabla');
  sym('cdot', 'centered dot, dot product');
  sym('times', 'multiply, cross product');
  sym('div', 'divide');
  sym('pm', 'plus-minus');
  sym('mp', 'minus-plus');
  sym('leq', 'less or equal');
  sym('geq', 'greater or equal');
  sym('neq', 'not equal');
  sym('approx', 'approximately equal');
  sym('equiv', 'equivalent, identical');
  sym('sim', 'similar, distributed as');
  sym('simeq cong', 'congruent, isomorphic');
  sym('propto', 'proportional to');
  sym('ll gg', 'much less / greater');
  sym('to', 'arrow, tends to');
  sym('rightarrow leftarrow leftrightarrow', 'arrow');
  sym('Rightarrow Leftarrow Leftrightarrow', 'double arrow');
  sym('implies', 'implies');
  sym('impliedby', 'is implied by');
  sym('iff', 'if and only if');
  sym('mapsto', 'maps to');
  sym('forall', 'for all');
  sym('exists', 'there exists');
  sym('nexists', 'there is no');
  sym('in', 'element of');
  sym('notin', 'not an element of');
  sym('ni', 'contains as element');
  sym('subset supset', 'proper subset / superset');
  sym('subseteq supseteq', 'subset / superset or equal');
  sym('subsetneq supsetneq nsubseteq', 'subset relations');
  sym('cup', 'union');
  sym('cap', 'intersection');
  sym('setminus', 'set difference');
  sym('emptyset varnothing', 'empty set');
  sym('complement', 'set complement');
  sym('neg lnot', 'not');
  sym('land wedge', 'and');
  sym('lor vee', 'or');
  sym('top bot', 'true / false, top / bottom');
  sym('therefore because', 'therefore / because');
  sym('ldots', 'dots on the baseline');
  sym('cdots', 'centered dots');
  sym('vdots ddots', 'vertical / diagonal dots');

  tpl('hat', `\\hat{${C}}`, '\\hat{x}', 'hat accent');
  tpl('bar', `\\bar{${C}}`, '\\bar{x}', 'bar accent, mean');
  tpl('vec', `\\vec{${C}}`, '\\vec{v}', 'vector arrow');
  tpl('dot', `\\dot{${C}}`, '\\dot{x}', 'time derivative dot');
  tpl('ddot', `\\ddot{${C}}`, '\\ddot{x}', 'second derivative dots');
  tpl('tilde', `\\tilde{${C}}`, '\\tilde{x}', 'tilde accent');
  tpl('overline', `\\overline{${C}}`, '\\overline{AB}', 'line over');
  tpl('underline', `\\underline{${C}}`, '\\underline{x}', 'line under');
  tpl('widehat', `\\widehat{${C}}`, '\\widehat{xyz}', 'wide hat');
  tpl('widetilde', `\\widetilde{${C}}`, '\\widetilde{xyz}', 'wide tilde');
  tpl('overrightarrow', `\\overrightarrow{${C}}`, '\\overrightarrow{AB}', 'arrow over');
  tpl('overleftarrow', `\\overleftarrow{${C}}`, '\\overleftarrow{AB}', 'arrow over, left');

  tpl('mathbb', `\\mathbb{${C}}`, '\\mathbb{R}', 'blackboard bold (ℝ ℕ ℤ ℚ ℂ)', 'reals naturals integers');
  tpl('mathcal', `\\mathcal{${C}}`, '\\mathcal{L}', 'calligraphic');
  tpl('mathbf', `\\mathbf{${C}}`, '\\mathbf{v}', 'bold');
  tpl('boldsymbol', `\\boldsymbol{${C}}`, '\\boldsymbol{\\alpha}', 'bold symbol');
  tpl('mathrm', `\\mathrm{${C}}`, '\\mathrm{d}x', 'upright');
  tpl('mathit', `\\mathit{${C}}`, '\\mathit{ab}', 'italic');
  tpl('mathsf', `\\mathsf{${C}}`, '\\mathsf{A}', 'sans-serif');
  tpl('mathtt', `\\mathtt{${C}}`, '\\mathtt{x}', 'monospace');
  tpl('mathfrak', `\\mathfrak{${C}}`, '\\mathfrak{g}', 'fraktur');
  tpl('text', `\\text{${C}}`, '\\text{if } x', 'normal text');
  tpl('textbf', `\\textbf{${C}}`, '\\textbf{bold}', 'bold text');
  tpl('textit', `\\textit{${C}}`, '\\textit{italic}', 'italic text');
  tpl('operatorname', `\\operatorname{${C}}`, '\\operatorname{rank}', 'named operator (rank, argmax)', 'argmax argmin');

  tpl('left', `\\left( ${C} \\right)`, '\\left( \\frac{a}{b} \\right)', 'auto-size ( )', 'parentheses brackets');
  tpl('left', `\\left[ ${C} \\right]`, '\\left[ \\frac{a}{b} \\right]', 'auto-size [ ]', 'brackets');
  tpl('left', `\\left\\{ ${C} \\right\\}`, '\\left\\{ \\frac{a}{b} \\right\\}', 'auto-size { }', 'braces');
  tpl('left', `\\left| ${C} \\right|`, '\\left| \\frac{a}{b} \\right|', 'auto-size | |', 'absolute value');
  tpl('langle', `\\langle ${C} \\rangle`, '\\langle u, v \\rangle', 'angle brackets, inner product');
  tpl('lfloor', `\\lfloor ${C} \\rfloor`, '\\lfloor x \\rfloor', 'floor');
  tpl('lceil', `\\lceil ${C} \\rceil`, '\\lceil x \\rceil', 'ceiling');
  tpl('lVert', `\\lVert ${C} \\rVert`, '\\lVert v \\rVert', 'norm');
  sym('rangle rfloor rceil rVert lvert rvert vert Vert backslash', 'delimiter');

  env('pmatrix', '\\begin{pmatrix}a&b\\\\c&d\\end{pmatrix}', 'matrix ( )');
  env('bmatrix', '\\begin{bmatrix}a&b\\\\c&d\\end{bmatrix}', 'matrix [ ]');
  env('vmatrix', '\\begin{vmatrix}a&b\\\\c&d\\end{vmatrix}', 'determinant | |');
  env('Bmatrix', '\\begin{Bmatrix}a&b\\\\c&d\\end{Bmatrix}', 'matrix { }');
  env('Vmatrix', '\\begin{Vmatrix}a&b\\\\c&d\\end{Vmatrix}', 'matrix ‖ ‖');
  env('matrix', '\\begin{matrix}a&b\\\\c&d\\end{matrix}', 'matrix, no brackets');
  tpl('cases', `\\begin{cases}\n  ${C} & \\text{if }  \\\\\n   & \\text{otherwise}\n\\end{cases}`,
    '\\begin{cases}a & \\text{if } x>0\\\\b & \\text{otherwise}\\end{cases}', 'piecewise cases', 'begin piecewise');
  tpl('aligned', `\\begin{aligned}\n  ${C} &=  \\\\\n  &= \n\\end{aligned}`, '\\begin{aligned}a&=b\\\\&=c\\end{aligned}', 'aligned equations', 'begin align');

  tpl('xrightarrow', `\\xrightarrow{${C}}`, '\\xrightarrow{f}', 'labelled arrow');
  tpl('xleftarrow', `\\xleftarrow{${C}}`, '\\xleftarrow{f}', 'labelled arrow, left');
  tpl('overset', `\\overset{${C}}{}`, '\\overset{!}{=}', 'symbol above');
  tpl('underset', `\\underset{${C}}{}`, '\\underset{x}{\\max}', 'symbol below');
  tpl('stackrel', `\\stackrel{${C}}{}`, '\\stackrel{def}{=}', 'stack over relation');
  tpl('overbrace', `\\overbrace{${C}}^{}`, '\\overbrace{a+b}^{n}', 'brace above');
  tpl('underbrace', `\\underbrace{${C}}_{}`, '\\underbrace{a+b}_{n}', 'brace below');
  tpl('boxed', `\\boxed{${C}}`, '\\boxed{E=mc^2}', 'box around');
  tpl('cancel', `\\cancel{${C}}`, '\\cancel{x}', 'strike out');
  tpl('pmod', `\\pmod{${C}}`, 'a \\pmod{n}', 'modulo (n)');
  tpl('textcolor', `\\textcolor{${C}}{}`, '\\textcolor{red}{x}', 'coloured math', 'color');

  sym('prod coprod iint iiint oint', 'big operator');
  sym('bigcup bigcap bigoplus bigotimes bigodot bigvee bigwedge bigsqcup', 'big operator');
  sym('sum int lim', 'plain (no limits)');
  sym('max min sup inf arg det dim ker deg gcd hom Pr limsup liminf', 'function');
  sym('log ln lg exp', 'logarithm, exponential');
  sym('sin cos tan cot sec csc arcsin arccos arctan sinh cosh tanh coth', 'trigonometry');
  sym('bmod', 'modulo');
  tpl('mod', `\\mod{${C}}`, 'a \\mod{n}', 'modulo');
  sym('ast star circ bullet oplus ominus otimes oslash odot sqcup sqcap dagger ddagger amalg wr', 'operator');
  sym('le ge ne', 'short form');
  sym('prec succ preceq succeq sqsubseteq perp parallel mid nmid models vdash dashv asymp doteq coloneqq triangleq approxeq lesssim gtrsim leqslant geqslant', 'relation');
  sym('longrightarrow longleftarrow longleftrightarrow Longrightarrow Longleftarrow Longleftrightarrow longmapsto gets', 'long arrow');
  sym('uparrow downarrow updownarrow Uparrow Downarrow nearrow searrow swarrow nwarrow', 'arrow');
  sym('hookrightarrow hookleftarrow rightharpoonup rightleftharpoons leadsto', 'arrow');
  sym('hbar ell wp Re Im aleph beth gimel imath jmath', 'letter-like');
  sym('prime backprime angle measuredangle triangle square Box Diamond checkmark degree surd dots', 'symbol');
  sym('quad qquad', 'space');
  sym('displaystyle textstyle', 'size style');
  return list;
})();
