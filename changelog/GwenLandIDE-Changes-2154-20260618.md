# Svelte Migration (M2.5), oklch Theme + Iconoir/Material Icons, and Windows Installers

- **Date:** 2026-06-18
- **Issue:** M2.5 (internal Svelte migration, untracked), plus polish + packaging follow-ups
- **Milestone:** Milestone 2.5 — Svelte Migration (Waves 1–5) + finishing touches

## Problem / Context

The frontend was a single ~4,675-line `frontend/dist/index.html` (vanilla JS + inline CSS) that had grown unwieldy: expensive to read/edit, all domains tangled in one file, and awkward to extend toward the new 3-panel layout. The goal of M2.5 was to rebuild the frontend as a modular Vite + Svelte + TypeScript app while preserving every shipped Milestone 1/2 behavior, with the Rust backend (`engine/`, `frontend/src/main.rs`) left untouched. After the migration, several rounds of polish followed (new oklch dark palette, animations, real icon sets) and the app was packaged into Windows installers, surfacing a few real bugs along the way.

## Change

**Repo-structure decision (confirmed before starting):**
- `frontend/` is itself the Tauri Rust crate (`frontend/src/main.rs`, `frontend/Cargo.toml`) — there is no `src-tauri/` split. The legacy monolith lives in `frontend/dist/index.html` (also the old build-output dir + CodeMirror bundle copy).
- New Svelte app placed in a fresh sibling folder `frontend/ui/`. `frontend/src/` (Rust) and `frontend/dist/` (reference) left in place. pnpm used throughout (not npm).

**Wave 1 — Scaffold & shell:**
- Vite + Svelte 5 + TS scaffolded in `frontend/ui/`; `@tauri-apps/api@2.11.x` matched to Tauri 2.11.2. Vite pinned to port 5173 (strictPort).
- Three-region shell (`App.svelte`): File Tree | Workspace | Terminal placeholder, plus a status bar. `panels.ts` store tracks size + collapsed per side panel with a 40px auto-collapse threshold and restore strips. Pointer-event drag-resize via `ResizeHandle`.
- Design tokens ported to `styles/tokens.css`; base reset + fonts wired.

**Wave 1.5 — Terminal moved below Workspace:**
- Restructured the layout so Terminal is a bottom panel (vertical resize) under the Workspace instead of a right-hand column, matching VS Code / Zed / JetBrains convention. Store dimension generalized to an axis-agnostic `size`; `ResizeHandle`/`RestoreStrip` made orientation-aware.

**Wave 2 — File Tree:**
- `FileTree.svelte` + recursive `TreeNode.svelte` render the tree from the existing `list_directory` command (backend already sorts dirs-first then case-insensitive — no re-sort on the frontend). Lazy-load children on expand. `workspace.ts` store + `openFolder()` via the existing `open_folder_dialog`; cancel is a no-op.

**Wave 3 — Workspace editor (largest wave):**
- Decision: import CodeMirror 6 as npm deps and let Vite bundle it, rather than reuse the committed IIFE vendor bundle. `entry.js` ported verbatim to `editor/codemirror-setup.ts` (same extensions, same custom VS Code-style search panel, no `lang-*`). Measured: bundle grew by ~318KB, matching the legacy ~310KB vendor bundle — no bloat.
- `tabs.ts` store: per-tab `EditorState` (cursor/scroll/undo preserved across switches), saved-content `baseline`, and dirty computed as `liveDoc !== baseline` (the bug-fixed semantics — opening never marks dirty, reverting clears it). `Editor.svelte` mounts one view for the active tab, snapshotting on switch. `Tabs.svelte` tab bar with dirty dot + close. `read_file`/`write_file` wrappers added. Ctrl/Cmd+S saves; dedup on reopen; binary files rejected.

**Wave 4 — Status bar + AI trigger:**
- `cursor.ts` store mirrors the active editor's Ln/Col out of CodeMirror (updates on `selectionSet`), shown in the status bar with hardcoded UTF-8. `ai-chat.ts` is an empty `{ isOpen }` store; the `[AI]` button toggles it but renders no chat (floating chat is a separate future milestone).

