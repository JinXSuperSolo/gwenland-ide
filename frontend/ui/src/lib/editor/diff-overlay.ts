// CodeMirror diff-review overlay (Milestone 8, Requirement 12).
//
// Renders a parsed diff for the active file directly in the editor WITHOUT
// touching the document: removed lines get a red line background + gutter class,
// and each hunk's added lines + Accept/Reject controls render as a block widget
// below the hunk. It is a separate StateField from M6's lint diagnostics, so it
// never clears or interferes with LSP squiggles/gutter markers (Req 12, Trap).

import {
  EditorView,
  Decoration,
  WidgetType,
  gutterLineClass,
  GutterMarker,
  type DecorationSet,
} from '@codemirror/view'
import { StateField, StateEffect, RangeSet } from '@codemirror/state'
import type { ReviewFile, ReviewHunk } from '../stores/diff-review'

export interface ReviewCallbacks {
  onAccept: (hunkId: string) => void
  onReject: (hunkId: string) => void
}

interface OverlayValue {
  deco: DecorationSet
  gutter: RangeSet<GutterMarker>
  /** First-line offset of each hunk, for keyboard navigation. */
  anchors: { hunkId: string; pos: number }[]
}

const emptyOverlay: OverlayValue = {
  deco: Decoration.none,
  gutter: RangeSet.empty,
  anchors: [],
}

const setOverlay = StateEffect.define<OverlayValue>()
const clearOverlay = StateEffect.define<null>()

const removedGutterMarker = new (class extends GutterMarker {
  elementClass = 'cm-diff-gutter-removed'
})()

/** Provides both the review decorations and the gutter line classes. */
const reviewField = StateField.define<OverlayValue>({
  create: () => emptyOverlay,
  update(value, tr) {
    for (const e of tr.effects) {
      if (e.is(setOverlay)) return e.value
      if (e.is(clearOverlay)) return emptyOverlay
    }
    if (tr.docChanged && value !== emptyOverlay) {
      return {
        deco: value.deco.map(tr.changes),
        gutter: value.gutter.map(tr.changes),
        anchors: value.anchors.map((a) => ({ hunkId: a.hunkId, pos: tr.changes.mapPos(a.pos) })),
      }
    }
    return value
  },
  provide: (f) => [
    EditorView.decorations.from(f, (v) => v.deco),
    gutterLineClass.from(f, (v) => v.gutter),
  ],
})

/** The extension to include in every editor state (idle until set). */
export const reviewExtension = [reviewField]

/** Block widget: a hunk's added lines plus its Accept/Reject controls. */
class HunkWidget extends WidgetType {
  constructor(
    readonly hunk: ReviewHunk,
    readonly cb: ReviewCallbacks
  ) {
    super()
  }

  eq(other: HunkWidget): boolean {
    return other.hunk.id === this.hunk.id && other.hunk.status === this.hunk.status
  }

  toDOM(): HTMLElement {
    const root = document.createElement('div')
    root.className = `cm-diff-widget cm-diff-${this.hunk.status}`

    for (const l of this.hunk.lines) {
      if (l.kind !== 'added') continue
      const row = document.createElement('div')
      row.className = 'cm-diff-added-row'
      // Use textContent so diff content is never interpreted as HTML.
      row.textContent = l.text === '' ? ' ' : l.text
      root.appendChild(row)
    }

    const bar = document.createElement('div')
    bar.className = 'cm-diff-bar'
    const status = document.createElement('span')
    status.className = 'cm-diff-bar-status'
    status.textContent = statusLabel(this.hunk)
    bar.appendChild(status)

    if (this.hunk.status === 'pending') {
      bar.appendChild(makeButton('Accept', 'accept', () => this.cb.onAccept(this.hunk.id)))
      bar.appendChild(makeButton('Reject', 'reject', () => this.cb.onReject(this.hunk.id)))
    }
    root.appendChild(bar)
    return root
  }

  ignoreEvent(): boolean {
    return true // let the buttons handle their own clicks
  }
}

function statusLabel(h: ReviewHunk): string {
  switch (h.status) {
    case 'accepted':
      return 'Accepted'
    case 'rejected':
      return 'Rejected'
    case 'failed':
      return h.error ? `Failed — ${h.error}` : 'Failed'
    default:
      return h.header ? `Hunk · ${h.header}` : 'Proposed change'
  }
}

