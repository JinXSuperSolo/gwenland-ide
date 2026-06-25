M14 — Local First Safety Foundation

- **Date:** 2026-06-25
- **Issue:** GWEN-M14 (all 7 waves)
- **Milestone:** M14 — Local First Safety Foundation

---

## Overview

M14 makes GwenLand IDE fully local-first. No feature in the core IDE requires cloud connectivity, mandatory sign-in, telemetry, or a remote policy service. All safety decisions, audit records, recovery artifacts, extension permissions, and crash reports live on disk under `.gwenland/` in the open workspace.

Zero new external Rust or npm dependencies were introduced across all seven waves.

---

## Wave 1 — Local-first Storage Contracts and Workspace Settings

**New file: `engine/src/workspace.rs`** (18 tests)

- Path helpers for every `.gwenland/` subdirectory: `settings`, `safety`, `snapshots`, `trash`, `backups`, `audit`, `agent`, `extensions`, `logs`
- `WorkspaceSettings` struct: theme override, accent color, editor font, terminal font, layout state, sidebar/panel state, keybindings, formatter, autosave, `safety_strictness` — every field `#[serde(default)]`, no secrets
- `SafetyStrictness` enum: `Standard` / `Strict` / `Paranoid`
- `load_workspace_settings` — fail-open (absent/malformed → all-`None` defaults)
- `save_workspace_settings` — atomic tmp-write + rename
- `is_inside_workspace` — path containment guard (rejects root itself)

**`engine/src/lib.rs`** — `pub mod workspace;` added.

**`frontend/src/main.rs`** — `workspace_load_settings`, `workspace_save_settings` commands registered.

**`frontend/ui/src/lib/tauri/commands.ts`** — `WorkspaceSettings` interface, `SafetyStrictness` type, `workspaceLoadSettings()`, `workspaceSaveSettings()` added.

---

## Wave 2 — Safety Engine and Protected Path Registry

**New directory: `engine/src/safety/`** (28 tests across 5 submodules)

- **`action.rs`** — `Actor` (User / Agent / Extension{id} / System), `SafetyActionKind` (18 variants: file read/write/delete/rename/move/copy, terminal exec, git push/destructive/clone-remote, AI context attach, extension permission, remote/export, unknown), `SafetyAction`
- **`decision.rs`** — `RiskLevel` (Safe / Low / Medium / High / Destructive / Secret / Remote / Unknown), `SafetyVerdict` (Allow / Ask / Block), `ConfirmationKind` (None / Simple / Typed / DangerAck{warning}), `SafetyDecision`
- **`protected_paths.rs`** — `ProtectedPathRegistry` with 29 default entries covering secrets (`.env`, `.env.*`, `*.pem`, `*.key`, `*.p12`, `credentials.json`, `.ssh/**`, `.aws/**`, `.gcloud/**`, `*.pfx`), VCS (`.git/**`), workspace store (`.gwenland/**`), lockfiles (`package-lock.json`, `Cargo.lock`, etc.), manifests (`package.json`, `Cargo.toml`, etc.); two-tier glob matcher (single-segment → basename match, multi-segment → rooted `**` match); load from `.gwenland/safety/protected-paths.json` with malformed-fallback to defaults
- **`confirmation.rs`** — `evaluate(action, registry, strictness) → SafetyDecision`; routes all 18 action kinds through strictness-aware rules; `Standard < Strict < Paranoid`; unknown actions fail conservative
- **`guards.rs`** — `check_file_action`, `file_write_allowed`, `check_terminal_command`, `check_ai_context` convenience helpers
- **`file_guard.rs`** — `preflight_file_mutation` → `PreflightOutcome` (Allowed{snapshot} / NeedsConfirmation / Blocked); agent mutation preflight gate

**`engine/src/lib.rs`** — `pub mod safety;` added.

---

## Wave 3 — Local Audit Log

**New file: `engine/src/audit.rs`** (7 tests)

- `AuditCategory` — Safety / Agent / Terminal / Git / Extension / Rollback; one JSONL file per category under `.gwenland/audit/`
- `AuditEvent` — id, timestamp, workspace_root, actor, category, kind, risk, verdict, reason, target_summary (redacted), correlation_id
- `AuditEvent::from_decision` — builds from a `SafetyDecision` + `SafetyAction`, auto-redacts `target_summary`
- `AuditWriter::append` — lazy dir create, atomic append-flush; `record_decision` convenience wrapper
- `AuditWriter::read_all` — skips malformed lines, never panics
- `should_block_on_audit_failure(risk)` — true for Destructive / Secret / High / Unknown

