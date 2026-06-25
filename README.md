# GwenLand IDE

A lean, local-first code editor built on Tauri 2 + Svelte, with an integrated AI coding assistant that runs entirely on your machine. No mandatory sign-in, no telemetry, no cloud sync.

## Features

### Core Editor
- File tree with inline rename / new-file / new-folder input (no modal dialogs)
- CodeMirror-based editor with tab management and unsaved-changes protection
- Command palette (Ctrl+Shift+P) with full keyboard navigation
- Full menu bar (File, Edit, Selection, View, Go, Run, Terminal, Help)
- Open Recent with workspace-switch safety guard
- Per-workspace settings stored in `.gwenland/settings.json`

### Integrated Terminal
- Real interactive shell via ConPTY (Windows) / openpty (Unix)
- Multiple tabbed sessions with independent scrollback
- 10 000-line ring-buffer scrollback cap
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

## Architecture

```
GwenLand IDE
├── engine/          Pure Rust business logic — zero Tauri imports
│   └── src/
│       ├── agentic/ Tool loop, memory, tier logic, prompts
│       ├── safety/  Action / decision / protected-paths / guards
│       ├── audit.rs, recovery.rs, permissions.rs, logs.rs
│       ├── terminal.rs, ring_buffer.rs, error_detect.rs
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
| `cargo test -p gwenland-engine` | 441 tests |
| `pnpm test` (vitest) | 72 tests |

All gates are green on the current branch.

## License

[LICENSE](LICENSE)
