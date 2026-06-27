# GwenLand IDE — Session Changes

**Date:** 2026-06-27 · **Milestone:** M19 — Performance & Scalability · **Version:** 0.1.12 → 0.1.14

---

## What changed this session

Eight waves of performance and scalability work, all under a hard constraint: **zero new Rust crates, zero new npm packages** — everything built from scratch on `std` / existing dependencies. The theme throughout is *Rust owns state, Svelte renders diffs* — the UI never receives a whole tree or a flood of events, only coalesced patches.

The result: the IDE stays smooth on the i3 / 8 GB target even with a 10k-file workspace open, a `cargo build` flooding the terminal, and a 5 MB log file in the editor.

Final gate: **487 engine tests**, **114 frontend tests**, release exe **4.77 MB** (budget ≤ 7 MB), zero new dependencies.

---

### Wave 1 — Batched file watcher (GWEN-376)

There was no file watcher at all before this — the tree only refreshed on a manual button press. Adding one normally means the `notify` crate, which the zero-crate rule forbids, and a hand-rolled native watcher (ReadDirectoryChangesW / inotify / FSEvents) is far too much `unsafe` for one wave.

So the watcher **polls**: a background thread re-`read_dir`s only the directories the UI has actually expanded (never a recursive walk) and diffs each against its previous snapshot. The poll interval *is* the coalescing window — a `npm install` writing thousands of files collapses into **one** `fs:patch` event per directory per cycle, never one event per file. `.git` internals are filtered out; git decorations refresh on a 500 ms debounce after the last patch.

- New `engine/src/fs_watch.rs` (tauri-free, callback-based like `PtySession`) + `stores/fs-watch.ts` bridge.
- 13 engine tests including the 1000-creates-in-one-patch case.

### Wave 2 — Virtualized file tree (GWEN-374 / GWEN-375)

The old tree rendered every expanded node as a recursive Svelte component — a 10k-file workspace meant 10k live components. Replaced with a **Rust-owned flat row model**: `engine/src/tree.rs` keeps the tree as one ordered `Vec<FlatRow>` plus an expanded-dir set, and every mutation (`expand` / `collapse` / `refresh_dir`) returns `TreePatch` deltas (`Insert` / `Remove` / `Update`). The frontend holds a mirror array and splices in patches — it never rebuilds from scratch.

`FileTree.svelte` renders it with a **scratch virtual scroller** (24 px rows, 20-row overscan), so only the visible window is ever in the DOM. Folders are lazy: children are listed on expand and arrive as Insert patches. The old recursive `TreeNode.svelte` was deleted.

- New `engine/src/tree.rs`, `components/FileTreeRow.svelte`, `stores/tree.ts`.
- The signal bus (`stores/file-tree.ts`) became a thin bridge to the new store, so context-menu actions and editor breadcrumbs needed no changes.
- 12 engine tests + 7 frontend patch-reconciler tests.

### Wave 3 — Large File Mode (GWEN-377)

Opening a `package-lock.json` froze the editor while every feature (syntax, LSP, minimap, folding) ran over it. New `file_meta()` in the engine reports size + line count (line counting skipped above 10 MB). On open, files are classified:

- **Large** (> 500 KB *or* > 10k lines) → reduced editor: no syntax highlight, autocomplete, lint gutter, inline diagnostics, bracket matching, minimap, sticky scroll; LSP `didOpen` is skipped.
- **Very large** (> 5 MB) → also read-only plain text.

A `⚡ Large File` badge appears in the status bar. Normal files are unaffected.

- `engine/src/fs.rs` `file_meta()`, `editor/large-file.ts`, `codemirror-setup.ts` `EditorMode`.
- 4 engine tests + 6 frontend classification tests.

### Wave 4 — Terminal frame-limiting (GWEN-378)

`cargo build` output was written to xterm one chunk at a time — one synchronous repaint per chunk, starving the editor's frame budget. New `terminal/terminal-scheduler.ts`: a per-session ring buffer + `requestAnimationFrame` scheduler coalesces every chunk arriving within a frame into a single `term.write` (~60 fps cap). It's also the pause point — while the tab is hidden or the window is backgrounded it buffers a bounded ~1 MB tail and writes nothing, flushing in one repaint on resume. Client-side scrollback capped at 50k lines.

- 6 frontend tests (rAF stubbed for determinism).

### Wave 5 — Low-End Mode (GWEN-379)

