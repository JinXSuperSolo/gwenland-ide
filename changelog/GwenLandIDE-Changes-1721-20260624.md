# Wave 1 + Wave 2: UX/UI polish (welcome screen, settings slide-in, AI panel, terminal CWD, chat copy/edit) and full Git integration

- **Date:** 2026-06-24
- **Issue:** GWEN-321 ... GWEN-331 (Wave 1: UX/UI polish; Wave 2: Git integration)
- **Milestone:** Post-M10 product polish + Git integration

## Problem / Context

With the agentic workflow (M10) landed, the IDE worked but still felt unfinished in everyday use, and it had no Git story at all:

1. Opening the app with no folder dropped you straight into the full IDE chrome (empty file tree, a terminal with a stray CWD, an AI panel that demanded a project) instead of a calm starting point.
2. The right-click context menu used a heavy full-accent (orange) highlight and oversized rows, unlike the compact VS Code style the rest of the app follows.
3. Settings was a centered blocking modal; the user wanted a dismissible slide-in with search.
4. The AI panel had a blocking empty state, a clunky conversation dropdown, a redundant "AI" label, and no way to name conversations or copy/edit messages.
5. The terminal could spawn before a folder was open, landing in an arbitrary directory, and never followed the workspace when a folder was opened later.
6. There was no Git UI: no branch indicator, no staging/commit/sync, no diff viewer, no status colors in the tree, no branch switching.

The standing constraints held throughout: keep the engine/Tauri/UI split (pure provider-neutral logic in `engine/`, the command bridge in `frontend/src/main.rs`, Svelte state and UI in `frontend/ui/src/lib/`), dark theme only, **no new dependencies** (Rust or frontend), and a lean release binary (budget 6.5 MB). The frontend crate carries `serde_json` as a dev dependency only, so the Tauri layer must never name `serde_json::Value`.

## Change

### Wave 1 — UX/UI polish

**GWEN-321 Welcome screen.** A new full-screen `WelcomeScreen.svelte` renders when no folder is open **and** no tabs exist (`App.svelte` swaps the whole layout reactively, hiding the sidebar, terminal, and AI panel). It centers the GwenLand logo with a pure-CSS shimmer sweep, Open Folder / New File buttons, a Recent Projects list (from `recent_projects.json`), and the `Ctrl+Shift+P` command-palette tip. Opening a folder (or creating a New File) transitions to the full IDE. The logo asset was copied to `frontend/ui/public/logo-dark.png` (a webview resource, not linked into the Rust binary).

**GWEN-322 Compact context menu.** Rebuilt the context menu to VS Code-compact style: 24px rows, 13px non-bold labels, 4px container padding and radius with square per-item corners, a subtle dark surface lift on hover (not a solid accent block), muted right-aligned 11px shortcuts, and 1px separators with 4px vertical margin.

**GWEN-323 Settings slide-in.** Replaced the blocking modal with a right-edge slide-in panel (150-200ms ease) dismissed by Escape or click-outside, triggered by the gear and `Ctrl+,` (rebound from `Ctrl+Shift+,`). Added a "Search settings..." box that filters sections by title + keyword and sticky section headers on scroll, preserving all existing settings (theme preset, accent, font, AI, LSP).

**GWEN-324 AI panel polish.** Removed the blocking empty state so the composer is always live (placeholder "Ask anything..."); `sendMessage` now auto-creates a conversation on first send. Replaced the conversation dropdown with a clock/history icon that opens a slide-out list (`AiHistory.svelte`, with new/rename/delete). After the first response, the conversation auto-names itself via a new one-shot, **non-persisted** `ai_complete` command that sends a short "give a 3-5 word title" side-prompt. Dropped the redundant "AI" text from the panel header (sparks glyph only). Agent tools stay gracefully disabled with no workspace.

**GWEN-325 Terminal workspace CWD sync.** The terminal is hidden entirely (no restore strip) when no folder is open, and `Ctrl+J` is a no-op in that state. New sessions spawn with `CWD = workspace root`; opening a folder while a terminal is already running silently `cd`s every live, non-pinned session to the new root (`terminal/terminal-sync.ts`, dynamically imported so the workspace store stays decoupled). The Rust PTY spawn already accepted a `cwd`.

**GWEN-326 Chat copy + edit/rollback.** Each message bubble gets a hover copy button (user → plain text, assistant → raw markdown) with a "Copied!" tooltip. User messages additionally get an edit pencil that turns the bubble into an inline textarea with the note "Editing will rollback conversation to this point"; confirming drops every message from that point on, truncates the JSONL to match, and re-submits the edited text. This is backed by a new engine `conversation::truncate_turns` (atomic tmp + rename rewrite) exposed as `conversation_truncate`.

### Wave 2 — Git integration

All Git logic lives in a new **tauri-free** `engine/src/git.rs` that shells out to the system `git` binary via `std::process` only (no `git2`/`libgit2`, to protect the binary budget). It exposes status (branch + porcelain parse), stage/unstage/discard, commit, push/pull, branch list/checkout/create/delete, and per-file unified diff. Thin `git_*` `#[tauri::command]` wrappers in `frontend/src/main.rs` delegate to it; the frontend never shells out itself.

**GWEN-327 Status bar.** `GitStatusBar.svelte` shows `⎇ branch ●N` (branch via `rev-parse --abbrev-ref HEAD`, dirty count from `status --porcelain`), hidden entirely when the folder is not a repo. A `git` store (`stores/git.ts`) polls every 4 seconds and refreshes on demand after any action. Clicking the dirty count opens Source Control; clicking the branch opens the branch picker.

