// CodeMirror 6 setup — ported from the legacy committed bundle
// (frontend/editor/entry.js) to native ESM so Vite bundles it with proper
// tree-shaking and types. Behavior is intentionally identical: same extensions,
// same custom VS Code-style search panel, no language grammars (@codemirror/lang-*).

import { EditorState, Facet, RangeSetBuilder, StateEffect, StateField } from '@codemirror/state'
import type { Extension } from '@codemirror/state'
import {
  Decoration,
  type DecorationSet,
  EditorView,
  WidgetType,
  hoverTooltip,
  keymap,
  lineNumbers,
  highlightActiveLine,
  highlightActiveLineGutter,
  drawSelection,
  rectangularSelection,
  crosshairCursor,
  dropCursor,
} from '@codemirror/view'
import {
  defaultKeymap,
  history,
  historyKeymap,
  indentWithTab,
  undo,
  redo,
} from '@codemirror/commands'
import {
  search,
  searchKeymap,
  highlightSelectionMatches,
  getSearchQuery,
  setSearchQuery,
  SearchQuery,
  findNext,
  findPrevious,
  selectMatches,
  replaceNext,
  replaceAll,
  openSearchPanel,
  closeSearchPanel,
} from '@codemirror/search'
import { indentOnInput, bracketMatching, indentUnit } from '@codemirror/language'
import { lintGutter, setDiagnostics, type Diagnostic as CmDiagnostic } from '@codemirror/lint'
import {
  autocompletion,
  closeBrackets,
  closeBracketsKeymap,
  completionKeymap,
  type CompletionContext,
  type CompletionResult,
} from '@codemirror/autocomplete'
import type { Text } from '@codemirror/state'
import { lspCompletion, lspHover, openBrowser, type LspDiagnostic } from '../tauri/commands'
import { lspChangePath, languageForPath } from '../stores/lsp'
import { reviewExtension } from './diff-overlay'
import { getLanguageExtension } from './language-detect'
// foldGutter/foldKeymap intentionally NOT imported — meaningful folding needs a
// language grammar (Milestone 6).

// Small inline SVG icons for the search widget (VS Code style).
const SVG = {
  chevronRight: '<svg viewBox="0 0 16 16" width="14" height="14"><path fill="currentColor" d="M6 4l4 4-4 4V4z"/></svg>',
  chevronDown: '<svg viewBox="0 0 16 16" width="14" height="14"><path fill="currentColor" d="M4 6l4 4 4-4H4z"/></svg>',
  up: '<svg viewBox="0 0 16 16" width="13" height="13"><path fill="currentColor" d="M8 4l4.5 4.5-1 1L8 6l-3.5 3.5-1-1L8 4z"/></svg>',
  down: '<svg viewBox="0 0 16 16" width="13" height="13"><path fill="currentColor" d="M8 12L3.5 7.5l1-1L8 10l3.5-3.5 1 1L8 12z"/></svg>',
  selectAll: '<svg viewBox="0 0 16 16" width="13" height="13"><path fill="currentColor" d="M2 3h12v2H2V3zm0 4h8v2H2V7zm0 4h12v2H2v-2z"/></svg>',
  close: '<svg viewBox="0 0 16 16" width="13" height="13"><path fill="currentColor" d="M8 9.06l3.7 3.7 1.06-1.06L9.06 8l3.7-3.7-1.06-1.06L8 6.94l-3.7-3.7L3.24 4.3 6.94 8l-3.7 3.7 1.06 1.06L8 9.06z"/></svg>',
}