**`engine/src/lib.rs`** — `pub mod audit;` added.

---

## Wave 4 — Snapshot, Trash, Backup, and Rollback

**New file: `engine/src/recovery.rs`** (10 tests)

- `MAX_SNAPSHOT_BYTES` = 10 MiB; oversized files fail safely
- `create_snapshot` — copies source to `.gwenland/snapshots/<id>/`, returns `SnapshotRecord` with metadata
- `move_to_trash` — moves path to `.gwenland/trash/files/<id>/`, writes metadata to `.gwenland/trash/index.jsonl`
- `restore_from_trash` — restores to original path; requires `force: true` if conflict exists
- `create_git_patch_backup` — `git diff HEAD` stored under `.gwenland/backups/git-patches/`, metadata in `.gwenland/backups/index.jsonl`
- `rollback_from_snapshot` — restores snapshot; requires `force: true` on conflict; emits rollback audit event

**`engine/src/lib.rs`** — `pub mod recovery;` added.

---

## Wave 5 — Guard Integration, Search Policy, and Tauri Bridge

**New file: `engine/src/search_policy.rs`** (integrated into existing test suite)

- `should_exclude_from_search(path, workspace_root)` — excludes secret paths (`.env`, credentials, keys), generated/dependency dirs (`node_modules`, `target`, `dist`, `.git`), and blocked protected paths from the local search index
- `should_exclude_fast(path)` — no I/O fast-path for hot loops

**`frontend/Cargo.toml`** — `serde_json` promoted from `[dev-dependencies]` to `[dependencies]` (already in workspace; zero new external dep; required by `safety_evaluate` Tauri command).

**`frontend/src/main.rs`** — `safety_evaluate` command (JSON-deserialized `SafetyActionKind`), `search_should_exclude` command added and registered.

**`frontend/ui/src/lib/tauri/commands.ts`** — `RiskLevel`, `SafetyVerdict`, `ConfirmationKind`, `SafetyDecision` types; `safetyEvaluate()`, `searchShouldExclude()` typed wrappers added.

**`engine/src/lib.rs`** — `pub mod search_policy;` added.

---

## Wave 6 — Extension Permission Foundation

**New file: `engine/src/permissions.rs`** (8 tests)

Default permission matrix (requirement 12.5):

| Permission | Default |
|---|---|
| `read_workspace` | allowed |
| `write_file` | ask |
| `delete_file` | blocked |
| `run_terminal` | ask |
| `access_git` | ask |
| `access_env` | blocked |
| `access_database` | blocked |
| `<unknown>` | blocked |

- `Permission` enum (7 known variants + `Unknown`), `PermissionDefault` (Allowed / Ask / Blocked)
- `PermissionRegistry` — load/save at `.gwenland/extensions/permissions.json`; atomic save; malformed-fallback to empty defaults; `resolve(extension_id, permission)` applies per-extension overrides over the default matrix
- `ApprovalRecord` — bounded (256 chars) + secrets-redacted `target_summary`; JSONL append to `.gwenland/extensions/approvals.jsonl`
- `evaluate_permission` — routes permission requests through the Wave 2 Safety Engine (not a separate policy silo)

**`engine/src/lib.rs`** — `pub mod permissions;` added.

**`frontend/src/main.rs`** — `permissions_load_state`, `permissions_record_approval` commands added and registered.

**`frontend/ui/src/lib/tauri/commands.ts`** — `ExtensionPermission`, `PermissionDefault`, `PermissionDecision` types; `permissionsLoadState()`, `permissionsRecordApproval()` wrappers added.

---

## Wave 7 — Logs, Crash Reports, Offline Validation, and Non-regression

**New file: `engine/src/logs.rs`** (10 tests)

