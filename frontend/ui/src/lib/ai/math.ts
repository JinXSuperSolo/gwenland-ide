/**
 * Dependency-free LaTeX → HTML math renderer (Milestone 8).
 *
 * Not a full TeX engine — it covers the subset assistant replies actually use:
 * Greek letters and common symbols/operators, superscripts/subscripts,
 * \frac, \sqrt (with optional index), \text/\mathrm/\mathbf/\mathit, function
 * names, and spacing — laid out with plain HTML + CSS (see AiMessage `.math-*`).
 * Unknown commands degrade to their (escaped) name so nothing ever throws.
 */

function esc(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
}

const SYMBOLS: Record<string, string> = {
  alpha: 'α', beta: 'β', gamma: 'γ', delta: 'δ', epsilon: 'ε', varepsilon: 'ε', zeta: 'ζ',
  eta: 'η', theta: 'θ', vartheta: 'ϑ', iota: 'ι', kappa: 'κ', lambda: 'λ', mu: 'μ', nu: 'ν',
  xi: 'ξ', pi: 'π', rho: 'ρ', sigma: 'σ', tau: 'τ', upsilon: 'υ', phi: 'φ', varphi: 'φ',
  chi: 'χ', psi: 'ψ', omega: 'ω', Gamma: 'Γ', Delta: 'Δ', Theta: 'Θ', Lambda: 'Λ', Xi: 'Ξ',
  Pi: 'Π', Sigma: 'Σ', Upsilon: 'Υ', Phi: 'Φ', Psi: 'Ψ', Omega: 'Ω',
  times: '×', div: '÷', pm: '±', mp: '∓', cdot: '·', ast: '∗', star: '⋆',
  leq: '≤', le: '≤', geq: '≥', ge: '≥', neq: '≠', ne: '≠', approx: '≈', equiv: '≡', sim: '∼',
  cong: '≅', ll: '≪', gg: '≫', propto: '∝', doteq: '≐',
  infty: '∞', partial: '∂', nabla: '∇', forall: '∀', exists: '∃', nexists: '∄', emptyset: '∅',
  varnothing: '∅', in: '∈', notin: '∉', ni: '∋', subset: '⊂', supset: '⊃', subseteq: '⊆',
  supseteq: '⊇', cup: '∪', cap: '∩', setminus: '∖', land: '∧', lor: '∨', lnot: '¬', neg: '¬',
  sum: '∑', prod: '∏', coprod: '∐', int: '∫', oint: '∮', iint: '∬',
  rightarrow: '→', to: '→', gets: '←', leftarrow: '←', leftrightarrow: '↔', Rightarrow: '⇒',
  Leftarrow: '⇐', Leftrightarrow: '⇔', mapsto: '↦', implies: '⟹', iff: '⟺', uparrow: '↑',
  downarrow: '↓', cdots: '⋯', ldots: '…', dots: '…', vdots: '⋮', ddots: '⋱',
  angle: '∠', perp: '⊥', parallel: '∥', deg: '°', circ: '∘', bullet: '•', oplus: '⊕',
  otimes: '⊗', odot: '⊙', hbar: 'ℏ', ell: 'ℓ', Re: 'ℜ', Im: 'ℑ', aleph: 'ℵ', wp: '℘',
  prime: '′', therefore: '∴', because: '∵', cdotp: '·', backslash: '\\',
}

const FUNCS = new Set([
  'sin', 'cos', 'tan', 'cot', 'sec', 'csc', 'sinh', 'cosh', 'tanh', 'log', 'ln', 'exp',
  'lim', 'max', 'min', 'det', 'dim', 'gcd', 'arg', 'deg', 'mod',
])

const PUNCT_CMD: Record<string, string> = {
  ',': ' ', ';': ' ', ':': ' ', '!': '', ' ': ' ', '{': '{', '}': '}',
  '%': '%', $: '$', _: '_', '#': '#', '&': '&amp;',
}

/** Read a balanced `{...}` group's RAW inner content. `i` is at the `{`. */
function readGroup(tex: string, i: number): [string, number] {
  let depth = 0
  for (let j = i; j < tex.length; j++) {
    if (tex[j] === '{') depth++
    else if (tex[j] === '}') {
      depth--
      if (depth === 0) return [tex.slice(i + 1, j), j + 1]
    }
  }
  return [tex.slice(i + 1), tex.length] // unbalanced — best effort
}