// Custom VS Code-style search panel (single compact row + collapsible replace).
function createSearchPanel(view: EditorView) {
  const dom = document.createElement('div')
  dom.className = 'gw-search'

  dom.innerHTML = `
    <button class="gw-s-toggle" title="Toggle Replace" aria-label="Toggle Replace">${SVG.chevronRight}</button>
    <div class="gw-s-rows">
      <div class="gw-s-row">
        <div class="gw-s-field">
          <input class="gw-s-input" name="search" placeholder="Find" aria-label="Find"/>
          <div class="gw-s-toggles">
            <button class="gw-s-tog" data-tog="case" title="Match Case">Aa</button>
            <button class="gw-s-tog" data-tog="word" title="Match Whole Word"><u>ab</u></button>
            <button class="gw-s-tog" data-tog="re" title="Use Regular Expression">.*</button>
          </div>
        </div>
        <span class="gw-s-count">No results</span>
        <button class="gw-s-btn" data-act="prev" title="Previous Match (Shift+Enter)">${SVG.up}</button>
        <button class="gw-s-btn" data-act="next" title="Next Match (Enter)">${SVG.down}</button>
        <button class="gw-s-btn" data-act="all" title="Select All Matches">${SVG.selectAll}</button>
        <button class="gw-s-btn gw-s-close" data-act="close" title="Close (Escape)">${SVG.close}</button>
      </div>
      <div class="gw-s-row gw-s-replace-row">
        <div class="gw-s-field">
          <input class="gw-s-input" name="replace" placeholder="Replace" aria-label="Replace"/>
        </div>
        <button class="gw-s-btn gw-s-text" data-act="replace" title="Replace">Replace</button>
        <button class="gw-s-btn gw-s-text" data-act="replaceAll" title="Replace All">All</button>
      </div>
    </div>
  `

  const searchInput = dom.querySelector<HTMLInputElement>('input[name="search"]')!
  const replaceInput = dom.querySelector<HTMLInputElement>('input[name="replace"]')!
  const countEl = dom.querySelector<HTMLElement>('.gw-s-count')!
  const toggleBtn = dom.querySelector<HTMLButtonElement>('.gw-s-toggle')!
  const togState: Record<string, boolean> = { case: false, word: false, re: false }

  function commitQuery(extra?: () => void) {
    const q = new SearchQuery({
      search: searchInput.value,
      replace: replaceInput.value,
      caseSensitive: togState.case,
      wholeWord: togState.word,
      regexp: togState.re,
    })
    view.dispatch({ effects: setSearchQuery.of(q) })
    if (extra) extra()
    updateCount()
  }

  function updateCount() {
    const query = getSearchQuery(view.state)
    if (!query.search) {
      countEl.textContent = 'No results'
      countEl.classList.remove('gw-s-has')
      return
    }
    let total = 0
    try {
      const cursor = query.getCursor(view.state.doc)
      while (!cursor.next().done) total++
    } catch {
      total = 0
    }
    if (total === 0) {
      countEl.textContent = 'No results'
      countEl.classList.remove('gw-s-has')
    } else {
      countEl.textContent = total + (total === 1 ? ' result' : ' results')
      countEl.classList.add('gw-s-has')
    }
  }

  toggleBtn.addEventListener('click', () => {
    const open = dom.classList.toggle('gw-s-expanded')
    toggleBtn.innerHTML = open ? SVG.chevronDown : SVG.chevronRight
    if (open) replaceInput.focus()
  })

  dom.querySelectorAll<HTMLButtonElement>('.gw-s-tog').forEach((btn) => {
    btn.addEventListener('click', () => {
      const k = btn.dataset.tog!
      togState[k] = !togState[k]
      btn.classList.toggle('gw-s-on', togState[k])
      commitQuery()
    })
  })

  dom.querySelectorAll<HTMLButtonElement>('.gw-s-btn').forEach((btn) => {
    btn.addEventListener('click', () => {
      const act = btn.dataset.act
      if (act === 'next') findNext(view)
      else if (act === 'prev') findPrevious(view)
      else if (act === 'all') selectMatches(view)
      else if (act === 'replace') replaceNext(view)
      else if (act === 'replaceAll') replaceAll(view)
      else if (act === 'close') closeSearchPanel(view)
      view.focus()
      updateCount()
    })
  })

  searchInput.addEventListener('input', () => commitQuery())
  searchInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      e.shiftKey ? findPrevious(view) : findNext(view)
    } else if (e.key === 'Escape') {
      e.preventDefault()
      closeSearchPanel(view)
      view.focus()
    }
  })
  replaceInput.addEventListener('input', () => commitQuery())
  replaceInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      replaceNext(view)
    } else if (e.key === 'Escape') {
      e.preventDefault()
      closeSearchPanel(view)
      view.focus()
    }
  })

  return {
    dom,
    top: true,
    mount() {
      const q = getSearchQuery(view.state)
      if (q.search) searchInput.value = q.search
      updateCount()
      searchInput.focus()
      searchInput.select()
    },
    update(update: { docChanged: boolean }) {
      if (update.docChanged) updateCount()
    },
  }
}

