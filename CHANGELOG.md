# Changelog

All notable changes to GwenLand IDE are documented here.

---

## [Unreleased]

### Added
- Full keyboard navigation (VSCode-style): file-tree arrow/Enter navigation, centralized Escape overlay stack (closes only topmost overlay), Tab / Shift+Tab pane cycling, Ctrl+Tab MRU editor switching (supports reverse), Insert key blocked to prevent accidental overwrite mode, and an OVR status-bar indicator; 46 new behavioral tests, typecheck clean, no new npm/Rust deps.
- Git Graph now opens as a floating IDE window with draggable, resizable, refresh, maximize/restore, and close controls.
- Canvas2D Git Graph surface with pan/zoom, frustum-culling-friendly rendering, branch colors, branch labels, selected-node highlight, HEAD marker, hover tooltip, and commit detail popup.
- Git Graph navigation dock with Find, Branch, Commit, and Date jump controls. Dock filtering uses the already-loaded graph payload and does not run git during hover, pan, zoom, or search.
- Full commit detail flow from graph popup into the existing commit diff tab.

### Fixed
- Production Tauri bundles no longer freeze during normal IDE interactions; blocking filesystem, Git, tree, watcher, safety, history, and process work now runs off the UI path.
- Windows release builds no longer flash external terminal windows during normal subprocess work.
- Terminal PTY startup is lazy and idempotent, with controlled failed state instead of infinite retry loops.
- File tree expansion and watcher refreshes now target only affected expanded folders instead of reloading the full workspace.

### Changed
- Source Control Graph now opens the floating Git Graph window instead of using a normal editor tab.
- Git Graph dock styling now uses the app card surface and compact workbench-style controls instead of an outlined generic floating toolbar.
- Release profile tightened for size with stripped symbols, no debug info, no split debug info, no incremental release artifacts, LTO, one codegen unit, `panic = "abort"`, and `opt-level = "z"`.
- Temporary redacted process-spawn logging was removed; crash/safety redaction remains where it protects user data.
- Windows bundling targets NSIS only.

### Infrastructure
- Git Graph frontend validation passed with `pnpm.cmd check`, `pnpm.cmd test`, `pnpm.cmd build`, and `git diff --check`; no new Rust crates or npm packages were added.

---

## [0.1.14] — 2026-06-27 (M19 — Performance & Scalability)