**GWEN-328 Git panel.** A new Source Control sidebar view, reached via a thin `ActivityBar.svelte` rail that switches the left panel between Explorer and Git (and is hidden when not a repo). `GitPanel.svelte` has CHANGES/Staged sections (per-file M/U/D/A badge plus Stage/Unstage/Discard and Stage-/Unstage-All), a COMMIT box (button disabled until a message and staged files exist), and SYNC Push/Pull. Every action refreshes the status.

**GWEN-329 File-tree status colors.** `TreeNode.svelte` colors filenames from the same poll data: modified amber `#e2c08d`, untracked/added green `#89d185`, deleted red `#f14c4c` with strikethrough; a parent folder goes amber when any descendant is dirty. Hidden when not a repo.

**GWEN-330 Inline diff viewer.** A new read-only `diff` tab kind (`stores/tabs.ts` `openDiff`, deduped by root+path, titled `filename.ext (diff)`). `GitDiffViewer.svelte` fetches `git diff HEAD` (or a synthesized `--no-index` diff for untracked files) and renders a unified view with green/red line backgrounds and old/new line numbers. Closing it touches no Git state.

**GWEN-331 Branch switcher.** Clicking the status-bar branch opens `BranchSwitcher.svelte`: a quick dropdown of local branches with the current one checked, a "＋ Create new branch..." entry (slugifies spaces to hyphens), and a non-blocking warning when switching with uncommitted changes. Added `Git: Checkout Branch`, `Git: Create Branch`, and `Git: Delete Branch` (excludes current) command-palette commands in `actions/gitActions.ts`.

## Why this approach

The empty-state gate keys on "no folder **and** no tabs" so a scratch New File from the welcome screen still opens the IDE, while a fresh launch stays calm. Auto-naming uses a dedicated one-shot `ai_complete` that drains a stream to a string and never records a turn, so the side-prompt cannot pollute conversation history; it falls back silently to the default title on any error. The edit/rollback maps store messages to JSONL turns by counting completed assistant messages before the edited one, then rewrites the file atomically, so a crash can never leave a half-written history. For Git, shelling to the user's own `git` keeps the binary lean and inherits their credentials and config for free; the engine module is pure and unit-tested (porcelain parsing, rename handling, path unquoting, slugify) while the UI just renders poll state and calls the wrappers. The 4-second poll (rather than a `.git` watcher) is simple, covers the "update within 5s of save" requirement, and avoids platform-specific FS-watch code.

## Impact

- A first-run user now sees a welcome screen, opens a folder, and gets a terminal already in the right directory; the AI panel is immediately usable and names its own conversations; messages can be copied and edited with a real rollback; and there is a complete Git workflow (status bar, Source Control panel, tree colors, inline diffs, branch switching) — all in the dark theme with the compact menu and slide-in settings.
- **New (engine):** `engine/src/git.rs` (+ unit tests), `conversation::truncate_turns` (+ tests). `engine/Cargo.toml` unchanged — no new Rust dependencies.
- **New (frontend):** `components/WelcomeScreen.svelte`, `AiHistory.svelte`, `GitStatusBar.svelte`, `BranchSwitcher.svelte`, `GitPanel.svelte`, `GitDiffViewer.svelte`, `ActivityBar.svelte`; `stores/git.ts`, `stores/sidebar.ts`; `terminal/terminal-sync.ts`; `actions/gitActions.ts`. **Changed (frontend):** `App.svelte` (welcome gate, activity bar + sidebar swap, terminal visibility), `SettingsPage.svelte` (slide-in + search + sticky headers), `AiPanel.svelte` / `AiMessage.svelte` / `ai/ai-chat-setup.ts` / `stores/ai-chat.ts` (always-live composer, history, auto-naming, copy/edit-rollback), `stores/workspace.ts` (auto-cd), `stores/tabs.ts` (diff tab kind), `TreeNode.svelte` (status colors), `Tabs.svelte`, `Workspace.svelte`, `StatusBar.svelte`, `actions.ts` (Ctrl+, rebind, terminal guard, git commands), context-menu components (compact). **Removed (frontend):** `AiConversation.svelte` (dropdown superseded by the history slide-out). **Changed (Tauri):** `frontend/src/main.rs` adds `ai_complete`, `conversation_truncate`, and twelve `git_*` commands. No new frontend dependencies.
- **Verification:** `cargo test -p gwenland-engine --lib` is **339 passing** (new git + truncate tests included); `cargo clippy -p gwenland-engine -p GwenLand-IDE` is clean; `svelte-check` is 0 errors / 0 warnings (248 files); `vitest run` is **20 passing**; `vite build` succeeds. The release `gwenland.exe` is **4.52 MB** (under the 6.5 MB budget) — Git added zero crates.
- **Things to keep in mind:**
  - **Git is hidden, not errored, when `git` is missing or the folder is not a repo** — the status store treats any failure as "not a repo" so the UI simply doesn't appear.
  - **The status poll is every 4s**, so freshly-changed files can take up to ~4s to recolor; any panel action refreshes immediately.
  - **Push/Pull surface raw `git` output** in a notice on failure (e.g. no upstream); they do not yet auto-set upstream or prompt for credentials beyond what `git` itself does.
  - **Conversation auto-naming needs a working provider key** — with no key it silently keeps "New Conversation".
  - **Verified by tests, type-check, clippy, and build**, plus a release binary size check; a full manual GUI pass of the Git flows on a live repo is still recommended.
