# Foundation Setup & OS-Specific Core Engine

- **Date:** 2026-06-17
- **Issue:** GWEN-229, GWEN-231, GWEN-232, GWEN-233
- **Milestone:** Milestone 1 — Foundation

## Problem / Context

We needed to establish the initial foundation for GwenLand IDE, aiming for a super lightweight and minimalist architecture. The initial state was completely empty. The main challenges were keeping the core engine (`gwenland-engine`) as pure Rust without any Tauri dependencies, dynamically handling data path resolution based on the OS (Windows, macOS, Linux), ensuring data persistence is safe from application failures, and keeping the release binary size as small as possible (target < 7MB).

## Change

- **Workspace:** Created a structured Cargo workspace with two separate crates: `gwenland-engine` (pure Rust) and `frontend` (Tauri app).
- **Frontend / UI Shell:** Configured `tauri.conf.json` with the `skipInstall` option and the `protocol-asset` feature. Built a modern UI Shell (`index.html`) using flexbox, CSS custom properties for a base dark theme, an explorer file-tree skeleton, a status bar, and a welcome/empty state screen. Implemented basic IPC bindings using Vanilla Javascript.
- **Engine / Core:**
  - `app_data.rs`: Automatic folder resolution (`%APPDATA%`, `~/.config`, or `~/Library/Application Support`) depending on the operating system.
  - `settings.rs`: A loader and saver system for TOML settings utilizing an atomic file rename strategy (`.tmp` -> `.toml`) to avoid data corruption in the event of an application crash.
  - `recent_projects.rs`: JSON storage to track recently opened projects, complete with path deduplication logic and a hard limit of 10 maximum recent projects.
- **Testing:** Implemented `proptest` to guarantee the stability of core functionalities (such as reading/writing settings) through automated property-based testing.

## Why this approach

A strict separation between `engine` and `frontend` ensures the business logic is entirely isolated and can be tested independently without requiring the Tauri GUI environment. `proptest` was selected to ensure the file I/O logic is bulletproof, even when subjected to hundreds of random concurrent input iterations. For `settings.rs`, the strategy of writing to a temporary file and then renaming it was chosen because it is the simplest standard approach to prevent data loss. On the Tauri side, the `skipInstall` configuration and minimalist features (`wry` + `protocol-asset`) were strictly selected to keep the release bundle size as minimal as possible by excluding unnecessary resources.

## Impact

- **Future Dependencies:** Upcoming features (like the workspace manager or editor preferences) are now strictly required to depend on the `gwenland-engine` library and adhere to its core structures, such as `load_settings()` / `get_app_data_dir()`.
- **Things to Keep in Mind:** Rust's unit test system runs in parallel by *default*. When writing tests that perform I/O modifications on the global persistent directory paths (like `%APPDATA%`), there is a high likelihood of race conditions/file lock conflicts occurring between unit tests (which we have currently addressed by weakening an assertion). Moving forward, the team is forbidden from introducing any `tauri::` dependencies into the `gwenland-engine` crate.
