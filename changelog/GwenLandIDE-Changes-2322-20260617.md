# Editor Core, Multi-file Tabs, Shortcuts, and a Real Settings Page

- **Date:** 2026-06-17
- **Issue:** GWEN-239, GWEN-237, GWEN-238, GWEN-240, GWEN-243, GWEN-242, GWEN-288
- **Milestone:** Milestone 2 — Core IDE (Waves 3–5 + extras)

## Problem / Context

By the end of Wave 2 you could open a folder, browse it, and read files into a placeholder textarea. This stretch turned that into an actual editor: a real CodeMirror 6 instance, atomic saving, proper multi-file tabs with unsaved-change tracking, a keyboard-shortcut layer, the titlebar menus, and — beyond the original plan — a Settings page with theming and font choices. A fair amount of this entry is also polish driven by hands-on testing: the search bar, the context menus, and the notification style all got reworked after seeing them in the running app.

## Change

**Editor core (Wave 3 — GWEN-239 / GWEN-237):**
- Added `frontend/editor/` — a CodeMirror 6 bundle built once via `build.sh` (esbuild → a committed `codemirror.bundle.js`, ~310 KB, never rebuilt by Tauri). The build also copies the bundle into `frontend/dist/editor/` because the webview only serves files under `dist`.
- Replaced the textarea with a `#cm-host` CodeMirror editor: line numbers, active-line highlight, bracket matching, undo history, 4-space indent, and search.
- Added the atomic `write_file` to the engine (`.tmp`-then-rename, same pattern as M1 settings) plus its command and property tests. Engine tests now sit at 18 passing, still with zero `tauri` in the engine dependency graph.

**Multi-file tabs (Wave 4 — GWEN-238 / GWEN-240):**
- Real tab bar with per-tab CodeMirror state, so cursor, scroll, and undo history survive switching tabs.
- Dirty tracking rebuilt around a saved-content baseline (a tab is dirty only when its text differs from what's on disk) after the first approach wrongly flagged freshly-opened files. Closing a dirty tab now asks for confirmation through a styled in-app dialog instead of the native `confirm()`.

**Interaction polish (Wave 5 — GWEN-243 / GWEN-242 / GWEN-288):**
- `shortcutRegistry` + `buildComboString`, with all the wave's shortcuts registered. Each shortcut calls `preventDefault` before its action so browser-native combos (Ctrl+S/F/W) don't leak through.
- Titlebar menus (File / Edit / Selection / View) driven by a single `MENUS` descriptor and a shared dropdown renderer; every in-scope item is wired to a real action and unsupported ones are greyed out.

**Polish from testing:**
- The CodeMirror search bar was restyled into a compact, floating VS Code-style widget — single row, icon buttons, the Aa / ab / .* toggles inside the field, a live match count, and a collapsible replace row.
- Code folding was removed: without a language grammar (Milestone 6) the fold arrows had nothing meaningful to fold, so they were misleading.
- Cut / Copy / Paste / Select All were wired up for real across the editor, terminal, and chat context menus, and the Edit/Selection titlebar menus now drive the live editor (undo/redo/find included).
- Every error path now flows through a single toast (errors get a red accent); the leftover `alert()`/`console.error`-only spots were folded in. The dirty-close confirmation stays a real dialog because that needs an actual yes/no answer.

**Settings + theming (beyond M2 scope, by request):**
- A Settings surface that opens two ways: Ctrl+Shift+, as a modal, or the titlebar gear as an editor page.
- A theme engine with three readable dark presets (Gwen Dark, Midnight, Slate), a 12-colour accent pool plus a custom accent picker, and a monospace font choice loaded over the Google Fonts CDN. Choices persist in localStorage and apply live to both the UI and the editor.
- The native `<select>` and color input were replaced with custom-styled controls so the panel matches the dark theme.

**Removed:**
- The light theme and the system/auto theme mode (light was unreadable). Dark is the baseline; the new preset system replaces the old dark/light toggle.
- The placeholder textarea editor and its hand-rolled line-number gutter (CodeMirror supplies its own).
- `foldGutter`/`foldKeymap` from the editor bundle, and the native `confirm()`/`alert()` calls.

## Why this approach

CodeMirror 6 was kept over Monaco on purpose: Monaco alone is 2 MB+ gzipped (it would blow the 7 MB budget), leans on a deprecated no-bundler loader, and is built around the LSP features this milestone defers. CodeMirror is ~310 KB and loads as one committed file with no runtime bundler. Theme and font settings persist in localStorage rather than the engine's `settings.toml` so the Rust schema and its tests stay untouched — that's a deliberate trade we can revisit if settings should sync through the engine later. Dirty tracking compares against a saved baseline instead of trusting editor change events, which is what made it reliable.

## Impact

- The full loop works end to end: open folder → browse → open files into real editor tabs → edit (with a dirty dot) → save atomically → close with an unsaved-changes guard.
- Keyboard shortcuts, titlebar menus, and right-click menus all do real work; notifications are consistent.
- Settings lets you restyle the IDE (preset, accent, font) and it sticks across restarts.
- `cargo test -p gwenland-engine` reports 18 passed, 0 failed, no warnings; the engine still has no `tauri` dependency.
- **Things to keep in mind:** Settings, custom theming, and the font picker are extras outside the Milestone 2 task list — useful, but not part of the original 12 features. Syntax highlighting and code folding are still deferred to Milestone 6 (both need the language layer). The terminal context menu's Paste and the file-tree's New/Rename/Delete are intentionally disabled because they need backends that don't exist yet. Ctrl+J now toggles the terminal (replacing the earlier Ctrl+backtick). Wave 6 (the command palette) is the remaining piece before the final checkpoint.
