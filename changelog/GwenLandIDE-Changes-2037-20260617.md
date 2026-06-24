# Rust Edition 2024 Upgrade

- **Date:** 2026-06-17
- **Issue:** Pre-Milestone 2 chore
- **Milestone:** Milestone 2 — Core IDE (prerequisite)

## Problem / Context

The workspace crates were still pinned to the 2021 Rust edition. Before starting Milestone 2 work, we wanted the whole workspace on edition 2024 so that new code is written against the current language defaults from day one rather than migrated later. This was deliberately run as an isolated chore, separate from any feature task, so that any fallout could be attributed to the edition bump alone and not tangled up with new functionality.

## Change

- Bumped `edition = "2021"` to `edition = "2024"` in both member crates: `engine/Cargo.toml` and `frontend/Cargo.toml`.
- Left the root `Cargo.toml` untouched — it has no `[workspace.package]` edition field, so there was nothing to change there.

## Why this approach

The bump was kept as a standalone change with nothing else mixed in. The installed toolchain (rustc/cargo 1.95.0) is well past the 1.85 minimum that edition 2024 requires, so no `rustup` upgrade was needed. We checked the usual 2024-edition migration hazards — the new return-position `impl Trait` lifetime capture rules, the changed `if let` temporary drop timing, the `unsafe extern` requirement, and the prelude additions (`Future`/`IntoFuture`) — but none applied to the current codebase, which has no FFI, no `impl Trait` returns capturing generic parameters, and no name collisions with the new prelude entries.

## Impact

- `cargo build`, `cargo build --release`, and `cargo test` all pass with zero errors and zero warnings.
- The existing test suite (5 tests across `app_data`, `settings`, and `recent_projects`) passes unchanged.
- This is a pure edition migration — no feature behavior changed, and no dependency versions were touched.