Eight waves of performance work under a hard **zero new Rust crates / zero new npm packages** rule — everything from scratch. Theme: Rust owns state, Svelte renders diffs (the UI never receives [...]

### Added
- **Batched file watcher (GWEN-376)** — new polling watcher (`engine/src/fs_watch.rs`, no `notify` crate) that snapshots only expanded directories and diffs them. A burst (e.g. `npm install` wri[...]
- **Virtualized file tree (GWEN-374 / GWEN-375)** — Rust-owned flat-row model (`engine/src/tree.rs`): the tree is one ordered `Vec<FlatRow>` and every mutation returns `TreePatch` deltas (Insert[...]
- **Large File Mode (GWEN-377)** — new `file_meta()` engine command classifies files on open: **large** (> 500 KB or > 10k lines) drops syntax/LSP/autocomplete/lint/minimap/sticky-scroll and ski[...]
- **Low-End Mode (GWEN-379)** — Settings → Performance toggle that disables git badges, indent guides, smooth scroll, minimaps, sticky scroll, animations, and file icons via a single `perfSett[...]
- **Status-bar performance badges (GWEN-381)** — data-driven badge row: Git scanning, Indexing, Large File, Low-End, Searching, and AI Running. Spinning badges fade out on resolve; cancellable o[...]
- **Optimistic file operations (GWEN-380)** — create/rename/delete/move update the tree immediately and roll back with a toast on failure; watcher refreshes are deferred during the op and reconc[...]
- **Workspace search (GWEN-382)** — new pure-`std` engine module (`engine/src/search.rs`): recursive walk, streamed line matches via callback, `AtomicBool` cancellation, `search_policy` exclusio[...]

### Changed
- **Terminal output is frame-limited (GWEN-378)** — a per-session ring buffer + `requestAnimationFrame` scheduler (`terminal/terminal-scheduler.ts`) coalesces all chunks arriving within a frame [...]
- **Terminal scrollback cap** raised to 50,000 lines (client-side), with the engine ring buffer still capping server-side retention separately.
- **Git store exposes `refreshing`** so the status bar can show a live git-scanning badge.

### Removed
- **Recursive `TreeNode.svelte`** — replaced by the flat-row model + `FileTreeRow.svelte`.

### Performance
- 10k-file workspaces scroll smoothly (constant DOM size via virtualization).
- Large files open without freezing the editor; the LSP is never handed a multi-MB buffer.
- Heavy terminal output costs at most one repaint per frame instead of one per chunk.

### Infrastructure
- Version bumped `0.1.12` → `0.1.14` across `frontend/tauri.conf.json`, `frontend/Cargo.toml`, `frontend/ui/package.json`.
- **Zero** new Rust crates, **zero** new npm packages.
- `cargo test -p gwenland-engine --lib`: **487 passed**. `pnpm test`: **114 passed** (15 suites). `svelte-check`: 0 errors.
- Release exe **4.77 MB** (budget ≤ 7 MB).

---

## [0.1.12] — 2026-06-26

### Added
- **File type icons in the file tree** — every tree node shows a proper icon for its file type (55+ extensions and special filenames like `package.json`, `Cargo.toml`, `.gitignore`). Folder node[...]
- **Git status decorations** — `M` / `A` / `U` / `D` letter badges on tree nodes; colored dirty dots on tabs; `↑N ↓N` ahead/behind counter in the status bar. Engine: new `ahead_behind()` in [...]
- **AI agent command streaming** — `run_terminal_tool` is now fully async. Stdout/stderr stream line-by-line via `agent://cmd_output` events; `agent://cmd_done` fires on exit. No more UI freeze [...]

### Fixed
- **AI agent UI freeze** — `run_terminal_tool` was synchronous, blocking the entire Tauri runtime. Now async with `spawn_blocking` I/O readers.
- **Multi-tab broken** — single click always opens a real permanent tab. The preview-slot reuse system has been removed.
- **Tab group locked on restore** — `isLocked`/`isMaximized` group state is no longer persisted; lock/maximize are session-only, eliminating a cold-start deadlock where new tabs routed to an inv[...]

### Changed
- **Context menu tokens** — dark `#1f1e1e` background, solid primary hover block, `var(--radius)` corners, no border, `scaleY` pop animation. Danger items (Move to Trash, Delete Permanently) kee[...]
- **All `<select>` elements replaced** — terminal shell picker and settings font picker now use `CustomDropdown`.
- **Git store auto-refreshes after agent commands** — `agent://cmd_done` triggers a git re-poll so tree badges update after the agent runs a build or install.
- **Undo / Redo in toolbar** — split-pane buttons replaced with Undo (↩) and Redo (↪). Clicking is identical to `Ctrl+Z` / `Ctrl+Y`.
- **Split pane removed** — Split Editor commands removed from registry, View menu, editor and tab right-click menus.
- **Terminal tab-strip now scrollable** — thin scrollbar appears when sessions overflow; was invisible and unscrollable before.

### Performance
- **Dropped `mermaid` npm package** — removed 3 MB of bundled diagram libraries (cytoscape, dagre, swimlanes, etc.). Replaced with `mermaid-lite.ts` at ~10 KB.
- **KaTeX lazy-loaded** — moved from a static import to a dynamic import; only fetched when a markdown file containing `$` math syntax is opened. Stays out of the cold-start bundle.
- **Release exe: 10.3 MB → 4.95 MB** — dist shrunk from 5.2 MB to 1.9 MB after the above changes.

### Infrastructure
- Version bumped `0.1.10` → `0.1.12` across `tauri.conf.json`, `frontend/Cargo.toml`, `frontend/ui/package.json`.
- `cargo test -p gwenland-engine`: **455 passed**.
- `pnpm test`: **82 passed** (9 suites).

---

## [0.1.8] — 2026-06-26 (M17 Stability Patch)

### Fixed
- **GWEN-357 — Preview tab content stale on file switch** — `Editor.svelte` `persistMounted()` was called before `mountTab()` during preview-slot replacement, overwriting the new file's `Edit[...]
- **GWEN-361 — Bulk delete UI freeze** — `moveToTrash` and `deletePath` calls in `fileActions.ts` and `TreeNode.svelte` now yield the render thread with `await new Promise(r => setTimeout(r, [...]
- **GWEN-358 — "Delete Permanently" missing from context menu** — `file.deletePermanently` action (`Shift+Del`) restored to the file-tree context menu. "Move to Trash" now routes to the OS-na[...]

### Added
- **Double-click tab header to pin** — double-clicking an italic preview tab in the tab bar promotes it to a permanent pinned tab (same as VS Code behaviour). `pinTab()` function added to `tabs[...]
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
