# M15 Critical Fixes: reload-safe workbench, command wiring, highlighting, editor UX, and web preview

- **Date:** 2026-06-25
- **Issue:** GWEN-348 -> GWEN-352
- **Milestone:** M15 - Critical IDE stabilization

---

## Overview

M15 fixes the workbench-level regressions that made the IDE feel fragile after the earlier agent and safety work. The main change is that GwenLand IDE now survives a WebView refresh without losing the user's workspace shape: open tabs, active tab, layout state, theme, terminal visibility, and the current chat surface are persisted under `.gwenland/`.

The same pass also replaces scattered shortcut/menu logic with one command registry, restores syntax highlighting across editor and rendered AI code blocks, adds expected editor affordances like Ctrl+Click and image previews, and lets the Web Preview command pick up a running dev server port from terminal output.

Zero new Rust crates and zero new frontend dependencies were added.

---

## GWEN-348 - Workspace State Persistence

**Problem:** Ctrl+R refreshed the Tauri WebView and wiped the active IDE state.

**Fixes**

- Added `.gwenland/workspace.json` and `.gwenland/layout.json` load/save helpers in the engine workspace module, using the same local-first style as the rest of `.gwenland/` storage.
- Added Tauri commands and typed frontend wrappers for saving and loading workspace/layout state.
- Added `workspace-state.ts`, a debounced app-level persistence store that saves meaningful tab, layout, terminal, theme, and chat-state changes after 500 ms.
- Restores state after opening a workspace:
  - open tabs are reopened in order
  - missing files are skipped gracefully
  - the active tab is restored when it still exists
  - sidebar, bottom panel, terminal, sizes, and theme are restored
  - the chat pane's selected conversation/provider/model and unsent input are restored where possible
- Missing, empty, or malformed JSON files fail open and start fresh instead of crashing the app.
- Ctrl+R / Cmd+R is intercepted in the global keybinding handler so the WebView no longer reloads accidentally.

---

## GWEN-349 - Command Registry and Menu Bar Wiring

**Problem:** menu items and keyboard shortcuts were split across components or left unwired.

**Fixes**

- Added `frontend/ui/src/lib/commands/registry.ts` as the single command registry with command id, title, category, optional default keybinding, optional `when`, and handler.
- Added `frontend/ui/src/lib/commands/keybinding-handler.ts` for global shortcut normalization and dispatch.
- Wired all MVP shortcuts:
  - Command Palette, Quick Open, New File, Save, Save As
  - sidebar, bottom panel, terminal, new terminal, split editor
  - comment toggle, formatting placeholder, line move/copy/delete, multi-select commands
  - definition, rename, file find/replace, workspace find, settings, AI chat, inline AI prompt
- Reworked `MenuBar.svelte` so File, Edit, Selection, View, Go, Run, Terminal, and Help all dispatch through the command registry instead of hardcoded local handlers.
- Kept older command-store consumers working through a compatibility shim over the new registry.
- Preserved existing Git command palette entries by registering them through the same system.

Some commands are intentionally placeholders where the underlying backend/editor feature does not exist yet, but the command ids, shortcuts, and menu wiring are now real and centralized.

---

## GWEN-350 - Global Syntax Highlighting

**Problem:** syntax highlighting was inconsistent across editor surfaces.

**Fixes**

- Added `frontend/ui/src/lib/editor/language-detect.ts` with file-extension based CodeMirror language selection.
- `createEditorState()` now receives the file path and installs the matching language extension when an editor tab opens.
- Covered TypeScript/JavaScript, Rust, Python, CSS/SCSS, HTML/Svelte, JSON/JSONL, and Markdown/MDX using the CodeMirror packages already present in the project.
- Added comment-token language data so editor commands like Toggle Comment work with the detected language.
- Added pure tests for language detection.
- Verified XTerm ANSI color support stayed enabled and did not replace XTerm or add a renderer dependency.
- Kept AI fenced-code rendering on the existing CodeMirror-backed code block path.

---

## GWEN-351 - Editor UX Fixes

**Problem:** expected editor affordances were missing or visually broken.

**Fixes**

- Added Ctrl+Click / Cmd+Click handling in CodeMirror:
  - `http` and `https` tokens open in the browser
  - file-like tokens resolve relative to the active file directory or workspace root and open as tabs
  - trailing punctuation and `:line[:col]` suffixes are stripped before opening
- Added image preview handling:
  - image extensions route to a preview tab instead of UTF-8 text loading
  - static image previews render as centered images through the Tauri file asset path
  - non-image previews still use the existing iframe path