- `workspace_logs_dir` → `.gwenland/logs/`; `workspace_crash_dir` → `.gwenland/logs/crash/`; `app_crash_dir` → OS app-data `crash/` (no-workspace fallback)
- `CrashReport` — id, timestamp, app_version, platform, error_summary (512-char bounded, redacted), stack_excerpt (2048-char bounded, redacted); uses `agentic::policy::redact_secrets`; does NOT include full file contents, full terminal history, or API keys
- `write_crash_report` — lazy dir create, JSONL append
- `record_crash` — workspace-aware helper (workspace dir when open, app dir otherwise)
- `read_crash_reports` — malformed-line-skipping reader
- Upload/export is **manual and opt-in only**; any future export must pass through the Safety Engine as `SafetyActionKind::RemoteExport` with explicit user preview

**`engine/src/lib.rs`** — `pub mod logs;` added.

---

## Offline Smoke Checklist (7.4)

All of the following work with no network connection:

| Feature | Local path |
|---|---|
| Open folder | `fs::list_directory` |
| File tree | `fs::list_directory` |
| Read / edit / save file | `fs::read_file` / `fs::write_file` |
| Workspace settings | `.gwenland/settings.json` |
| Terminal local shell | PTY / ConPTY |
| Git status/diff on local repo | `git::status` / `git::diff_file` |
| Search (exclusion policy) | `search_policy::should_exclude_from_search` |
| Diff preview | `ai::parse_unified_diff` (pure parse) |
| Safety block/ask behavior | `safety::evaluate` (no network) |
| Audit log write | `audit::AuditWriter::append` |
| Trash / snapshot rollback | `recovery::move_to_trash` / `rollback_from_snapshot` |
| M13 local memory retrieval | `agentic::search_memory` |

---

## Dependency Review (7.5)

`git diff -- Cargo.toml engine/Cargo.toml frontend/ui/package.json` → **empty**.

No cloud SDK, sync daemon, vector DB, database, Git library, or indexing crate was added. The only change to `frontend/Cargo.toml` was promoting the already-present workspace `serde_json` from `[dev-dependencies]` to `[dependencies]` (no new external download).

---

## Files Changed

| File | Change |
|---|---|
| `engine/src/workspace.rs` | New — path helpers, `WorkspaceSettings`, `SafetyStrictness`, atomic load/save (18 tests) |
| `engine/src/safety/mod.rs` | New — module root + public re-exports |
| `engine/src/safety/action.rs` | New — `Actor`, `SafetyAction`, `SafetyActionKind` (18 variants) |
| `engine/src/safety/decision.rs` | New — `RiskLevel`, `SafetyVerdict`, `ConfirmationKind`, `SafetyDecision` |
| `engine/src/safety/protected_paths.rs` | New — `ProtectedPathRegistry` (29 defaults, two-tier glob) |
| `engine/src/safety/confirmation.rs` | New — `evaluate()` strictness-aware policy |
| `engine/src/safety/guards.rs` | New — `check_file_action`, `file_write_allowed`, `check_terminal_command`, `check_ai_context` |
| `engine/src/safety/file_guard.rs` | New — `preflight_file_mutation`, `PreflightOutcome` |
| `engine/src/audit.rs` | New — append-only JSONL audit log (7 tests) |
| `engine/src/recovery.rs` | New — snapshot / trash / backup / rollback (10 tests) |
| `engine/src/search_policy.rs` | New — local search exclusion policy |
| `engine/src/permissions.rs` | New — extension permission matrix, registry, approvals (8 tests) |
| `engine/src/logs.rs` | New — log path helpers, crash report storage (10 tests) |
| `engine/src/lib.rs` | Added `workspace`, `safety`, `audit`, `recovery`, `search_policy`, `permissions`, `logs` modules |
| `frontend/Cargo.toml` | Promote `serde_json` dev-dep → regular dep |
| `frontend/src/main.rs` | Added `workspace_load/save_settings`, `safety_evaluate`, `search_should_exclude`, `permissions_load_state`, `permissions_record_approval` commands |
| `frontend/ui/src/lib/tauri/commands.ts` | Added M14 typed wrappers for all new commands + DTOs |

---

## Test Gates

- `cargo test -p gwenland-engine` — **441 tests pass** (0 failures)
- `cargo check --workspace` — clean
- `pnpm check` — 0 errors, 0 warnings
- `pnpm test` — 72 tests pass
- `pnpm build` — succeeds
- `git diff -- Cargo.toml engine/Cargo.toml frontend/ui/package.json` — empty
