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

## [0.1.22] — 2026-07-01 (AI Diff & Attachment UI)

Two display-layer follow-ups to the M26 AI work, both presentation-only (no new engine logic, no new dependencies). The diff viewer gained a Unified ↔ Split toggle extracted into a single reusable component shared by every diff surface, and files referenced in AI Chat now render as a proper attachment chip instead of a raw path.

### Added
- **Unified/Split diff view (GWEN-459)** — new reusable `DiffView.svelte` renders a parsed diff (`DiffFile[]`) as either a unified column or a side-by-side split, with a borderless view toggle, per-file header (filename + extension badge + `+N −N` stats), and old/new line numbers tracked independently. The split-pairing algorithm lives in pure, unit-tested `lib/ai/diff-rows.ts` (removed lines pair index-by-index with the following added lines; the shorter side gets blank filler cells so uneven blocks stay aligned).
- **File attachment chip (GWEN-460)** — new `FileAttachment.svelte` renders an attached/referenced file as a chip: icon by MIME/extension, truncated filename (extension stays visible), human-readable size, and a download (data-bearing) / open-in-editor (path ref) action. `sm` inline and `card` standalone variants, styled with GwenUI tokens to match the AI panel. Pure helpers (`attachmentIconSvg`, `formatFileSize`, `truncateFileName`) in `lib/ai/file-attachment.ts`, unit-tested.
- **Global diff color tokens** — `--diff-add-*` / `--diff-del-*` / `--diff-num` / `--diff-divider` added to `tokens.css` (green/red tuned to the warm `#1f1e1e` base, not GitHub hex), shared by all diff surfaces.
- **Persisted diff view mode** — `diffViewMode` added to `editor-preferences.ts` (localStorage), shared across surfaces.
- **`download` icon** added to `gwenland-icons.ts`.

### Changed
- **`GitDiffViewer.svelte` and `GitCommitDiffViewer.svelte`** both dropped their duplicate TS diff parsers + inline rendering and now parse via the engine (`parseDiff` → existing `parse_unified_diff`) and render through the shared `DiffView` — one source of truth, so the commit diff gets Unified/Split too.
- **`AiMessage.svelte`** renders user `file` attachments via `FileAttachment` (icon + name + size + open) instead of a plain path-text chip; selection/terminal-error chips and image thumbnails unchanged.
- **`ContextAttachment` / `ImageAttachment` TS types** extended with optional display-only fields (`size?`, `name?`) on the TypeScript mirror only — the Rust structs were left untouched since the engine neither needs nor persists them.

### Infrastructure
- Zero new Rust crates, zero new npm packages. Both tasks are frontend-only (no engine changes; GWEN-459 reuses the existing diff parser).
- `pnpm test`: 207 passed (2 pre-existing, unrelated `actionRegistry` failures), including 10 new `diff-rows` + 15 new `file-attachment` tests. `svelte-check`: 0 errors.
- The M23 AI accept/reject editor overlay was deliberately left untouched (it's a live-editor overlay, not a standalone renderer) — no regression to the accept/reject-per-hunk flow.
- Version bumped `0.1.21` → `0.1.22` across `frontend/tauri.conf.json`, `frontend/Cargo.toml`, `frontend/ui/package.json`.

---

## [0.1.21] — 2026-07-01 (M26 — Model Selector & AI UI Enhancements)

Foundational model-registry work plus a composer redesign. A single static catalog now covers all 9 AI providers with real brand icons, pricing, and per-model reasoning-effort metadata; the model picker and reasoning control were rebuilt against it; token usage is now tracked end-to-end and shown as a per-message cost line; and the composer toolbar was consolidated behind a single "+" button.

### Added
- **Model registry (GWEN-454)** — `engine/src/ai/model_catalog.rs`: `ModelEntry`/`Provider`/`Pricing`/`ReasoningConfig` schema, 39 seeded models across Anthropic/OpenAI/Google/xAI/DeepSeek/Z.AI/Moonshot/Qwen/Mistral, exposed via a new `ai_model_catalog` Tauri command. Unverifiable pricing is marked `// TODO: verify pricing` rather than guessed.
- **Provider brand icons (GWEN-455)** — real logomarks (not placeholders), sourced from `simple-icons` (CC0) and Wikimedia Commons, rendered in each provider's actual brand color/gradient.
- **Model selector UI (GWEN-456)** — `ComposerModelMenu.svelte` rebuilt as a flat, borderless, compact dropdown listing every catalog model with icon, name, context window, and $/1M pricing, sized to match the composer's own width.
- **Reasoning/effort control (GWEN-458)** — new `ReasoningMenu.svelte`, separate from the model picker since levels are per-model (Anthropic offers low/medium/high/xhigh/max, Grok offers none/low/medium/high, several models offer none at all).
- **Token usage tracking (GWEN-457)** — all 4 provider adapters now report input/output token counts (OpenAI required a new `stream_options.include_usage` request field to unlock this); persisted to conversation history (backward-compatible with old JSONL); shown as a per-message `"1.2K in · 340 out · $0.008"` footer, priced using the model that actually generated that reply.
- **Composer "+" menu** — new `ComposerActionsMenu.svelte` consolidates context-attach, image upload, assistant mode (Ask/Edit/Agent), and agent approval tier behind one button.

### Changed
- Composer toolbar reduced from `[attach] [Mode] [Model] [Effort] [Tier] [send]` to `[+] [Model] [Effort] [send]`.
- Composer dropdowns are now borderless with rounded corners, compact padding, and a springy pop-in animation instead of bordered boxes with a flat fade.
- `lib/ai/reasoning.ts` rewritten from hardcoded model-name regex heuristics to pure catalog lookups.

### Infrastructure
- Zero new Rust crates, zero new npm packages.
- `cargo test -p gwenland-engine`: 574 passed (1 pre-existing, unrelated LSP smoke-test failure). `pnpm test`: 182 passed (2 pre-existing, unrelated failures). `svelte-check`: 0 errors.
- Version bumped `0.1.14` → `0.1.21` across `frontend/tauri.conf.json`, `frontend/Cargo.toml`, `frontend/ui/package.json`.

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