/** Read one RAW argument (a group, a command token, or a single char). */
function readArg(tex: string, i: number): [string, number] {
  while (i < tex.length && tex[i] === ' ') i++
  if (i >= tex.length) return ['', i]
  if (tex[i] === '{') return readGroup(tex, i)
  if (tex[i] === '\\') {
    let j = i + 1
    if (j < tex.length && !/[a-zA-Z]/.test(tex[j])) return [tex.slice(i, j + 1), j + 1]
    while (j < tex.length && /[a-zA-Z]/.test(tex[j])) j++
    return [tex.slice(i, j), j]
  }
  return [tex[i], i + 1]
}

function readCommand(tex: string, i: number): [string, number] {
  let j = i + 1
  if (j < tex.length && !/[a-zA-Z]/.test(tex[j])) {
    const p = tex[j]
    return [PUNCT_CMD[p] ?? esc(p), j + 1]
  }
  let name = ''
  while (j < tex.length && /[a-zA-Z]/.test(tex[j])) {
    name += tex[j]
    j++
  }
  if (name === 'frac' || name === 'dfrac' || name === 'tfrac') {
    const [a, j1] = readArg(tex, j)
    const [b, j2] = readArg(tex, j1)
    return [
      `<span class="math-frac"><span class="math-num">${convert(a)}</span><span class="math-den">${convert(b)}</span></span>`,
      j2,
    ]
  }
  if (name === 'sqrt') {
    let k = j
    let index = ''
    if (tex[k] === '[') {
      const end = tex.indexOf(']', k)
      if (end !== -1) {
        index = tex.slice(k + 1, end)
        k = end + 1
      }
    }
    const [a, j1] = readArg(tex, k)
    const idx = index ? `<sup class="math-root">${convert(index)}</sup>` : ''
    return [`${idx}<span class="math-sqrt"><span class="math-sqrt-body">${convert(a)}</span></span>`, j1]
  }
  if (['text', 'mathrm', 'operatorname', 'mathbf', 'mathit', 'mathsf', 'mathtt'].includes(name)) {
    const [a, j1] = readArg(tex, j)
    const style =
      name === 'mathbf'
        ? ' style="font-weight:700"'
        : name === 'mathit'
          ? ' style="font-style:italic"'
          : name === 'mathtt'
            ? ' style="font-family:var(--font-mono)"'
            : ''
    return [`<span class="math-text"${style}>${esc(a)}</span>`, j1]
  }
  if (name === 'left' || name === 'right') {
    const d = tex[j] ?? ''
    return [d === '.' ? '' : esc(d), j + 1]
  }
  if (name in SYMBOLS) return [SYMBOLS[name], j]
  if (FUNCS.has(name)) return [`<span class="math-func">${name}</span>`, j]
  return [esc(name), j] // unknown command → its name
}

/** Read one atom (group / command / char) starting at `i`. */
function readAtom(tex: string, i: number): [string | null, number] {
  if (i >= tex.length) return [null, i]
  const c = tex[i]
  if (c === ' ') return ['', i + 1]
  if (c === '{') {
    const [inner, j] = readGroup(tex, i)
    return [convert(inner), j]
  }
  if (c === '\\') return readCommand(tex, i)
  // Single variables render italic (like real math); digits/operators upright.
  if (/[A-Za-z]/.test(c)) return [`<i>${c}</i>`, i + 1]
  return [esc(c), i + 1]
}

/** Convert raw TeX into HTML, attaching ^/_ scripts to the preceding atom. */
function convert(tex: string): string {
  let out = ''
  let i = 0
  while (i < tex.length) {
    const [atom, ni] = readAtom(tex, i)
    if (atom === null) break
    i = ni
    let sup = ''
    let sub = ''
    while (i < tex.length && (tex[i] === '^' || tex[i] === '_')) {
      const isSup = tex[i] === '^'
      const [s, nj] = readAtom(tex, i + 1)
      i = nj
      if (isSup) sup = s ?? ''
      else sub = s ?? ''
    }
    let html = atom
    if (sup && sub) html += `<span class="math-ss"><sup>${sup}</sup><sub>${sub}</sub></span>`
    else if (sup) html += `<sup>${sup}</sup>`
    else if (sub) html += `<sub>${sub}</sub>`
    out += html
  }
  return out
}

/** Render a TeX string as inline or display math HTML. */
export function renderMath(tex: string, display: boolean): string {
  return `<span class="${display ? 'math-block' : 'math-inline'}">${convert(tex.trim())}</span>`
}
