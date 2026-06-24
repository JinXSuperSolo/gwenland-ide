# Browse and Read Wave, Plus the Fixes That Made It Actually Work

- **Date:** 2026-06-17
- **Issue:** GWEN-235, GWEN-236
- **Milestone:** Milestone 2 — Core IDE

## Problem / Context

Wave 1 rooted the IDE to a folder and added the panel-resize skeleton, but the explorer was still empty and nothing could be opened. Wave 2 was supposed to fix that: list a real directory, render the tree, and open a file's contents into the editor. What actually happened is that we shipped the Wave 2 code, the build was green, and then none of it worked in the running app — clicking a file did nothing, the explorer wouldn't populate, and the Open Folder button only worked from one place. Most of this entry is about chasing down why, because the reasons turned out to be three separate problems sitting underneath the feature work.

## Change

**Wave 2 feature work (GWEN-235 / GWEN-236):**
- Added `read_file` to `engine/src/fs.rs` — reads bytes, returns the text on valid UTF-8, returns `BinaryFile` otherwise, `Io` on a read failure. Registered the matching two-line `read_file` command in `frontend/src/main.rs` (three M2 commands now: open dialog, list, read).
- Rewrote the explorer in `index.html` as `renderExplorerTree` — a real filesystem-backed tree that lazily lists a folder's children when you expand it, renders `.ft-item` rows with the right indentation per depth, and swaps the folder icon open/closed.
- Added `openFileInTab` (with a dedup guard so opening the same file twice just re-focuses the existing tab, and a toast for binary files) and `updateBreadcrumbs` (path split relative to the workspace root).
- Added tests for `read_file`: UTF-8 round-trip, binary-rejection, missing-file, plus property tests for the binary case and the read half of the write/read round-trip. Engine test count went from 9 to 14, all passing, no `tauri` in the engine dependency graph.

**The three bugs we fixed along the way:**
- **The app showed a 404 instead of the UI.** `frontendDist` in `tauri.conf.json` pointed at a single file (`../index.html`), but Tauri wants a directory — so dev mode fell back to an empty dev-server URL and 404'd. Created `frontend/dist/`, moved `index.html` into it, and pointed `frontendDist` at `./dist`. This also gives us a clean "servable assets only" folder, matching where the CodeMirror bundle will live later.
- **Every backend call was silently faked.** `window.__TAURI__` was never exposed to the webview because `withGlobalTauri` wasn't set, so every `invoke()` quietly hit the browser-preview mock and returned `null` or canned data. That single missing flag is why the explorer wouldn't fill and files wouldn't open. Set `"withGlobalTauri": true`, confirmed the correct Tauri 2 global path (`window.__TAURI__.core.invoke`), made the mock return safe empties instead of `null`, and added an "IPC bridge: connected / not connected" line to the terminal panel so this is obvious next time.
- **Menus opened but their items did nothing.** The titlebar dropdowns and right-click menus were mock labels with no handlers. Gave `showContextMenu` real per-item actions and wired the ones Milestone 2 actually supports — Open Folder, Save (lands fully in Wave 3), Close Editor, Close Folder, and the file-tree's Open / Copy Path / Copy Relative Path. Everything without a backing feature yet is now greyed out instead of looking clickable.

**Removed:**
- The entire Milestone 1 mock layer: the in-memory `MOCK_FILES`, the hardcoded `tree`, and the old `renderFileTree` / `handleFileTreeClick` / `selectTab` / `renderTabs` / `closeTab` functions and their `activeTab` / `openTabs` globals. All of it is replaced by the real `workspaceState`-driven code.
- The mock "open project" `alert()` on Recent Projects — clicking a recent project now actually opens that folder.
- The native browser context menu (the Back / Reload / Inspect / extension junk) is suppressed app-wide via a capture-phase handler.

## Why this approach

We kept the editor and tab-bar as interim textarea-based implementations on purpose — the real CodeMirror editor and the dirty-tracking tab bar belong to Waves 3 and 4, and stubbing them now would have meant throwaway code that fights the spec. The interim versions keep the open-file-and-see-it flow testable today without pretending to be the final thing. On the editor question specifically: we looked at Monaco since it came up, but its 2 MB+ gzipped footprint alone would blow the 7 MB budget, its no-bundler path relies on a deprecated loader, and it's built around the LSP features this milestone explicitly defers — so CodeMirror 6 stays. The engine kept its hard rule of zero `tauri` imports throughout; the folder-dialog plumbing lives in the frontend command wrapper for exactly that reason.

## Impact

- The core loop finally works end to end in the real app: open a folder, the explorer fills with real files and folders, expanding a folder lists its contents, and clicking a file opens its text in the editor.
- `cargo test -p gwenland-engine` reports 14 passed, 0 failed, no warnings; the engine still has no `tauri` in its dependency graph.
- **Things to keep in mind:** the missing `withGlobalTauri` flag cost us the most time and looked exactly like a frontend logic bug — the IPC-status line in the terminal panel exists now so it's a five-second check next time. Config changes like this need a full `cargo tauri dev` restart, not just a window reload. Menu items for file creation, rename, and delete are deliberately disabled because they need new engine commands that aren't in the Milestone 2 plan; Undo/Redo/Cut/Copy/Paste/Find come alive with the real editor in Wave 3.
