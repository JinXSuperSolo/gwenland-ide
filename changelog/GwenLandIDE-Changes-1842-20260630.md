# GwenLand IDE - Session Changes

**Date:** 2026-06-30
**Scope:** Full Keyboard Navigation (VSCode-style)
**Status:** Implemented and validated — 46 new behavioral tests, typecheck clean

---

## What changed this session

The IDE is now fully drivable from the keyboard, VSCode-style. The file tree
navigates with arrow keys (respecting expand/collapse), Escape closes exactly
one overlay per press through a single centralized stack instead of every modal
firing at once, Tab cycles focus across the major panes, Ctrl+Tab cycles editor
tabs from anywhere, the Insert key can no longer flip the editor into overwrite
mode, and a defensive OVR status-bar indicator appears if overwrite mode ever
activates anyway.

Every interaction is intentional and predictable: keys are consumed where they
should be (no page scroll leaking from the tree, no in-editor Tab hijacking),
and nothing new was added to the dependency tree. All decision logic lives in
small pure modules so each behavior is unit-testable without a DOM.

---

## Added

| Area | Change |
| --- | --- |
| File tree arrow navigation | Up/Down move selection between visible rows; Right expands a collapsed folder (or steps into the first child); Left collapses an expanded folder (or jumps to the parent); Enter opens a file or toggles a folder. Selection scrolls into the virtual viewport and stays focus-trapped. |
| Centralized Escape stack | A single overlay stack closes exactly the topmost open overlay per Escape press and no-ops when nothing is open, replacing the previous per-overlay listeners that all fired at once. |
| Tab pane-cycling | Tab / Shift+Tab cycle keyboard focus across Sidebar → Editor → Terminal (reverse on Shift+Tab), with an orange accent focus ring on the active pane. The terminal is skipped when collapsed or no workspace is open. |
| OVR status-bar indicator | A hidden-by-default "OVR" badge that surfaces only if overwrite mode is somehow activated, giving the user visible feedback that an unexpected state occurred. |

---

## Changed

| Area | Change |
| --- | --- |
| Ctrl+Tab editor-tab cycling | The existing MRU quick-switcher was kept and hardened: it now no-ops with 0 or 1 open tab and supports Ctrl+Shift+Tab to cycle in reverse, working regardless of which pane has focus. |
| Insert key | Hard-blocked at the highest-precedence editor keymap so it can never toggle overwrite/typeover. Shift+Insert is intentionally left alone so native paste still works. |
| Overlay Escape handling | About, Changelog, Settings, Command Palette, and Prompt dialogs no longer listen for Escape themselves; the context menu and tree-input keep local Escape but stop propagation so a single press never peels two layers. |
| File tree rows | Rows are removed from the tab sequence; the viewport owns keyboard focus and pulls DOM focus onto the selected row only while focus is already inside the tree (survives row virtualization). Per-row Delete is preserved. |

---

## Added files

| File | Purpose |
| --- | --- |
| `frontend/ui/src/lib/stores/tree-navigation.ts` | Pure arrow/Enter navigation logic for the flat virtual tree. |
| `frontend/ui/src/lib/stores/overlay-stack.ts` | Centralized Escape/overlay stack (register + topmost-only close). |
| `frontend/ui/src/lib/ui/overlay-setup.ts` | Wires every app overlay into the Escape stack. |
| `frontend/ui/src/lib/stores/pane-focus.ts` | Tab pane-cycling order, availability, and in-editor guard. |
| `frontend/ui/src/lib/stores/tab-cycle.ts` | Pure Ctrl+Tab MRU switcher index math. |
| `frontend/ui/src/lib/editor/overwrite-mode.ts` | Overwrite-mode safety-net store + view probing. |

---

## Files changed

| File | Change |
| --- | --- |
| `frontend/ui/src/App.svelte` | Routes Escape through the overlay stack; adds Tab pane-cycling and Ctrl+Tab hardening; pane focus ring + `data-pane` markers + focusin tracking; inits the overlay stack. |
| `frontend/ui/src/lib/components/FileTree.svelte` | Viewport-level arrow/Enter navigation, scroll-into-view, focus trap; tree-input Escape stops propagation. |
| `frontend/ui/src/lib/components/FileTreeRow.svelte` | Rows leave the tab sequence; row self-focuses when selected via keyboard; keeps Delete. |
| `frontend/ui/src/lib/editor/codemirror-setup.ts` | Adds the highest-precedence Insert-block keymap. |
| `frontend/ui/src/lib/components/StatusBar.svelte` | Adds the hidden-by-default OVR indicator. |
| `frontend/ui/src/lib/components/AboutDialog.svelte` | Drops local Escape (Enter still dismisses). |
| `frontend/ui/src/lib/components/ChangelogModal.svelte` | Drops local Escape window handler. |
| `frontend/ui/src/lib/components/SettingsPage.svelte` | Drops local Escape window handler. |
| `frontend/ui/src/lib/components/CommandPalette.svelte` | Drops local Escape (Arrow/Enter kept). |
| `frontend/ui/src/lib/components/PromptDialog.svelte` | Drops local Escape (Enter kept). |
| `frontend/ui/src/lib/context-menu/keybindings.ts` | Escape closes locally and stops propagation. |

---

## Validation

| Gate | Result |
| --- | --- |
| `pnpm.cmd test` (new behavioral tests) | Passed: 6 new test files, 46 tests, covering all 6 keyboard areas. |
| `pnpm.cmd test` (full suite) | 171 passing. The only 2 failures are pre-existing and unrelated (`actionRegistry.test.ts`). |
| `svelte-check` | 0 errors introduced by this work. The 1 remaining error (`AiPanel.svelte` `chat-teardrop` icon) is pre-existing and unrelated. |

---

## Notes

| Topic | Note |
| --- | --- |
| Dependencies | Zero new npm packages and zero new Rust crates. |
| Ctrl+Tab decision | The existing MRU quick-switcher was kept (rather than replaced with strict left-to-right cycling) and hardened, per product decision. |
| Pre-existing breakage | `actionRegistry.test.ts` (2 failures) and an `AiPanel.svelte` icon-type error already existed in the working tree from an unrelated uncommitted edit; they are not caused by this milestone. |