function makeButton(label: string, kind: string, onClick: () => void): HTMLButtonElement {
  const b = document.createElement('button')
  b.className = `cm-diff-btn cm-diff-btn-${kind}`
  b.type = 'button'
  b.textContent = label
  b.addEventListener('mousedown', (e) => e.preventDefault()) // keep editor focus stable
  b.addEventListener('click', onClick)
  return b
}

const removedLineDeco = Decoration.line({ class: 'cm-diff-removed-line' })

/** Build line strings once for content-based hunk location. */
function docLines(view: EditorView): string[] {
  const arr: string[] = []
  const doc = view.state.doc
  for (let i = 1; i <= doc.lines; i++) arr.push(doc.line(i).text)
  return arr
}

function locate(lines: string[], oldLines: string[], nearIdx: number): number {
  if (oldLines.length === 0) return Math.min(Math.max(nearIdx + 1, 0), lines.length)
  const len = oldLines.length
  const matches = (start: number): boolean => {
    if (start < 0 || start + len > lines.length) return false
    for (let i = 0; i < len; i++) if (lines[start + i] !== oldLines[i]) return false
    return true
  }
  if (matches(nearIdx)) return nearIdx
  for (let d = 1; d <= lines.length; d++) {
    if (matches(nearIdx - d)) return nearIdx - d
    if (matches(nearIdx + d)) return nearIdx + d
  }
  return -1
}

/** Compute decorations for one file's hunks against the current document. */
function build(view: EditorView, file: ReviewFile, cb: ReviewCallbacks): OverlayValue {
  const doc = view.state.doc
  const lines = docLines(view)
  const decoRanges: { from: number; value: Decoration }[] = []
  const gutterRanges: { from: number; value: GutterMarker }[] = []
  const anchors: { hunkId: string; pos: number }[] = []

  for (const h of file.hunks) {
    if (h.status === 'rejected') continue
    const oldSide = h.lines.filter((l) => l.kind !== 'added').map((l) => l.text)
    const at = locate(lines, oldSide, h.oldStart - 1)
    if (at === -1) continue // can't place; apply step will report a conflict

    // Decorate removed lines + gutter within the located old-side range.
    let offset = 0
    for (const l of h.lines) {
      if (l.kind === 'added') continue
      const docLineNo = at + offset + 1
      if (l.kind === 'removed' && docLineNo >= 1 && docLineNo <= doc.lines) {
        const line = doc.line(docLineNo)
        decoRanges.push({ from: line.from, value: removedLineDeco })
        gutterRanges.push({ from: line.from, value: removedGutterMarker })
      }
      offset++
    }

    // Anchor the block widget after the old-side range (or at the insert point).
    const anchorPos =
      oldSide.length > 0
        ? doc.line(Math.min(at + oldSide.length, doc.lines)).to
        : at === 0
          ? 0
          : doc.line(Math.min(at, doc.lines)).to
    decoRanges.push({
      from: anchorPos,
      value: Decoration.widget({ widget: new HunkWidget(h, cb), block: true, side: 1 }),
    })

    const firstLine = oldSide.length > 0 ? doc.line(Math.min(at + 1, doc.lines)).from : anchorPos
    anchors.push({ hunkId: h.id, pos: firstLine })
  }

  decoRanges.sort((a, b) => a.from - b.from)
  gutterRanges.sort((a, b) => a.from - b.from)
  return {
    deco: Decoration.set(
      decoRanges.map((r) => r.value.range(r.from)),
      true
    ),
    gutter: RangeSet.of(
      gutterRanges.map((r) => r.value.range(r.from)),
      true
    ),
    anchors,
  }
}

/** Apply (or refresh) the review overlay for `file` on `view`. */
export function applyReviewOverlay(view: EditorView, file: ReviewFile, cb: ReviewCallbacks): void {
  view.dispatch({ effects: setOverlay.of(build(view, file, cb)) })
}

/** Remove all review decorations/gutters (Req 12.12, 12.13). */
export function clearReviewOverlay(view: EditorView): void {
  if (view.state.field(reviewField, false) === undefined) return
  view.dispatch({ effects: clearOverlay.of(null) })
}
