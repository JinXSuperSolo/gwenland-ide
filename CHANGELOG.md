# Changelog

All notable changes to GwenLand IDE are documented here.

---

## [Unreleased] — v0.1.9

### Added
- **File type icons in the file tree** — every tree node now shows a proper icon for its file type (55+ extensions, special filenames like `package.json` / `Cargo.toml` / `.gitignore`). Icons come from `material-icon-theme` (already a project dependency — zero new packages). Folder nodes show an open vs closed variant based on expand state.
- **Git decorations in file tree** — modified files get an `M` badge, added `A`, untracked `U`, deleted `D`. Directories go amber when any child inside is dirty.
- **Git dirty dots on tabs** — each tab shows a small colored dot (amber/green/red) next to the close button when its file has uncommitted changes. Hides when the tab's own unsaved `●` dot is already visible.
- **Branch ahead/behind in status bar** — the git status bar shows `↑N` (green) and `↓N` (amber) when your branch is ahead or behind upstream. Returns `(0, 0)` gracefully on no upstream / detached HEAD. Backed by a new `ahead_behind()` in `engine/src/git.rs` with its own unit test.
- **`CustomDropdown` component** — styled, keyboard-navigable replacement for all native `<select>` elements. Supports inline SVG icons per item, compact mode, full keyboard nav, and `role="listbox"/"option"` accessibility.
- **Shell picker icons in terminal panel** — the shell selector now uses `CustomDropdown` and shows inline SVG icons for PowerShell, CMD, WSL, Bash, Zsh, Node, Python, and a generic terminal fallback.
- **AI agent live command output** — while the agent runs a terminal command, a pulsing "executing" banner and a live scrollable output log (capped at 500 lines) replace the normal Run/Skip buttons.
- **Agent command kill button** — a red ✕ button appears during execution. Fires `agent_kill_terminal`, which kills the process tree via `taskkill /F /T` on Windows or `kill -TERM` on Unix.
- **`agent://cmd_output` / `agent://cmd_done` events** — the backend now streams stdout/stderr line-by-line while the agent command runs, then fires a done event on exit.

### Fixed
- **AI agent UI freeze on long commands** — `run_terminal_tool` was a synchronous blocking call. Now fully async: stdout/stderr streamed line-by-line via `spawn_blocking`, so the Tauri runtime and UI stay responsive during `npm install` and similar operations.
- **Multi-tab broken** — single-click on a file now always opens a permanent tab. The preview-slot system (one italic reusable slot) has been removed. Every click on a file tree entry creates a real tab; clicking an already-open file activates its existing tab. No more "replace the only tab" behaviour.
- **Tab group locked on restore** — `isLocked` and `isMaximized` group state is no longer persisted to `layout.json` nor restored on startup. Lock/maximize are session-only. This eliminates the cold-start state where the group was silently locked, causing new tabs to route to an invisible secondary group.

### Changed
- **Context menus redesigned** — updated to GwenLand design tokens: `#1f1e1e` background, orange-tinted hover/active/border, `0.5rem` radius, `scaleY` pop animation. Danger items (Move to Trash, Delete Permanently) get a red label and red-tinted hover background.
- **All `<select>` elements replaced** — terminal shell picker and font picker now use `CustomDropdown`.
- **Git store refreshes after agent command** — `git.ts` listens for `agent://cmd_done` and re-polls, so tree badges update automatically after the agent runs a build or install.
- **Undo / Redo buttons** — replaced split-pane icon buttons in the editor tab-row toolbar with Undo (↩) and Redo (↪) icon buttons. Clicking them is identical to `Ctrl+Z` / `Ctrl+Y`.
- **Split pane removed** — `Split Editor`, `Split Editor Horizontal`, `Split Editor Vertical` commands removed from the command registry, View menu, editor right-click menu, and tab right-click menu. `splitHorizontal`/`splitVertical`/`openFileToSide` imports cleaned up.
- **Terminal tab-strip scroll** — the terminal session tab strip now shows a thin scrollbar when sessions overflow; previously the overflow was invisible and unscrollable.

### Infrastructure
- `cargo test -p gwenland-engine`: **455 passed** (up from 454 — new `ahead_behind` parse test).
- `pnpm test`: **82 passed** (9 suites) — unchanged.

---

