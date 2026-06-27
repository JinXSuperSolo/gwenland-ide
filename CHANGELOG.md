# Changelog

All notable changes to GwenLand IDE are documented here.

---

## [Unreleased]

---

## [0.1.14] — 2026-06-27 (M19 — Performance & Scalability)

Eight waves of performance work under a hard **zero new Rust crates / zero new npm packages** rule — everything from scratch. Theme: Rust owns state, Svelte renders diffs (the UI never receives a whole tree or an event flood, only coalesced patches). Targets the i3 / 8 GB machine: smooth with a 10k-file workspace, a `cargo build` flooding the terminal, and a 5 MB file in the editor.

### Added
- **Batched file watcher (GWEN-376)** — new polling watcher (`engine/src/fs_watch.rs`, no `notify` crate) that snapshots only expanded directories and diffs them. A burst (e.g. `npm install` writing thousands of files) coalesces into **one** `fs:patch` per directory per cycle, never one event per file. `.git` internals filtered out; git decorations debounced 500 ms.
- **Virtualized file tree (GWEN-374 / GWEN-375)** — Rust-owned flat-row model (`engine/src/tree.rs`): the tree is one ordered `Vec<FlatRow>` and every mutation returns `TreePatch` deltas (Insert/Remove/Update). The UI splices patches into a mirror array and renders it with a scratch virtual scroller (only the visible window + 20-row overscan are in the DOM). Folders list children lazily on expand.
- **Large File Mode (GWEN-377)** — new `file_meta()` engine command classifies files on open: **large** (> 500 KB or > 10k lines) drops syntax/LSP/autocomplete/lint/minimap/sticky-scroll and skips LSP `didOpen`; **very large** (> 5 MB) also opens read-only plain text. `⚡ Large File` status-bar badge.
- **Low-End Mode (GWEN-379)** — Settings → Performance toggle that disables git badges, indent guides, smooth scroll, minimaps, sticky scroll, animations, and file icons via a single `perfSettings` derived store + a `low-end` body class. Persists across restart; `⚡ Low-End` status-bar badge.
- **Status-bar performance badges (GWEN-381)** — data-driven badge row: Git scanning, Indexing, Large File, Low-End, Searching, and AI Running. Spinning badges fade out on resolve; cancellable ones (search, AI) cancel on click.
- **Optimistic file operations (GWEN-380)** — create/rename/delete/move update the tree immediately and roll back with a toast on failure; watcher refreshes are deferred during the op and reconciled against the snapshot. Ctrl+Z file-op undo on the tree.
- **Workspace search (GWEN-382)** — new pure-`std` engine module (`engine/src/search.rs`): recursive walk, streamed line matches via callback, `AtomicBool` cancellation, `search_policy` exclusions. UI panel (Ctrl+Shift+F) debounced 300 ms, cancels the previous search per keystroke, groups results by file, jumps to line on click.

### Changed
- **Terminal output is frame-limited (GWEN-378)** — a per-session ring buffer + `requestAnimationFrame` scheduler (`terminal/terminal-scheduler.ts`) coalesces all chunks arriving within a frame into one `term.write` (~60 fps), so a `cargo build` flood no longer drops editor frames. Output pauses (bounded ~1 MB buffer) while the tab is hidden or the window is backgrounded, flushing in one repaint on resume.
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
- **File type icons in the file tree** — every tree node shows a proper icon for its file type (55+ extensions and special filenames like `package.json`, `Cargo.toml`, `.gitignore`). Folder nodes show open/closed variants based on expand state.
- **Git status decorations** — `M` / `A` / `U` / `D` letter badges on tree nodes; colored dirty dots on tabs; `↑N ↓N` ahead/behind counter in the status bar. Engine: new `ahead_behind()` in `git.rs` + unit test (455 total).
- **AI agent command streaming** — `run_terminal_tool` is now fully async. Stdout/stderr stream line-by-line via `agent://cmd_output` events; `agent://cmd_done` fires on exit. No more UI freeze during `npm install` or long builds.
- **Agent kill button** — red ✕ in the command gate kills the running process tree (`taskkill /F /T` on Windows, `kill -TERM` on Unix) via new `agent_kill_terminal` Tauri command.
- **`CustomDropdown` component** — fully styled replacement for all native `<select>` elements. Keyboard nav (Arrow Up/Down, Enter, Escape), inline SVG icons per item, compact mode, `role="listbox"/"option"` accessibility.
- **Shell picker icons** — terminal panel shell selector uses `CustomDropdown` with inline SVGs for PowerShell, CMD, WSL, Bash, Zsh, Node, Python, and a generic fallback.
- **Mermaid diagrams in markdown preview** — zero-dependency in-house renderer (`mermaid-lite.ts`, ~300 lines) handles flowchart / graph (all directions, four node shapes, four edge styles), sequenceDiagram (actors, lifelines, solid/dashed arrows, self-calls, notes, dividers), and pie charts (arc slices, percentage labels, legend, title). Renders as inline SVG in the GwenLand dark palette.
- **Markdown preview panel** — live split-pane preview for `.md` files with GFM tables, task lists, blockquotes, images, and KaTeX math (lazy-loaded on first `$...$` token).

### Fixed
- **AI agent UI freeze** — `run_terminal_tool` was synchronous, blocking the entire Tauri runtime. Now async with `spawn_blocking` I/O readers.
- **Multi-tab broken** — single click always opens a real permanent tab. The preview-slot reuse system has been removed.
- **Tab group locked on restore** — `isLocked`/`isMaximized` group state is no longer persisted; lock/maximize are session-only, eliminating a cold-start deadlock where new tabs routed to an invisible group.
- **Context menu hover style** — hovered item now fills with solid `var(--primary)` block (matching the screenshot reference), with dark text on the warm background. Previously was a faint rgba tint.
- **Context menu radius** — container now uses `var(--radius)` (`1rem`) from global tokens instead of the old hardcoded `0.5rem`.

### Changed
- **Context menu tokens** — dark `#1f1e1e` background, solid primary hover block, `var(--radius)` corners, no border, `scaleY` pop animation. Danger items (Move to Trash, Delete Permanently) keep red label + red-tinted hover.
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
