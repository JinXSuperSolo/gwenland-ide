# GwenLand IDE

A lean, local-first code editor built on Tauri 2 + Svelte, with an integrated AI coding assistant that runs entirely on your machine. No mandatory sign-in, no telemetry, no cloud sync.

**Version 0.1.14** · Windows / macOS / Linux · ships as a **~4.8 MB** native binary.

Built to stay fast on modest hardware (the reference target is an 11th-gen i3 with 8 GB RAM and no GPU): the Rust engine owns all heavy state and streams the UI compact diffs/patches — it never ships a whole file tree or a flood of events to the WebView.

## Features

### Core Editor
- File tree with inline rename / new-file / new-folder input (no modal dialogs)
- CodeMirror-based editor with tab management and unsaved-changes protection
- Command palette (Ctrl+Shift+P) with full keyboard navigation
- Workspace text search (Ctrl+Shift+F) — pure-Rust streamed results, cancellable, grouped by file
- Full menu bar (File, Edit, Selection, View, Go, Run, Terminal, Help)
- Open Recent with workspace-switch safety guard
- Per-workspace settings stored in `.gwenland/settings.json`

### Integrated Terminal
- Real interactive shell via ConPTY (Windows) / openpty (Unix)
- Multiple tabbed sessions with independent scrollback
- Engine-side ring-buffer scrollback + 50 000-line client cap
- Frame-limited output (rAF batching) so heavy builds never drop editor frames; pauses when the panel is hidden
- Reactive error detection per output chunk
- Dev-server ready detection — auto-surfaces Vite / Next / CRA URLs

### Web Preview
- Native WebView2 / WebKit preview pane (no embedded Chromium)
- Static file preview via Tauri asset protocol
- Dev-server preview via `http://localhost` auto-detection

### Git Integration
- Status indicators in the file tree and status bar
- Git panel: staged / unstaged diff, commit, branch switcher
- Diff tab with unified-diff viewer

### LSP Bridge
- Bring-your-own language server for Rust, TypeScript, JavaScript, Python
- Diagnostics + autocomplete via hand-rolled JSON-RPC (no `lsp-types` crate)
- Full document sync, zero new engine dependencies

### AI Assistant
- Multi-provider chat (bring your own API key, stored in the OS keychain)
- Per-workspace persona + system prompt (`.gwenland/GwenLand.md`)
- `@mention` context providers: `@file`, `@folder`, `@git`, `@diagnostics`, `@terminal`, `@web`
- Slash commands: `/clear`, `/new`, `/compact`, `/get-history`, `/model`, `/mode`, `/add-ctx-folder`, `/setup`
- Chat history with auto-naming and edit-rollback

### Agentic Coding (M10)
- Inline ReAct tool-calling agent in the same chat stream as plain chat
- Autonomy tiers: **Ask** / **Accept For Me** / **Full Control** — with a hard safety floor that always gates destructive, dependency-changing, and blocked actions
- Codex-style activity display: shimmer row + expandable timeline
- Inline diff approval (Accept / Reject) and command gates (Run / Skip)
- Path-preflight: model guesses are resolved to the real file before any gate is shown

### Self-Improving Memory (M13)
- Local Markdown memory at `.gwenland/agent/memory/`
- Keyword extraction + weighted multi-keyword grep retrieval
- Memory block injected into context before each response
- Automatic write-back after every response
- AI self-search: model emits a trigger phrase → IDE fetches from DuckDuckGo and resumes the stream

### Local-First Safety (M14)
- All safety decisions, audit records, and recovery artifacts live on disk under `.gwenland/`
- `SafetyStrictness`: Standard / Strict / Paranoid
- 29-entry protected-path registry (secrets, VCS, lockfiles, manifests) with two-tier glob matching
- Append-only JSONL audit log per category (Safety / Agent / Terminal / Git / Extension / Rollback)
- Snapshot / trash / backup / rollback with atomic writes and 10 MiB size cap
- Extension permission matrix with per-extension overrides
- Local crash reports — bounded, secrets-redacted, manual opt-in export only
- All features work fully offline — zero cloud dependencies in the core IDE