## [0.1.8] — 2026-06-26 (M17 Stability Patch)

### Fixed
- **GWEN-357 — Preview tab content stale on file switch** — `Editor.svelte` `persistMounted()` was called before `mountTab()` during preview-slot replacement, overwriting the new file's `EditorState` with the old file's stale view state. Fix: skip `persistMounted()` when the tab id is the same but the path changed (in-place preview slot replacement).
- **GWEN-361 — Bulk delete UI freeze** — `moveToTrash` and `deletePath` calls in `fileActions.ts` and `TreeNode.svelte` now yield the render thread with `await new Promise(r => setTimeout(r, 0))` before the Tauri blocking call, keeping the UI responsive during large directory deletes. A success toast confirms completion.
- **GWEN-358 — "Delete Permanently" missing from context menu** — `file.deletePermanently` action (`Shift+Del`) restored to the file-tree context menu. "Move to Trash" now routes to the OS-native Recycle Bin (Windows PowerShell, macOS osascript, Linux gio) rather than a custom trash folder.

### Added
- **Double-click tab header to pin** — double-clicking an italic preview tab in the tab bar promotes it to a permanent pinned tab (same as VS Code behaviour). `pinTab()` function added to `tabs.ts`.
- **`undo` / `redo` icons** — Iconoir `undo.svg` and `redo.svg` added to the `Icon.svelte` registry.

### Changed (carry-over from M16)
- **Editor groups** — full multi-group model (`EditorGroup`, `TabsState`), split/merge/resize, lock, maximize, orientation toggle.
- **Tab drag & overflow** — tabs are draggable between groups; overflow scrolls with `scrollbar-width: none` (hidden but scrollable).
- **OS-native trash** — `move_path_to_os_trash` in `main.rs` uses PowerShell `Microsoft.VisualBasic.FileIO.FileSystem` on Windows, `osascript`/Finder on macOS, `gio trash`/kioclient on Linux.
- **Version bump** `0.1.7` → `0.1.8` across `tauri.conf.json`, `frontend/Cargo.toml`, `frontend/ui/package.json`.

### Infrastructure
- `cargo clean` required after disk-full build failure (8.2 GB freed from `target/`).
- `cargo test -p gwenland-engine`: 450 passed. `pnpm test`: 80 passed (9 suites).

---

## [0.1.7] — M16 IDE Polish

### Added
- Editor groups with horizontal/vertical split, resize divider, lock, maximize.
- Local history panel (snapshot on save, diff view, restore).
- Status bar: language mode, cursor position, indent info, LSP indicator, git branch, AI running indicator.
- CI/CD release pipeline (GitHub Actions — Windows MSI/NSIS, macOS, Linux).
- Tab drag-and-drop between groups.
- Tab overflow scroll.

### Fixed
- Terminal CWD sync with workspace.
- Chat copy / edit-rollback.
- Full git UI: status, panel, tree colours, diff tab, branch switcher.

---

## [0.1.0] — M10 through M15

### Milestones
- **M10** — Agentic workflow: human-gated plan→edit→validate loop, ReAct tool loop, autonomy tiers (Ask / AcceptForMe / FullControl).
- **M13** — Self-improving memory agent: local markdown memory at `.gwenland/agent/memory/`, Rust grep retriever, keyword extractor, write-back after response.
- **M14** — Local-first safety foundation: protected paths, AI agent trash at `.gwenland/trash/`, safety evaluation engine.
- **M15** — Critical IDE flow stabilisation.
- **M9** — Registry-driven right-click context menus, workspace-scoped file ops, `PromptDialog`.
- **M6** — Generic LSP bridge: diagnostics + autocomplete for Rust/TS/JS/Python, hand-rolled JSON-RPC, zero new engine deps.
- **M4** — AI system: streaming SSE, keychain-only API keys, `ChunkSource` model.
- **M3** — PTY terminal: ConPTY (Windows), XTerm.js canvas renderer, multi-session tab strip, 10k-line ring-buffer scrollback, per-chunk error detection.

---

## Legend

| Tag | Meaning |
|-----|---------|
| **Added** | New feature or capability |
| **Fixed** | Bug fix |
| **Changed** | Behaviour change to an existing feature |
| **Removed** | Feature or code deleted |
| **Infrastructure** | Build, CI, tooling, dependency changes |
