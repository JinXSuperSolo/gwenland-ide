# GwenLand IDE - Session Changes

**Date:** 2026-06-27

---

## What changed this session

This session focused on production stability for the bundled Tauri app: the
release build no longer freezes during normal IDE interactions, Windows no
longer flashes external terminal windows during routine work, and startup now
defers expensive services until after the UI can paint.

The end result is a smaller, calmer production build that stays responsive when
clicking files, expanding folders, typing, opening terminals, and running the
bundled app outside the dev server.

---

## Fixed

| Area | Change |
| --- | --- |
| Production Tauri freeze | Converted heavy Tauri commands to async handlers and moved blocking filesystem, Git, tree, watcher, safety, history, and process work through `spawn_blocking`. |
| Windows child process flicker | Added hidden-window process launch flags for Windows shell, Git, validation, terminal-tool, LSP, recovery, browser, and OS-trash subprocesses. |
| Terminal startup | Terminal sessions now start only when the user opens or creates a terminal; restored workspace state no longer auto-spawns a PTY. |
| Terminal retry behavior | Terminal session startup is idempotent with `idle`, `starting`, `running`, and `failed` states, so failed spawns show a controlled error instead of retrying forever. |
| Git refresh pressure | Git and other workspace services are deferred after first paint instead of starting directly in the initial render path. |
| File tree expansion | Folder expansion loads and refreshes only the relevant expanded directory instead of rebuilding the whole workspace tree. |
| File watcher refreshes | Content-only file modifications no longer force unnecessary tree refresh work; structural directory patches still refresh the affected folder. |
| File tree UI state | Selection, focus, and active-editor tracking are split into a separate tree interaction store instead of being mixed into tree row data. |
| Production bundle target | Windows Tauri bundling now targets NSIS only, avoiding the MSI/WiX path that failed when the Windows Installer service was unavailable. |

---

## Changed

| Area | Change |
| --- | --- |
| Release profile | Tightened release size settings with `opt-level = "z"`, LTO, one codegen unit, `panic = "abort"`, stripping, no debug info, no split debug info, and no incremental release build artifacts. |
| Process logging | Removed the temporary process-spawn JSONL logging layer and its redacted spawn records. Crash and safety redaction remain where they protect user data. |
| Local planning | Added `internal-plan/` to `.gitignore` so local planning notes stay out of public commits. |
| Next roadmap | Added `NEXT-PLAN.md` with the next startup and Git Graph work items. |

---

## Validation

| Gate | Result |
| --- | --- |
| `cargo fmt --all` | Passed, with a path-canonicalization warning only. |
| `cargo check --workspace` | Passed. |
| `pnpm.cmd check` | Passed with 0 Svelte/TypeScript errors. |
| `cargo tauri build --debug` | Passed and produced an NSIS debug bundle. |
| `cargo tauri build` | Passed and produced an NSIS release bundle. |

---

## Release artifact sizes

| Artifact | Size |
| --- | ---: |
| `target/release/GwenLand-IDE.exe` | 7.09 MB |
| `target/release/bundle/nsis/GwenLand IDE_0.1.14_x64-setup.exe` | 2.68 MB |

---

## Notes for users

| Topic | Note |
| --- | --- |
| Normal interactions | Clicking, selecting files, expanding folders, and typing should stay responsive in debug and release bundles. |
| Terminal | A terminal process is started only after an explicit terminal action. |
| Git | Git status should not run on every UI interaction. |
| Windows console windows | Normal interactions should not flash `cmd.exe`, PowerShell, Windows Terminal, Git, Node, or language-server console windows. |
| Failed spawns | Failed process starts should surface as controlled UI errors instead of infinite retry loops. |
