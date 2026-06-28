# GwenLand IDE - Session Changes

**Date:** 2026-06-28
**Scope:** GWEN-404 - Git Graph Infinite Canvas
**Status:** Frontend graph experience completed and validated

---

## What changed this session

Git history is now a first-class visual surface in the IDE instead of a plain
source-control tab. The graph opens as a floating window over the workbench,
renders commits on a Canvas2D timeline, and includes a compact navigation dock
for jumping by search result, branch, commit number, or date.

The UI stays local-first and lightweight: graph navigation filters the already
loaded payload in Svelte, the dock does not run git commands during hover,
search, pan, or zoom, and no new Rust crates or npm packages were added.

---

## Added

| Area | Change |
| --- | --- |
| Git graph floating window | Added a draggable, resizable IDE window for the Git Graph with refresh, maximize/restore, and close controls. |
| Canvas commit graph | Git history renders as a pan/zoom Canvas2D graph with frustum culling, selected-node highlight, branch colors, branch labels, and a HEAD marker. |
| Navigation dock | Added Find, Branch, Commit, and Date jump controls. The dock searches commit message, SHA, branch/ref names, author, and date from the loaded graph payload. |
| Dock jump behavior | Selecting a result pans to the commit with a short finite animation and selects the target node. |
| Commit detail popup | Clicked commits show details and can open the full commit diff view through existing tab infrastructure. |
| Git graph state | Added a small window state store for opening, closing, moving, resizing, and maximizing the floating graph. |

---

## Changed

| Area | Change |
| --- | --- |
| Source Control panel | The Graph toggle now opens the floating Git Graph window instead of routing the graph through a normal editor tab. |
| Dock styling | Reworked the dock away from a generic outlined glass toolbar. It now uses the app `--card` surface, compact workbench-style buttons, inset shading, and a subtle commit-lane rail. |
| Dock interaction | Dock auto-hides during graph pan/zoom and reappears after movement settles. |
| Graph refresh | Refresh from the Source Control panel targets the graph payload when the graph window is active. |

---

## Files changed

| File | Change |
| --- | --- |
| `frontend/ui/src/lib/components/git/GitGraph.svelte` | Canvas rendering, pan/zoom, labels, HEAD marker, selected-node navigation, dock integration. |
| `frontend/ui/src/lib/components/git/GitGraphDock.svelte` | New dock with Find, Branch, Commit, and Date navigation. |
| `frontend/ui/src/lib/components/git/GitGraphWindow.svelte` | Floating graph window shell. |
| `frontend/ui/src/lib/components/git/GitGraphPopup.svelte` | Commit detail popup and full diff handoff. |
| `frontend/ui/src/lib/stores/gitGraph.ts` | Graph payload loading and commit-details cache. |
| `frontend/ui/src/lib/stores/gitGraphWindow.ts` | Floating graph window state. |
| `frontend/ui/src/lib/types/git.ts` | Commit graph, branch, edge, and detail payload types. |
| `frontend/ui/src/lib/components/GitPanel.svelte` | Source Control graph toggle and graph-aware refresh behavior. |
| `frontend/ui/src/App.svelte` | Mounts the Git Graph floating window overlay. |

---

## Validation

| Gate | Result |
| --- | --- |
| `pnpm.cmd check` | Passed with 0 Svelte/TypeScript errors and 0 warnings. |
| `pnpm.cmd test` | Passed: 17 test files, 118 tests. |
| `pnpm.cmd build` | Passed. Vite still reports the existing large-chunk / dynamic-import warnings. |
| `git diff --check` | Passed. Only existing CRLF line-ending warnings were reported. |
| Local dev server | Responded with HTTP 200 at `http://localhost:5173/`. |

---

## Notes

| Topic | Note |
| --- | --- |
| Dependencies | Zero new Rust crates and zero new npm packages. |
| Runtime behavior | Hover, pan, zoom, and dock search do not call git. |
| Current scope | This entry documents the graph UI and docking work on top of the existing GWEN-404 graph payload implementation in this checkout. |
