# Workspace Foundation — Folder Open, Directory Listing, and Panel Resizing

- **Date:** 2026-06-17
- **Issue:** GWEN-234, GWEN-287
- **Milestone:** Milestone 2 — Core IDE

## Problem / Context

Milestone 1 left the IDE as a styled shell driven entirely by mock data — the file tree, tabs, and editor were all backed by an in-memory `MOCK_FILES` object with no connection to the real filesystem. Wave 1 of Milestone 2 roots the IDE to an actual folder on disk: the user picks a directory through a native dialog, and the explorer reads its real contents. Alongside that, the panel layout needed proper drag-to-resize behaviour for the explorer and terminal dividers, replacing the one-off terminal resize handler from M1.

## Change

- **Engine (`engine/src/fs.rs`):** Added a new filesystem module with `FsError` (covering I/O, invalid UTF-8, cancelled dialog, non-directory, and binary-file cases), a `DirEntry` struct, and `list_directory`, which returns a directory's immediate children sorted directories-first and then case-insensitively by name. The module is registered in `engine/src/lib.rs`.
- **Frontend commands (`frontend/src/main.rs`):** Registered two new Tauri commands. `list_directory` is a thin two-line wrapper over the engine. `open_folder_dialog` holds the native folder-picker logic directly in the frontend (using `tauri-plugin-dialog`) so that the dialog plumbing never pulls a Tauri dependency into the engine crate.
- **Permissions (`frontend/capabilities/default.json`):** Added a capability granting `core:default` and `dialog:default` for the main window, which the dialog plugin requires.
- **UI (`index.html`):** Introduced the canonical `workspaceState` and `panelState` objects that the rest of Milestone 2 will build on, a non-blocking toast helper, an "Open Folder" button in the explorer action row wired to the new command, a generic `makeDraggableResizer` helper, and a new explorer resize divider. Both the explorer and terminal dividers now route through the shared resizer, replacing M1's bespoke terminal-resize handler.
- **Tests:** Added example and property-based tests for `list_directory` (sort order and the non-directory case) using `proptest` and `tempfile`.

## Why this approach

Keeping the engine free of any `tauri::` dependency remains a hard rule from Milestone 1, so the folder-dialog logic was placed in the frontend command wrapper rather than the engine. This is the one deliberate exception to the "wrappers stay at two lines" convention — native dialog handling is inherently Tauri-side plumbing — and `engine` continues to verify clean: `cargo tree -p gwenland-engine` shows no Tauri crate. The Milestone 1 UI is a mock prototype with a different internal architecture than the one Milestone 2 targets, so rather than rewrite it all at once, the new real state objects were introduced alongside the mock layer; later waves retire the mock pieces as their tasks require. The single `makeDraggableResizer` helper was chosen over per-panel handlers so both dividers share one tested code path.

## Impact

- The explorer can now open and read real directories from disk, and both side and bottom panels resize by dragging.
- Engine verification passes: `cargo check -p gwenland-engine` is clean with no Tauri in its dependency graph, and `cargo test -p gwenland-engine` reports 9 passed, 0 failed (0 warnings).
- New dependencies were added only where required: `tauri-plugin-dialog` and `tokio` (sync feature, for the dialog's response channel) on the frontend, and `tempfile` as a workspace dev-dependency for the filesystem tests.
- **Things to keep in mind:** `open_folder_dialog` signals user cancellation by returning the `DialogCancelled` error variant as a string; the frontend treats a cancelled dialog as a no-op rather than an error toast. The next waves will replace the remaining M1 mock layer — the `renderExplorerTree` function (Wave 2) supersedes the mock file tree, and the CodeMirror editor (Wave 3) replaces the placeholder textarea.