/**
 * The absolute path of the document in an editor state, used by the LSP
 * autocomplete source to route completion requests. Empty for untitled/unknown
 * documents (the source then yields no LSP options).
 */
export const lspPath = Facet.define<string, string>({
  combine: (values) => values[0] ?? '',
})

const setInlineDiagnostics = StateEffect.define<LspDiagnostic[]>()

class InlineDiagnosticWidget extends WidgetType {
  constructor(private readonly diagnostic: LspDiagnostic) {
    super()
  }

  eq(other: InlineDiagnosticWidget): boolean {
    return (
      other.diagnostic.message === this.diagnostic.message &&
      other.diagnostic.severity === this.diagnostic.severity
    )
  }

  toDOM(): HTMLElement {
    const dom = document.createElement('div')
    dom.className = `cm-inline-diagnostic cm-inline-diagnostic-${this.diagnostic.severity}`
    dom.textContent = this.diagnostic.message
    return dom
  }
}

function inlineDiagnosticDecorations(doc: Text, diagnostics: LspDiagnostic[]): DecorationSet {
  const byLine = new Map<number, LspDiagnostic>()
  for (const diagnostic of diagnostics) {
    const line = Math.min(Math.max(diagnostic.range.start_line + 1, 1), doc.lines)
    const existing = byLine.get(line)
    if (!existing || severityRank(diagnostic.severity) < severityRank(existing.severity)) {
      byLine.set(line, diagnostic)
    }
  }

  const builder = new RangeSetBuilder<Decoration>()
  for (const [lineNo, diagnostic] of [...byLine.entries()].sort((a, b) => a[0] - b[0])) {
    const line = doc.line(lineNo)
    builder.add(
      line.to,
      line.to,
      Decoration.widget({
        widget: new InlineDiagnosticWidget(diagnostic),
        block: true,
        side: 1,
      }),
    )
  }
  return builder.finish()
}

function severityRank(severity: LspDiagnostic['severity']): number {
  if (severity === 'error') return 0
  if (severity === 'warning') return 1
  if (severity === 'information') return 2
  return 3
}

const inlineDiagnosticsField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(value, tr) {
    let next = tr.docChanged ? value.map(tr.changes) : value
    for (const effect of tr.effects) {
      if (effect.is(setInlineDiagnostics)) {
        next = inlineDiagnosticDecorations(tr.state.doc, effect.value)
      }
    }
    return next
  },
  provide: (field) => EditorView.decorations.from(field),
})

