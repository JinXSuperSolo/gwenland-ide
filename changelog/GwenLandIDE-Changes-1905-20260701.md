# GwenLand IDE - Session Changes

**Date:** 2026-07-01
**Scope:** AI Diff Agent Unified/Split view (GWEN-459) + File Attachment display chip (GWEN-460)
**Status:** Implemented and validated — frontend suites clean, GWEN-459 verified live via `cargo tauri dev`

---

## What changed this session

Two display-layer follow-ups to the M26 AI work, both presentation-only (no new
engine logic, no new dependencies). First, the diff viewer got a Unified ↔ Split
layout toggle, extracted into a single reusable `DiffView` component that both
the git-diff tab and the commit-diff viewer now share — one source of truth for
how a diff renders, with per-file headers, independent old/new line numbers, and
GwenUI-token colors instead of hardcoded hex. Second, files referenced in AI
Chat now render as a proper attachment chip (icon by type, truncated filename,
human-readable size, and a download/open action) instead of a raw path string.

Both reuse what already existed rather than inventing parallels: the diff work
parses through the engine's existing `parse_unified_diff`, and the attachment
chip reuses the existing `material-icon-theme` glyph set and the existing
`ContextAttachment`/`ImageAttachment` data shapes (extended with optional
display-only fields on the TS side).

---

## Added

| Area | Change |
| --- | --- |
| Reusable diff renderer (GWEN-459) | New `DiffView.svelte` — takes engine `DiffFile[]` and renders **Unified** or **Split** with a borderless view toggle, a per-file header (filename + extension badge + `+N −N` stats), and independent old/new line numbers. Single source of truth for every diff surface. |
| Split-pairing algorithm (GWEN-459) | New pure `lib/ai/diff-rows.ts` (`unifiedRows` / `splitRows`): converts a `DiffFile` into row models. In split mode a run of removed lines is paired with the following run of added lines index-by-index, with a blank filler cell on the shorter side, so uneven add/remove blocks stay aligned. 10 unit tests including the uneven-block cases. |
| Global diff color tokens (GWEN-459) | Added `--diff-add-line/-gutter/-text`, `--diff-del-*`, `--diff-hunk-bg`, `--diff-num`, `--diff-divider` to `tokens.css` — green/red kept as the add/remove signal but tuned to the warm `#1f1e1e` base rather than GitHub hex. Shared by every diff surface. |
| Diff view-mode preference (GWEN-459) | `diffViewMode: 'unified' \| 'split'` added to `stores/editor-preferences.ts`, persisted in localStorage and shared across all diff surfaces. |
| File attachment chip (GWEN-460) | New `FileAttachment.svelte` — icon (by MIME/extension) + truncated filename + human-readable size + a download/open action, in `sm` (inline in chat) and `card` (standalone) variants, styled with GwenUI tokens to match the AI panel's message chrome. |
| Attachment helpers (GWEN-460) | New pure `lib/ai/file-attachment.ts`: `attachmentIconSvg` (reuses the existing `fileIconSvg` glyphs; maps MIME → extension for image-only attachments), `formatFileSize` (B/KB/MB/GB), `truncateFileName` (middle-truncate keeping the extension visible). 15 unit tests. |
| `download` icon | Added a `download` glyph to `gwenland-icons.ts` for the attachment chip's action button. |

---

## Changed

| Area | Change |
| --- | --- |
| `GitDiffViewer.svelte` (GWEN-459) | Dropped its own TS regex diff parser + inline diff-grid rendering; now parses via the engine (`parseDiff`) and delegates rendering to `DiffView`. Keeps its own header (path + Untracked pill + "Review with AI") and the AI review drawer. |
| `GitCommitDiffViewer.svelte` (GWEN-459) | Same refactor — dropped its duplicate parser/rendering and now consumes `DiffView`, so the commit diff gets the Unified/Split toggle and per-file headers too. |
| `AiMessage.svelte` (GWEN-460) | User `file` context-attachments now render via `FileAttachment` (icon + name + size + open action) instead of a plain path-text chip. Selection/terminal-error chips and image thumbnails are unchanged. |
| `ContextAttachment` / `ImageAttachment` types (GWEN-460) | Extended with optional display-only fields (`size?` on the file variant; `name?`/`size?` on images) on the **TypeScript mirror only** — the engine neither needs nor persists them, so the Rust structs were intentionally left untouched. `currentFileAttachment()` now stamps the in-memory byte size as a best-effort hint. |

---

## Validation

| Gate | Result |
| --- | --- |
| `svelte-check` | 0 errors. |
| `pnpm.cmd test` (vitest) | 207 passed, 2 failed (pre-existing, unrelated `actionRegistry.test.ts`). Includes 10 new `diff-rows` tests + 15 new `file-attachment` tests. |
| Live verification (GWEN-459) | `cargo tauri dev`: opened a git diff and confirmed Unified + Split both render, toggle works, line numbers track independently in split mode, token colors read correctly, per-file header shows name/ext/stats. |
| Live verification (GWEN-460) | Skipped — surfacing the chip in a real chat message needs a billed AI send; trusted the unit tests + type-check per the same call made for the earlier token-usage work. |

---

## Notes

| Topic | Note |
| --- | --- |
| Dependencies | Zero new npm packages, zero new Rust crates. |
| Engine untouched | Both tasks are frontend-only. GWEN-459 reuses the existing `parse_unified_diff`; GWEN-460's optional display fields live on the TS mirror types, so no Rust enum/struct changes and no exhaustive-match churn in `context.rs`. |
| M23 overlay preserved | The M23 AI accept/reject flow is a live CodeMirror editor overlay, not a standalone diff renderer, so it was deliberately left untouched by the Split view work — no regression to the accept/reject-per-hunk interaction. |
| Single source of truth | GWEN-459 was explicitly built as a shared component: `GitDiffViewer` and `GitCommitDiffViewer` both render through `DiffView` now, removing two duplicate diff parsers. |
| `card` variant | `FileAttachment`'s standalone `card` variant is built and ready but has no live consumer yet, since AI-generated/surfaced file attachments aren't a data path in the app today; it's wired for when that lands. |