### Performance & Scalability (M19)
- **Virtualized file tree** — Rust owns the tree as a flat row model and streams `Insert`/`Remove`/`Update` patches; the UI renders only the visible window, so 10k-file workspaces scroll smoothly
- **Batched file watcher** — a from-scratch polling watcher coalesces bursts (e.g. `npm install`) into one patch per directory instead of one event per file, with no extra crate
- **Large File Mode** — files over 500 KB / 10k lines drop syntax, LSP, and minimap; files over 5 MB open read-only as plain text
- **Low-End Mode** — one toggle disables git badges, indent guides, minimaps, sticky scroll, animations, and file icons for older hardware
- **Status-bar activity badges** — Git scan / Indexing / Large File / Low-End / Searching / AI Running, with click-to-cancel where applicable
- **Optimistic file operations** — create / rename / delete / move update the tree instantly and roll back on failure, with Ctrl+Z undo

## Architecture

```
GwenLand IDE
├── engine/          Pure Rust business logic — zero Tauri imports
│   └── src/
│       ├── agentic/ Tool loop, memory, tier logic, prompts
│       ├── safety/  Action / decision / protected-paths / guards
│       ├── audit.rs, recovery.rs, permissions.rs, logs.rs
│       ├── terminal.rs, ring_buffer.rs, error_detect.rs
│       ├── fs.rs, fs_watch.rs   File ops + polling watcher (M19)
│       ├── tree.rs              Flat-row tree model + patches (M19)
│       ├── search.rs            Streamed workspace text search (M19)
│       ├── devserver_detect.rs
│       ├── workspace.rs
│       └── git.rs, lsp/, ...
├── frontend/        Tauri 2 application shell (Rust)
│   └── src/main.rs  Thin command-and-event bridge; no engine logic
└── frontend/ui/     Svelte 5 + TypeScript UI
    └── src/lib/
        ├── stores/  All app state (tabs, ai-chat, agentic, terminal, ...)
        ├── ai/      Persona, mentions, slash commands, search
        ├── agentic/ Tool-loop driver, activity model
        ├── actions/ File, git, search ops
        └── tauri/   Typed command wrappers
```

The engine crate carries **no Tauri dependency** — all logic is unit-testable without a GUI runtime. The Tauri layer is a thin bridge of `#[tauri::command]` functions and events. The Svelte UI holds no secrets and imports nothing from the engine directly.

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (stable, latest)
- [Node.js](https://nodejs.org/) ≥ 18 and [pnpm](https://pnpm.io/)
- [Tauri CLI](https://tauri.app/start/prerequisites/): `cargo install tauri-cli`
- Windows: WebView2 runtime (ships with Windows 11; installer available for Windows 10)

### Development

```sh
# Install frontend dependencies
cd frontend/ui && pnpm install

# Run in dev mode (hot-reload Svelte + Tauri window)
cargo tauri dev

# Run engine tests only (fast, no GUI)
cargo test -p gwenland-engine
```

### Production Build

```sh
cargo tauri build
# Produces MSI and NSIS installers under target/release/bundle/
```

### Type-check & lint

```sh
cd frontend/ui
pnpm check    # svelte-check
pnpm test     # vitest
```

## Project Layout

```
.gwenland/              Per-workspace data (git-ignored by convention)
├── settings.json       Workspace settings (theme, fonts, layout, …)
├── GwenLand.md         AI persona + system prompt
├── agent/memory/       Local AI memory (Markdown notes)
├── safety/             Protected-path overrides
├── audit/              Append-only JSONL audit logs
├── snapshots/          Pre-mutation file snapshots
├── trash/              Soft-deleted files + index
├── backups/            Git-patch backups
├── extensions/         Permission registry + approval log
└── logs/               App logs + crash reports
```

## Test Coverage

| Suite | Count |
|---|---|
| `cargo test -p gwenland-engine` | 487 tests |
| `pnpm test` (vitest) | 114 tests |

All gates are green on the current branch.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for the per-release summary, and [`changelog/`](changelog/) for detailed dated session notes.

## Engineering Constraints

These rules hold across the whole codebase and are enforced in review:

- **Zero-dependency growth** — no new Rust crates or npm packages are added; new capabilities (file watcher, tree diffing, workspace search, diagram/markdown rendering) are implemented from scratch on `std` and existing deps.
- **Binary budget** — the release executable stays small (currently ~4.8 MB; budget ≤ 7 MB including the LSP server).
- **Rust owns state, the UI renders diffs** — the WebView never receives a full tree or an unbounded event stream, only bounded patches.
- **The engine crate has no Tauri dependency** — all logic is unit-testable headlessly.

## License

[LICENSE](LICENSE)