- Centered the tracked prompt dialog overlay by fixing its CSS to use full-screen centering.

---

## GWEN-352 - Web Preview Command Palette and Port Detection

**Problem:** Web Preview had to be opened manually even when terminal output already showed the dev server URL.

**Fixes**

- Added `frontend/ui/src/lib/terminal/port-detect.ts` with ANSI-stripping port and URL detection for common dev-server output:
  - Vite-style `Local: http://localhost:<port>`
  - `listening on`, `ready on`, `started server on`, `running at`
  - plain trailing `:<port>` and `port: <port>` forms
- Terminal output now keeps a small rolling buffer and updates the terminal store whenever a preview target is detected.
- Terminal store now tracks `detectedPort`, `detectedUrl`, and the session that produced it.
- Added `workbench.openWebPreview` to the command registry and View menu.
- `Ctrl+Shift+W` opens the detected localhost preview when available; otherwise it prompts for a URL or port and opens that as a preview tab.
- Added pure tests for port detection.

---

## Files Changed

| File | Change |
|---|---|
| `engine/src/workspace.rs` | Added `.gwenland/workspace.json` and `.gwenland/layout.json` path helpers plus atomic JSON load/save and tests |
| `frontend/src/main.rs` | Added workspace/layout state commands and `path_exists` |
| `frontend/ui/src/lib/tauri/commands.ts` | Added typed wrappers and persisted-state DTOs |
| `frontend/ui/src/lib/stores/workspace-state.ts` | New app-level persistence/restore store |
| `frontend/ui/src/App.svelte` | Mounted workspace persistence and global keybinding handler |
| `frontend/ui/src/lib/commands/registry.ts` | New centralized command registry and menu descriptors |
| `frontend/ui/src/lib/commands/keybinding-handler.ts` | New global keydown handler with reload prevention |
| `frontend/ui/src/lib/components/MenuBar.svelte` | Dispatches menu items through the command registry |
| `frontend/ui/src/lib/components/CommandPalette.svelte` | Uses the new command shape |
| `frontend/ui/src/lib/stores/commands.ts` | Compatibility shim over the registry |
| `frontend/ui/src/lib/context-menu/ContextMenuItem.svelte` | Shortcut labels come from the registry |
| `frontend/ui/src/lib/editor/language-detect.ts` | New language detection and CodeMirror language extension selection |
| `frontend/ui/src/lib/editor/language-detect.test.ts` | New language detection tests |
| `frontend/ui/src/lib/editor/codemirror-setup.ts` | Applies language extensions and Ctrl+Click handling |
| `frontend/ui/src/lib/editor/active-editor.ts` | Added wrappers for editor commands used by shortcuts |
| `frontend/ui/src/lib/components/Editor.svelte` | Resolves Ctrl+Click file paths relative to file/workspace context |
| `frontend/ui/src/lib/stores/tabs.ts` | Added image tab routing, Save As, and close-all support |
| `frontend/ui/src/lib/components/PreviewPane.svelte` | Renders image previews directly |
| `frontend/ui/src/lib/components/PromptDialog.svelte` | Fixed full-screen centered overlay |
| `frontend/ui/src/lib/terminal/port-detect.ts` | New terminal output port/URL detection |
| `frontend/ui/src/lib/terminal/port-detect.test.ts` | New port detection tests |
| `frontend/ui/src/lib/stores/terminal-sessions.ts` | Tracks detected preview target |
| `frontend/ui/src/lib/components/TerminalInstance.svelte` | Parses output chunks for preview target detection |
| `frontend/ui/src/main.ts` | Registers the new command registry |

---

## Validation

- `pnpm.cmd test` - passed, 8 files / 76 tests
- `pnpm.cmd check` - 0 errors, 0 warnings
- `pnpm.cmd build` - succeeded
- `cargo check --workspace` - succeeded
- `cargo test -p gwenland-engine workspace_state` - passed
- `cargo test -p gwenland-engine layout_state` - passed

Known non-blocking output:

- `pnpm build` still prints the existing Vite warnings about dynamic imports and chunk size.
- `cargo check --workspace` still prints the existing path canonicalization warning for `C:\Users\reyha`.

---

## Notes

- The command layer is now ready for future implementations to fill in the remaining placeholder commands without touching menu or shortcut wiring again.
- Workspace state is intentionally local to the opened project under `.gwenland/`, matching the local-first storage direction from M14.
- The change keeps all logic local-first and dependency-neutral: no network calls, no new crates, no new npm packages.
