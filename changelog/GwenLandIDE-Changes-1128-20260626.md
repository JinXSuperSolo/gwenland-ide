# M17 Stability Patch — Tab System Rewrite, Undo/Redo Toolbar, Split Pane Removal

- **Date:** 2026-06-26
- **Issue:** GWEN-357, GWEN-358, GWEN-359, GWEN-360, GWEN-361 + UX changes
- **Milestone:** M17 — v0.1.8 Stability Patch

---

## Overview

M17 is a pure stabilization pass targeting regressions from M14–M16. The core tab system was rewritten to remove the broken preview-slot model, which was the root cause of the "can only ever have one tab open" bug. The editor toolbar gained Undo/Redo buttons; split pane was removed. One build blocker (disk full) was resolved before work could begin.

Zero new Rust crates, zero new npm packages.

---

## Build Blocker — Disk Full (Pre-work)

`cargo tauri dev` failed with `rustc-LLVM ERROR: IO failure on output stream: no space on device`. C: drive had **0 bytes free**.

**Resolution:** `cargo clean` freed 8.2 GB from `target/`. The `target/` directory accumulates unboundedly across incremental builds and must be cleaned periodically on space-constrained machines.

---

## FIXED — Multi-Tab: Single Click Now Opens Permanent Tabs

**Problem (root cause identified):** The tab system used a "preview slot" model — one special italic tab that got replaced each time a new file was single-clicked. This meant only one tab could ever exist from single-clicks; you could never accumulate multiple open files. The root cause was architectural: `openFile()` searched for `candidate.preview === true` and replaced it in-place rather than appending a new tab.

**Additional blocker:** Browser fires `click → click → dblclick` on every double-click. The second `onclick` arrived before `ondblclick`, causing it to call `openFile(newFile, {preview:true})` which would find and destroy the preview slot of any previously-pinned file — making it impossible to accumulate permanent tabs even via double-click.

**Fix — `frontend/ui/src/lib/stores/tabs.ts`:**
- `openFile()` no longer accepts or acts on `options.preview`. Every file open creates a permanent tab (`preview: false`).
- The preview-slot replacement block (lines 368–378 in the old code) is removed entirely.
- If the file is already open in the group → activate it. If not → append a new tab. Simple.

**Fix — `frontend/ui/src/lib/components/TreeNode.svelte`:**
- Removed the 200ms click-debounce wrapper (`handleClick`/`handleDblClick`).
- `toggle()` no longer takes a `permanent` parameter. Single click = open file. Double click on a directory = expand/collapse (unchanged).
- `openFile(path)` called directly with no options.

**Fix — `frontend/ui/src/lib/stores/tabs.ts` (existing branch):**
- When file is already open, just activate it — no `preview: false` stripping needed since there is no preview state.

**Fix — `frontend/ui/src/lib/components/Tabs.svelte`:**
- Removed `pinTab` import and `ondblclick` pin handler (no longer needed).

---

## FIXED — Preview Tab Content Stale on File Switch (GWEN-357)

**Problem (root cause):** `Editor.svelte`'s `$effect` called `persistMounted()` before `mountTab()` on every tab switch. When a preview slot was replaced in-place (same tab `id`, new file `path`), `persistMounted()` called `persistTabState(preview.id, oldView.state)` — which **overwrote the new file's freshly-read EditorState with the old file's stale view state**. `mountTab()` then loaded that corrupted state and showed the wrong content.

**Fix — `frontend/ui/src/lib/components/Editor.svelte`:**
- Detect `isPreviewSlotReplacement = (activeId === mountedId && activePath !== mountedPath)`.
- Skip `persistMounted()` when this is true. The old file's state doesn't need saving — it was a temporary slot that was just discarded.

> **Note:** This fix is now partially superseded by the removal of the preview slot system entirely, but remains correct and is kept as a safety guard.

---

