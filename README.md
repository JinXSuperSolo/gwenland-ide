# GwenLand IDE

## Directory Layout
- `frontend/`: Tauri 2 application shell.
- `engine/`: Pure Rust business logic crate.
- `index.html`: Single-file web interface.
- `Cargo.toml`: Workspace definition.

## Architecture
GwenLand IDE isolates all business logic and state within the `engine` crate. This guarantees unit-testability without a Tauri runtime and ensures portability to CLI, TUI, or web frontends in the future.

## Goals
- Target release binary size: ≤ 7 MB (v0.1)
