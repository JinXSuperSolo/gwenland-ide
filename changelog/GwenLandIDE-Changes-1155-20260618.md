# Command Palette, Full Menu Bar, Open Recent, and a Workspace-Switch Data-Loss Fix

- **Date:** 2026-06-18
- **Issue:** GWEN-241, GWEN-288 (follow-up), GWEN-240 (workspace-switch guard)
- **Milestone:** Milestone 2 — Core IDE (Wave 6 + Wave 5 follow-up)

## Problem / Context

This stretch finished the command palette (the last planned Milestone 2 feature), completed the titlebar menu bar that the first GWEN-288 pass left half-done, wired Open Recent to the real recent-projects store, and — most importantly — fixed a data-loss bug found while testing the Open Recent flow: switching workspaces could silently drop an in-flight save and never warned about unsaved changes. The palette also got a visual rework after the first version looked generic and had a broken keyboard path.

## Change

**Command palette (GWEN-241):**
- Added the palette overlay, input, and results list, plus the `commandPalette` object: it builds its command list straight from the shortcut registry, filters by label/id, and runs the matched shortcut's action on Enter or click. Opens via Ctrl+Shift+P or View → Command Palette.
- Fixed a real bug: the keyboard wiring ran at script-parse time, before the palette DOM existed near `</body>`, so it silently bailed and arrow keys/Enter/Escape did nothing. Moved it to run on `DOMContentLoaded`.
- Reworked the look to read like VS Code rather than a generic box: a real search field with focus ring and an Esc hint, command rows with a category prefix (File:, Tabs:, View:…), shortcuts rendered as individual key chips, and a clean rounded selected-row highlight (an earlier accent-bar version looked off and was removed).

**Menu bar completion (GWEN-288 follow-up):**
- Added the remaining titlebar menus — Go, Run, Terminal, Help — following the same descriptor + dropdown pattern as File/Edit/Selection/View, with the same "disabled, not hidden" convention for anything without a backing feature. Run and Terminal are all-disabled placeholders (no run/debug or PTY yet); Go has a working "Go to File" (opens the palette) and a disabled "Go to Line"; Help has a working "About" dialog.
- Open Recent submenu, populated live from the recent-projects store — most-recent-first, capped at 10 by the store, each entry showing the full path.
- Selecting a recent workspace now asks "Open this workspace?" first (OK / Cancel) instead of opening immediately, reusing the existing styled dialog.
- "About GwenLand IDE" reuses the same dialog component (extended to support a single-button info variant) to show the app name and version.

**Workspace-switch safety fix:**
- Switching workspaces now goes through one guarded path that waits for any in-flight save to finish, then — if any tab has unsaved changes — shows the existing unsaved-changes dialog and only proceeds on confirm. Clean state switches immediately, as before.
- `saveActiveFile` now snapshots the target path and content before awaiting the write, tracks its in-flight promise, and only updates the dirty/baseline bookkeeping if the tab still exists afterward.

**Other:**
- The welcome screen no longer shows the Recent Projects / Shortcuts panels when a workspace is already open — that state now shows only a short hint. The full welcome content is reserved for the true no-folder-open state.

## Why this approach

Everything here is frontend-only — no engine changes and no new Tauri commands. Open Recent reuses the recent-projects storage that already exists, and OK in the confirmation dialog reuses the same "open this known path" flow as the folder picker, just skipping the native dialog. The dropped-save bug was a sequencing problem, not a missing feature: the save was async but teardown was synchronous, so the fix was to make the switch await the pending save and gate it behind the dirty check rather than rewriting the save or dirty model.

## Impact

- The command palette works end to end with keyboard navigation and looks consistent with the rest of the IDE.
- The full menu bar is present and stable; every item either does something real or is clearly greyed out.
- Open Recent reflects the actual `recent_projects.json` contents, not a mock list, and opening a recent or a new folder can no longer silently lose unsaved work.
- The welcome screen only appears in the genuinely-empty state.
- **Things to keep in mind:** three items were deliberately left out and need follow-ups. "Clear Recently Opened" is omitted because clearing the store needs a new engine command that's intentionally out of scope here. "New Window" was dropped from the open-workspace dialog because the app has no multi-window mechanism yet, so the dialog is OK/Cancel only. "Open Repository" in the Help menu is a disabled stub because the project has no repository URL configured anywhere — it'll be wired once a real URL exists rather than guessing one.