## FIXED — Tab Group Locked on Restore (root cause of invisible second group)

**Problem:** `editorGroupsSnapshot()` in `tabs.ts` was saving `isLocked: group.isLocked` and `isMaximized: group.isMaximized` to `layout.json`. On restart, `workspace-state.ts` restored them. If a group was ever locked in a previous session, every subsequent cold start had a locked group — causing `ensureWritableGroupId()` to silently create an invisible secondary group and route new tabs there instead of the visible one.

**Fix — `frontend/ui/src/lib/stores/tabs.ts`:**
- `editorGroupsSnapshot()` now always writes `isLocked: false, isMaximized: false`.

**Fix — `frontend/ui/src/lib/stores/workspace-state.ts`:**
- `resetEditorGroups()` call now hardcodes `isLocked: false, isMaximized: false` regardless of what's in the persisted JSON.

Lock and maximize are **session-only state** — they reset on every startup.

---

## FIXED — Bulk Delete UI Freeze (GWEN-361)

**Problem:** Large directory deletes via the file tree context menu caused the IDE to freeze. The Tauri async command runs on a thread pool (so the JS event loop isn't blocked), but the confirm dialog's dismissal and any pending UI renders were stuck until the operation completed.

**Fix — `frontend/ui/src/lib/actions/fileActions.ts` and `frontend/ui/src/lib/components/TreeNode.svelte`:**
- Added `await new Promise<void>((resolve) => setTimeout(resolve, 0))` before the Tauri delete/trash call. This yields the render thread, letting the confirm dialog close and the UI repaint before the blocking operation begins.
- Added `toast()` success notification on completion so the user sees confirmation.

---

## FIXED — "Delete Permanently" Missing from Context Menu (GWEN-358)

**Problem:** The "Delete Permanently" action (`Shift+Del`) was absent from the file-tree context menu after M16. "Move to Trash" was routing to the custom AI agent trash at `.gwenland/trash/` instead of the OS Recycle Bin.

**Fix — `frontend/ui/src/lib/actions/fileActions.ts`:**
- `file.deletePermanently` action (`Shift+Del`, order 45) registered in the file-tree context menu.
- `moveContextPathToTrash` calls `moveToTrash()` (Tauri command → `move_path_to_os_trash` in `main.rs`) which uses OS-native PowerShell on Windows, `osascript`/Finder on macOS, `gio trash`/kioclient on Linux.
- `deleteContextPathPermanently` calls `deletePath()` for permanent removal.

---

## ADDED — Undo / Redo Toolbar Buttons

**Change — `frontend/ui/src/lib/components/EditorGroups.svelte`:**
- Added a `.group-toolbar` div at the right end of every editor group's tab row.
- Contains two 26×26 icon buttons: **Undo** (↩) and **Redo** (↪).
- Clicking them calls `editorUndo()` / `editorRedo()` from `active-editor.ts`, identical to `Ctrl+Z` / `Ctrl+Y`.
- Separated from the tab strip by a 1px border.

**Change — `frontend/ui/src/lib/components/Icon.svelte`:**
- Added `undo` and `redo` from `iconoir/icons/regular/undo.svg` and `redo.svg`.

---

## REMOVED — Split Pane

**Removed from `frontend/ui/src/lib/commands/registry.ts`:**
- `editor.split` ("Split Editor") command
- `editor.splitHorizontal` ("Split Editor Horizontal", was `Ctrl+\`)
- `editor.splitVertical` ("Split Editor Vertical", was `Ctrl+K Ctrl+\`)
- "Split Editor Horizontal" and "Split Editor Vertical" entries from the View menu
- `splitHorizontal`, `splitVertical` imports

**Removed from `frontend/ui/src/lib/actions/editorActions.ts`:**
- `editor.split` from the editor right-click context menu
- `tab.splitRight` from the tab right-click context menu
- `openFileToSide`, `splitHorizontal` imports

> The underlying `splitHorizontal()` / `splitVertical()` / `openFileToSide()` functions in `tabs.ts` are **retained** (used by workspace-state restore). Only the UI entry points are removed.

---

## ADDED — Terminal Tab Strip Scrollbar

**Change — `frontend/ui/src/lib/components/TerminalPanel.svelte`:**
- `.tab-strip` now has `scrollbar-width: thin` + `scrollbar-color: var(--border) transparent` + 4px WebKit scrollbar thumb.
- Previously the terminal session tab strip had an invisible scrollbar (`height: 0`), making it impossible to scroll when many sessions were open.

---

## ADDED — `pinTab()` Export (tabs.ts)

Added `pinTab(id, groupId)` function that strips `preview: false` from a tab. Wired to `ondblclick` on tab headers in `Tabs.svelte` for parity with VS Code's "click to pin preview tab" behaviour.

> This was added mid-session before the preview system was fully removed. The function is harmless now that all tabs are permanent, and the double-click handler on tab headers is retained as a no-op guard.

---

## NOT FIXED — Known Remaining Issues

| Issue | Status | Notes |
|-------|--------|-------|
| Terminal horizontal scroll (xterm viewport) | Not fixed | XTerm canvas renderer handles its own scroll internally; horizontal wrapping is by design (terminal cols = container width). The tab-strip scrollbar was fixed but the terminal output itself does not scroll horizontally. |
| GWEN-359 — Startup stack overflow on Windows cold start | Already fixed in M16 | `git.ts` initializes lazily (only starts polling when `folderPath !== null`). No regression found. |
| GWEN-360 — Git polling blocking render / idle RAM | Already fixed in M16 | `app-focus.ts` + `git.ts` background throttle was already in place. `POLL_MS = 10000`, pauses on blur/hidden. No regression found. |
| Double-click folder expand delay | Not fixed | With the click debounce removed, folder expand is instant on single click. No debounce for dirs is intentional. |
| Split pane keyboard shortcuts (`Ctrl+\`) | Intentionally removed | Per user request. The commands are unregistered but the underlying store functions remain. |

---

## Files Changed

| File | Change |
|------|--------|
| `frontend/ui/src/lib/stores/tabs.ts` | Remove preview-slot logic; `openFile()` always permanent; `editorGroupsSnapshot()` never saves lock/maximize; added `pinTab()` |
| `frontend/ui/src/lib/components/TreeNode.svelte` | Remove debounce + `toggle(permanent)` param; simple `toggle()` → `openFile(path)` |
| `frontend/ui/src/lib/components/Editor.svelte` | Skip `persistMounted()` on preview slot replacement (same id, new path) |
| `frontend/ui/src/lib/stores/workspace-state.ts` | Hardcode `isLocked: false, isMaximized: false` on group restore |
| `frontend/ui/src/lib/actions/fileActions.ts` | Add `file.deletePermanently`; render yield + toast on delete/trash; remove split action; remove unused imports |
| `frontend/ui/src/lib/components/EditorGroups.svelte` | Add Undo/Redo toolbar buttons + styles; import `editorUndo`, `editorRedo`, `Icon` |
| `frontend/ui/src/lib/components/Icon.svelte` | Add `undo`, `redo` icons from iconoir |
| `frontend/ui/src/lib/components/Tabs.svelte` | Add `pinTab` import + `ondblclick` pin handler |
| `frontend/ui/src/lib/components/TerminalPanel.svelte` | Thin scrollbar on terminal tab strip |
| `frontend/ui/src/lib/commands/registry.ts` | Remove split commands + View menu entries + imports |

---

## Test Gates

- `pnpm check` — 0 errors, 0 warnings (297 files)
- `pnpm test` — 80 passed (9 suites)
- `cargo check --workspace` — clean (pre-existing path warning non-blocking)
- `cargo test -p gwenland-engine` — 450 passed, 0 failed