A single Settings → **Performance** toggle that strips expensive visual features for older hardware: git badges, indent guides, smooth scroll, minimaps, sticky scroll, animations, and file icons. The effect map lives in one place — a `perfSettings` derived store (`stores/performance.ts`) — and a `low-end` class on `<body>` globally kills animations/transitions without threading a flag through dozens of components. Persists across restart; a `⚡ Low-End` status-bar badge opens the setting.

- 3 frontend tests for the effect map.

### Wave 6 — Status-bar performance badges (GWEN-381)

A data-driven badge row on the right of the status bar, each wired to real app state: **Git** (scanning, spinner — backed by the new `git.refreshing` flag), **Indexing** (workspace first scan), **Large File** (Wave 3), **Low-End** (Wave 5), **Searching** (Wave 8, click to cancel), and **AI Running** (chat/agent activity, click to stop). Spinning badges use a CSS spinner and fade out over 300 ms when their state resolves; cancellable ones cancel the operation on click.

### Wave 7 — Optimistic file operations (GWEN-380)

Create / rename / delete / move now update the tree **immediately** (snapshot → apply local tree patches → run the FS op), rolling back with a toast if the operation fails. Watcher-driven refreshes are deferred while an optimistic op is in flight and replayed against the snapshot afterward, so the optimistic state and disk reconcile cleanly. A small file-op undo history backs Ctrl+Z on the tree.

- New `stores/file-ops.ts`; `stores/tree.ts` gained snapshot/restore + optimistic-op gating; `file-op-patches.test.ts`.

### Wave 8 — Workspace search (GWEN-382)

New `engine/src/search.rs`: a pure-`std` recursive walk that streams line matches through a callback, observes an `AtomicBool` for cancellation, and reuses the existing `search_policy` exclusions (`node_modules`, `target`, `dist`, `.git`, …). Tauri side emits `search:result` / `search:done` and exposes `search_cancel`. The UI search panel (Ctrl+Shift+F) debounces input 300 ms, cancels the previous search on each keystroke, groups results by file, and jumps to the line on click.

- New `engine/src/search.rs` + `stores/workspace-search.ts`. Binary delta well under the 50 KB budget.

---

## Files changed (highlights)

| Area | Files |
|------|-------|
| Engine — new modules | `engine/src/fs_watch.rs`, `engine/src/tree.rs`, `engine/src/search.rs` |
| Engine — extended | `engine/src/fs.rs` (`file_meta`), `engine/src/lib.rs` |
| Tauri glue | `frontend/src/main.rs` (fs-watch / tree / file-meta / search commands + managed state + events) |
| Frontend — new stores | `stores/tree.ts`, `stores/fs-watch.ts`, `stores/performance.ts`, `stores/file-ops.ts`, `stores/workspace-search.ts` |
| Frontend — new components | `components/FileTreeRow.svelte`, `terminal/terminal-scheduler.ts`, `editor/large-file.ts` |
| Frontend — reworked | `components/FileTree.svelte` (virtual scroll), `Editor.svelte`, `TerminalInstance.svelte`, `StatusBar.svelte`, `SettingsPage.svelte`, `stores/workspace.ts`, `stores/file-tree.ts`, `stores/editor-preferences.ts`, `stores/git.ts` |
| Removed | `components/TreeNode.svelte` (replaced by the flat model) |

---

## Gotchas recorded for future work

- The codebase had **no** file watcher before M19 (the milestone plan assumed one existed).
- Format-on-save does **not** exist in this codebase (the plan listed it speculatively for Wave 3).
- Git status and file icons are derived **JS-side** — they were deliberately kept out of the Rust row model to avoid a second source of truth.
- UI preferences (including Low-End Mode) persist via `localStorage` (`editor-preferences.ts`), matching every other UI preference, rather than `.gwenland/settings.json`.
- The polling file watcher was a deliberate choice over the `notify` crate to honor the zero-crate rule.

---

## Verification

- `cargo test -p gwenland-engine --lib`: **487 passed**.
- `pnpm test`: **114 passed** (15 suites).
- `svelte-check`: 0 errors.
- Release build: OK · exe **4.77 MB** (budget ≤ 7 MB).
- New Rust crates: **0** · new npm packages: **0**.
- Version bumped `0.1.12` → `0.1.14` across `frontend/Cargo.toml`, `frontend/tauri.conf.json`, `frontend/ui/package.json`.