**Wave 5 — Command palette, settings, menu bar:**
- `commands.ts` registry (shortcuts + palette) ported from the legacy `shortcutRegistry`/`commandPalette`. `CommandPalette.svelte` opens on Ctrl+Shift+P, filters, runs on Enter/click. `settings.ts` + `SettingsPage.svelte`: 3 dark presets, accent pool + custom picker, font picker, persisted to localStorage (not settings.toml), applied live to CSS custom properties. `MenuBar.svelte` + `actions.ts`: File/Edit/Selection/View/Go/Run/Terminal/Help with the legacy item set; Open Recent fetched from `get_recent_projects`; disabled stubs preserved.

**Polish round (post-migration):**
- Swapped the palette to the provided oklch dark theme (`tokens.css` + the "Gwen Dark" preset kept in sync; localStorage key bumped to `gwen.theme.v2`). Added a lightweight `animations.css` (fade/pop/slide/rise keyframes, `prefers-reduced-motion` honored) applied to overlays, dropdowns, tabs, tree rows, and settings controls — no Tailwind installed, components stay on scoped CSS + `var(--token)`.
- Moved the Create File / Open Folder / Open Recent actions into the File Tree empty state (where they belong, VS Code-style); Workspace empty state reduced to a simple brand + hint. Added a new untitled-buffer `newUntitledFile()` and wired Ctrl+N.
- Installed `iconoir` for UI glyphs (`Icon.svelte`, SVGs imported via Vite `?raw`, `currentColor`) — close, chevron, folder, search, sparks, etc. Installed `material-icon-theme` for colored per-file-type icons (`FileIcon.svelte`, curated extension/name map → generic file fallback; folder open/closed) applied to tree rows and tabs. Chose the npm package over the root `fetch_icons.js` (which fetched from a GitHub raw URL — network-dependent and non-reproducible).

**Packaging + fixes found while packaging:**
- Wired `tauri.conf.json` build fields: `devUrl` → `http://localhost:5173`, `frontendDist` → `./ui/dist`, `beforeDevCommand`/`beforeBuildCommand` → `cd ../ui && pnpm <dev|build>`.
- Built Windows x64 installers (`bundle.targets: ["msi", "nsis"]`, icon set to `icons/icon.ico`): NSIS `-setup.exe` ≈ 1.10 MB, MSI ≈ 1.59 MB; standalone exe ≈ 3.31 MB. Confirmed NSIS uses `SetCompressor /SOLID lzma` (LZMA is the 7-Zip-grade algorithm — already optimal).
- Fixed a console window appearing alongside the app: added `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` to `frontend/src/main.rs` (GUI subsystem in release, console kept in debug for logs).

## Why this approach

The whole migration is frontend-only: no `engine/` changes and no new Tauri commands — the Svelte side just wraps the nine existing `#[tauri::command]`s via `@tauri-apps/api`. Behavior was sourced from the legacy monolith (and `engine/src/fs.rs`) as the spec rather than re-derived, so shipped nuances (dirty semantics, per-tab state, backend sort order, binary-file handling) carried over intact. CodeMirror moved to npm because the IIFE-on-a-global was an awkward fit for ESM/Vite and the size came out identical. Several config bugs only surfaced at real build/run time (`frontendDist` base-dir, `beforeDevCommand` CWD resolving to `frontend/editor`, the missing GUI subsystem attribute) and were fixed as found.

## Impact

- The IDE now runs on a modular Vite + Svelte + TS frontend; the monolithic `index.html` is no longer in the active path (kept only as reference).
- All Milestone 2 behavior (tabs, dirty tracking, command palette, settings, menu bar, file tree, editor) works as before, with a refreshed oklch theme, subtle animations, and proper Iconoir + Material file-type icons.
- Release binary measured at ~3.31 MB; Windows installers at ~1.1 MB (NSIS) / ~1.6 MB (MSI). No console window on launch.
- `engine/` was not touched at any point.
- **Things to keep in mind:** close-while-dirty still uses a native `confirm()` (the legacy styled dialog wasn't ported — a small follow-up). Cross-platform packaging (macOS `.dmg`, Linux `.deb`/`.rpm`/`.AppImage`) cannot be built from Windows and needs a GitHub Actions matrix; native ARM64 Windows is a separate build target. The file-type icon map is curated (common languages) rather than the full `material-icons.json` mapping. Saving an untitled buffer to a chosen path is not yet implemented. The legacy `fetch_icons.js` at the repo root is now superseded and can be deleted.