function tokenAt(doc: Text, pos: number): string {
  const line = doc.lineAt(pos)
  const stop = /[\s"'`<>()\[\]{}]/
  let from = pos
  let to = pos
  while (from > line.from && !stop.test(doc.sliceString(from - 1, from))) from--
  while (to < line.to && !stop.test(doc.sliceString(to, to + 1))) to++
  return doc
    .sliceString(from, to)
    .replace(/[.,;!?]+$/, '')
    .replace(/:\d+(?::\d+)?$/, '')
}

function wordRangeAt(doc: Text, pos: number): { from: number; to: number } | null {
  const line = doc.lineAt(pos)
  const isWord = /[\w$]/
  let from = pos
  let to = pos
  while (from > line.from && isWord.test(doc.sliceString(from - 1, from))) from--
  while (to < line.to && isWord.test(doc.sliceString(to, to + 1))) to++
  return from === to ? null : { from, to }
}

function maybeOpenCtrlClickTarget(
  event: MouseEvent,
  view: EditorView,
  onOpenPath?: (path: string) => void,
  onGoToDefinition?: (line: number, character: number) => void,
): boolean {
  if (!event.ctrlKey && !event.metaKey) return false
  const pos = view.posAtCoords({ x: event.clientX, y: event.clientY })
  if (pos === null) return false
  const text = tokenAt(view.state.doc, pos)
  if (!text) return false
  if (/^https?:\/\//i.test(text)) {
    void openBrowser(text).catch(() => {})
    return true
  }
  if ((text.includes('/') || text.includes('\\') || text.includes('.')) && onOpenPath) {
    onOpenPath(text)
    return true
  }
  if (onGoToDefinition) {
    const line = view.state.doc.lineAt(pos)
    onGoToDefinition(line.number - 1, pos - line.from)
    return true
  }
  return false
}

/**
 * CodeMirror autocomplete source backed by the LSP bridge (Milestone 6, Wave 5).
 * Converts the cursor offset to an LSP line/character, flushes the latest text
 * to the server, then requests completions. Returns null (no options) for
 * unsupported files, missing servers, or timeouts so typing is never disrupted.
 */
async function lspCompletionSource(
  context: CompletionContext,
): Promise<CompletionResult | null> {
  const path = context.state.facet(lspPath)
  if (!path || !languageForPath(path)) return null

  // The identifier prefix being typed (drives the replace range + filtering).
  const word = context.matchBefore(/[\w$]+/)
  // Member-access triggers ('.', '::', '->') let completion open with no prefix.
  const trigger = context.matchBefore(/[.:>]/)
  if (!context.explicit && !word && !trigger) return null

  const pos = context.pos
  const line = context.state.doc.lineAt(pos)
  const lspLine = line.number - 1
  const lspChar = pos - line.from

  // Ensure the server has the current buffer before asking. Awaited first so the
  // didChange write is ordered before the completion request.
  await lspChangePath(path, context.state.doc.toString())

  let options
  try {
    options = await lspCompletion(path, lspLine, lspChar, 0)
  } catch {
    return null
  }
  if (!options.length) return null

  return {
    from: word ? word.from : pos,
    options: options.map((o) => ({
      label: o.label,
      detail: o.detail ?? undefined,
      info: o.documentation ?? undefined,
      type: o.kind ?? undefined,
      apply: o.insert_text,
    })),
    validFor: /^[\w$]*$/,
  }
}

async function lspHoverSource(view: EditorView, pos: number) {
  const path = view.state.facet(lspPath)
  if (!path || !languageForPath(path)) return null
  const range = wordRangeAt(view.state.doc, pos)
  if (!range) return null

  const line = view.state.doc.lineAt(pos)
  const lspLine = line.number - 1
  const lspChar = pos - line.from

  await lspChangePath(path, view.state.doc.toString())

  const hover = await lspHover(path, lspLine, lspChar, 0).catch(() => null)
  const contents = hover?.contents?.trim()
  if (!contents) return null

  return {
    pos: range.from,
    end: range.to,
    above: true,
    create() {
      const dom = document.createElement('div')
      dom.className = 'cm-lsp-hover'
      dom.textContent = contents
      return { dom }
    },
  }
}

/**
 * Build an EditorState for `doc`.
 * - `onDocChange` (optional) fires on every document change so the host can
 *   recompute dirty state against its baseline.
 * - `onSelectionChange` (optional) fires whenever the doc OR selection changes,
 *   so the host can mirror the cursor position (Ln/Col) into a reactive store.
 * - `path` (optional) is the document's absolute path, enabling LSP
 *   diagnostics/autocomplete routing for that file.
 */
/**
 * Per-document mode flags (M19 Wave 3 — Large File Mode). When `large`, the
 * heavy extensions (syntax highlight, LSP autocomplete, lint gutter, inline
 * diagnostics, bracket matching, selection-match highlight) are dropped so a
 * huge file doesn't freeze the editor. When `veryLarge`, the document is also
 * read-only.
 */
export interface EditorMode {
  large?: boolean
  veryLarge?: boolean
}

export function createEditorState(
  doc?: string,
  onDocChange?: () => void,
  onSelectionChange?: () => void,
  path?: string,
  onOpenPath?: (path: string) => void,
  onGoToDefinition?: (line: number, character: number) => void,
  mode: EditorMode = {},
): EditorState {
  const large = mode.large ?? false
  const veryLarge = mode.veryLarge ?? false

  // Shared base: present in every mode (incl. large). Cheap, structural-only.
  const base: Extension[] = [
    lspPath.of(path ?? ''),
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightActiveLine(),
    drawSelection(),
    dropCursor(),
    rectangularSelection(),
    crosshairCursor(),
    history(),
    search({ top: true, createPanel: createSearchPanel }),
    indentUnit.of('    '), // 4-space indent
    keymap.of([
      ...completionKeymap,
      ...closeBracketsKeymap,
      ...defaultKeymap,
      ...historyKeymap,
      ...searchKeymap,
      indentWithTab,
    ]),
    EditorView.updateListener.of((update) => {
      if (update.docChanged && typeof onDocChange === 'function') onDocChange()
      if (
        (update.docChanged || update.selectionSet) &&
        typeof onSelectionChange === 'function'
      ) {
        onSelectionChange()
      }
    }),
    EditorView.domEventHandlers({
      click(event, view) {
        return maybeOpenCtrlClickTarget(event, view, onOpenPath, onGoToDefinition)
      },
    }),
  ]

  // Expensive features — skipped entirely in large-file mode.
  const rich: Extension[] = large
    ? []
    : [
        getLanguageExtension(path ?? ''),
        autocompletion({ override: [lspCompletionSource] }),
        hoverTooltip(lspHoverSource, { hideOnChange: true }),
        closeBrackets(),
        indentOnInput(),
        bracketMatching(),
        highlightSelectionMatches(),
        // M6: gutter markers for LSP diagnostics; inline squiggles enabled on
        // demand by setDiagnostics (see applyDiagnostics).
        lintGutter(),
        inlineDiagnosticsField,
        // M8: diff-review overlay (idle until a review session sets decorations).
        reviewExtension,
      ]

  // Very large files are read-only plain text.
  const readOnly: Extension[] = veryLarge
    ? [EditorState.readOnly.of(true), EditorView.editable.of(false)]
    : []

  return EditorState.create({ doc: doc ?? '', extensions: [...base, ...rich, ...readOnly] })
}

/** Mount a fresh EditorView for `state` inside `parent`, replacing prior content. */
export function mountEditorView(state: EditorState, parent: HTMLElement): EditorView {
  parent.innerHTML = ''
  return new EditorView({ state, parent })
}

// --- LSP diagnostics (Milestone 6, Wave 4) ---------------------------------

/** Map an LSP severity string to CodeMirror's diagnostic severity. */
function cmSeverity(s: LspDiagnostic['severity']): CmDiagnostic['severity'] {
  return s === 'information' ? 'info' : s // error | warning | hint pass through
}

/**
 * Convert a zero-based LSP line/character to a CodeMirror document offset.
 * Full-document sync keeps the CM doc identical to the server's, and both index
 * characters in UTF-16 code units, so `line.from + character` is exact. Values
 * are clamped so an out-of-date range can never throw.
 */
function posToOffset(doc: Text, line0: number, char0: number): number {
  const lineNo = Math.min(Math.max(line0 + 1, 1), doc.lines)
  const line = doc.line(lineNo)
  return Math.min(line.from + Math.max(char0, 0), line.to)
}

function lspToCmDiagnostic(doc: Text, d: LspDiagnostic): CmDiagnostic {
  const from = posToOffset(doc, d.range.start_line, d.range.start_character)
  let to = posToOffset(doc, d.range.end_line, d.range.end_character)
  if (to < from) to = from
  // A zero-width range would render no squiggle; widen by one where possible.
  if (to === from) to = Math.min(from + 1, doc.length)
  return {
    from,
    to,
    severity: cmSeverity(d.severity),
    message: d.message,
    source: d.source ?? d.code ?? undefined,
  }
}

/**
 * Replace the view's diagnostics with `diagnostics` (mapped from LSP ranges).
 * An empty array clears them. `setDiagnostics` also enables the inline squiggle
 * lint extension on first use, so no base-config change is required.
 */
export function applyDiagnostics(view: EditorView, diagnostics: LspDiagnostic[]): void {
  const cm = diagnostics.map((d) => lspToCmDiagnostic(view.state.doc, d))
  view.dispatch(setDiagnostics(view.state, cm), {
    effects: setInlineDiagnostics.of(diagnostics),
  })
}

export { EditorView, EditorState, undo, redo, openSearchPanel }
