# GwenLand IDE — Session Changes
**Date:** 2026-06-26 · **Session end:** ~20:30

---

## What we built this session

Four separate feature pushes landed, all without touching tests (82/82 frontend, 455/455 engine still green) and without adding a single new dependency.

---

### 1 — AI agent no longer freezes the UI during long commands

This was a real pain: kicking off `npm install` or any slow shell command from the agent would lock up the entire window for however long the process ran. The root cause was `run_terminal_tool` sitting there blocking until the child process finished. We fixed it properly.

**What changed:**
- `run_terminal_tool` in `frontend/src/main.rs` is now a true `async fn`. It spawns the child with piped stdout/stderr, registers the child's PID in a new `cmd_pids: Arc<Mutex<HashMap<String, u32>>>` field on `AgentManager`, then hands off each line of output to two `spawn_blocking` threads — one for stdout, one for stderr. Each line arrives as an `agent://cmd_output` event. When the process exits, an `agent://cmd_done` event fires.
- New `agent_kill_terminal` Tauri command: looks up the PID in that map and runs `taskkill /F /T /PID` on Windows (kills the whole process tree, so npm's sub-processes die too), or `kill -TERM` on Unix.
- The agentic store (`stores/agentic.ts`) gained `isRunningCommand` and `cmdOutputLines` state, with `setRunningCommand()` and `appendCmdOutputLine()` helpers.
- `CommandGate.svelte` (the UI card that appears when the agent wants to run a terminal command) now shows a pulsing green dot + "Agent is executing command…" while running, a live scrollable output box (capped at 500 lines), and a red kill button instead of the normal Run/Skip buttons.
- `agentic-setup.ts` wires up the event listeners when a tool gate resolves, and tears them down on session end.

**New events/commands:** `agent://cmd_output`, `agent://cmd_done`, `agent_kill_terminal`

---

### 2 — Git decorations everywhere

The IDE finally looks like it knows you're in a git repo.

**File tree badges** — `TreeNode.svelte` now reads the `git` store and shows a small letter badge flushed to the right of every filename: `M` for modified/renamed, `A` for added (tracked untracked), `U` for untracked, `D` for deleted. Directories go amber when any child inside them is dirty. The badge is styled as a tiny pill — amber/orange for modified, green for added, red for deleted — and disappears completely when the file is clean.

**Tab dirty dots** — `Tabs.svelte` adds a 6px square dot after each tab's close button when the file has uncommitted changes. Amber for modified, green for added, red for deleted. It hides itself when the tab already shows the unsaved `●` dot to avoid double-signalling.

**Branch ahead/behind counter** — `GitStatusBar.svelte` now shows `↑N` (green) and `↓N` (amber) next to the branch name when you're ahead or behind your upstream. The numbers come from `git rev-list --count --left-right HEAD...@{u}` — a pure-parse operation we added to `engine/src/git.rs` as `ahead_behind()`. It returns `(0, 0)` gracefully if the branch has no upstream, git isn't available, or the repo is in detached HEAD state.

**Engine:** `GitStatus` struct gained `ahead: usize` and `behind: usize` fields. A new unit test (`ahead_behind_parses_rev_list_output`) covers the parser — no git process needed.

The git store also now refreshes after the agent finishes a terminal command (via `agent://cmd_done`), so git badges update automatically after an `npm install` or similar.

---

### 3 — Context menus and dropdowns redesigned with GwenLand tokens

Everything that pops up now looks like it belongs in the IDE rather than a browser default.

**`ContextMenuPortal.svelte`** — replaced the old generic gray tokens with the full GwenLand palette: `#1f1e1e` background, `rgba(225,132,69,0.10)` hover, `rgba(225,132,69,0.18)` active, orange-tinted border, `0.5rem` radius, `scaleY(0.95 → 1)` pop animation in 100ms.

**Danger items** — `ContextAction` interface gained an optional `danger?: boolean` flag. "Move to Trash" and "Delete Permanently" are now marked as danger actions. `ContextMenuItem.svelte` picks that up and renders them with a red label (`#e06c75`) and red-tinted hover/active backgrounds. No other items are affected.

**`CustomDropdown.svelte`** (new component) — a fully native-free styled dropdown to replace every `<select>` in the app. Supports optional `svgIcon` per item, compact mode, placement hint, full keyboard navigation (Arrow Up/Down, Enter, Escape), outside-click dismiss, and `role="listbox"/"option"` for accessibility. Uses the same GwenLand design tokens as the context menu.

**Shell picker in the terminal panel** — `TerminalPanel.svelte` replaced its `<select>` with `CustomDropdown` in compact mode. Each shell option has a matching inline SVG icon: PowerShell, CMD, WSL, Bash, Zsh, Node, Python, and a generic terminal fallback. All SVGs are 14×14, self-contained, no external refs.

**Font picker in Settings** — `SettingsPage.svelte` replaced its `<select>` with `CustomDropdown`. The font name is both the label and the value, so nothing special needed there.

---

### 4 — File type icons in the file tree

`FileIcon.svelte` and `frontend/ui/src/lib/icons/gwenland-icons.ts` (new file) hook into the file tree so every node gets a proper icon instead of a plain page glyph.

Icons come from `material-icon-theme` (already a project dependency — zero new packages). The map covers 55+ extensions and special filenames:

- **Special filenames:** `package.json` → Node, `pnpm-lock.yaml` → pnpm, `Cargo.toml` → Rust, `tauri.conf.json` → Tauri, `Dockerfile` → Docker, `.gitignore` / `.gitattributes` → Git, `.env*` → lock, `tsconfig.json` → TypeScript, `vite.config.*` → Vite, `vitest.config.*` → Vitest, `eslint.config.*` → ESLint, `.prettierrc` → Prettier, `README.md` → Readme, `LICENSE` → License
- **Extensions:** ts/mts/cts → TypeScript, tsx → React TS, dts → TypeScript Def, js/mjs/cjs → JavaScript, jsx → React, rs → Rust, svelte, vue, json/jsonc, toml, md/mdx/markdown, css, scss/sass, less, html/htm, py, go, java, c/h, cpp/cc/cxx/hpp, cs, php, rb, lua, dart, yml/yaml, xml, graphql/gql, prisma, sql, sh/bash/zsh/fish, ps1, png/jpg/jpeg/gif/svg/webp/ico/bmp, pdf, doc/docx, ppt/pptx, mp3/wav, mp4/mov, woff/woff2/ttf/otf, zip/gz/tar, lock, txt

Added `mdx` specifically this session — it was the only extension from the requirements list that was missing from the initial implementation.

Also exported `FILE_ICONS` (merged map) and `getFileIcon` (alias for `fileIconSvg`) as canonical names for any future callers.

Folder icons show the "open folder" variant when the node is expanded and "closed folder" when collapsed.

---

## Files changed

| Area | Files |
|------|-------|
| Engine (Rust) | `engine/src/git.rs` (+`ahead_behind`, `ahead`/`behind` fields, new test) |
| Tauri backend | `frontend/src/main.rs` (async `run_terminal_tool`, `AgentManager.cmd_pids`, `agent_kill_terminal`) |
| New components | `CustomDropdown.svelte`, `EditorBreadcrumbs.svelte`, `MarkdownPreview.svelte` |
| New stores/libs | `frontend/ui/src/lib/icons/gwenland-icons.ts`, `stores/editor-preferences.ts`, `preview/markdown.ts` |
| Modified components | `CommandGate.svelte`, `TreeNode.svelte`, `Tabs.svelte`, `GitStatusBar.svelte`, `TerminalPanel.svelte`, `SettingsPage.svelte`, `FileIcon.svelte`, `ContextMenuPortal.svelte`, `ContextMenuItem.svelte`, `ContextMenuSeparator.svelte` |
| Stores | `stores/agentic.ts`, `stores/git.ts`, `stores/tabs.ts` |
| Tauri bridge | `tauri/commands.ts` (new event types, `onAgentCmdOutput`, `onAgentCmdDone`, `agentKillTerminal`, `GitStatus.ahead/behind`) |
| Context menu | `contextTypes.ts` (danger field), `fileActions.ts` (danger on delete actions) |

## Test counts
- Frontend: **82 / 82** passed (9 suites)
- Engine: **455 / 455** passed
