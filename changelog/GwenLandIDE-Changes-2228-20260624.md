# AI composer: slash commands, @-mention context providers, and per-workspace persona / system prompt

- **Date:** 2026-06-24
- **Issue:** GWEN-332 (@ mentions), GWEN-333 (slash commands), GWEN-334 (persona & system prompt)
- **Milestone:** Post-M10 AI composer power features

## Problem / Context

The unified AI composer worked but was a single plain textarea: no quick commands, no way to pull workspace context into a prompt, and one hardcoded engine system prompt for every project. This wave adds three composer features, all sharing the same store/setup/component split (Tauri-free registry + pure helpers in `stores/`, side effects in `ai/`, UI in `components/`) and the existing constraints: engine/Tauri/UI separation, dark theme, **no new dependencies** (Rust or frontend).

## Change

### GWEN-333 — Slash commands

Typing `/` opens a filterable dropdown above the composer (Arrow/Enter/Tab/Esc nav); a fully-typed command also runs on send instead of messaging.

- `stores/slash-commands.ts` — Tauri-free registry + pure helpers (`parseSlashQuery`, `filterCommands`, `exactCommand`, `parseHistoryCount`).
- `ai/slash-command-setup.ts` — `runSlashCommand(id, rest)` dispatcher returning a `SlashResult`; best-effort (errors go to `aiChat.lastError`, never throw).
- `components/SlashCommandMenu.svelte` — controlled dropdown (parent owns the filtered list + active index).
- Commands: `/clear`, `/new`, `/compact` (summarize history via `ai_complete`, replace with one block), `/get-history [n=5]` (load last n from JSONL, inject as a display-only note), `/model` + `/mode` (inline pickers built into `AiPanel`; `/mode` switches the **AgentTier**), `/add-ctx-folder` (folder dialog → `CONTEXT.md`), `/setup` (workspace scan → `.gwenland/GwenLand.md`).

### GWEN-332 — @-mention context providers

Typing `@` opens a dropdown (specials first, then fuzzy file/folder results). Selected mentions become removable pills; on send they resolve into a `<context>…</context>` block prepended to the message.

- `stores/mention-providers.ts` — Tauri-free: `MentionItem`/`MentionCandidate`, `SPECIAL_PROVIDERS`, `parseMentionQuery` (caret-aware), `parseLineRange` (`file:10-50`, ignores `C:\` drive colons), `fuzzyScore`/`fuzzySearch` (exact > prefix > boundary > substring > subsequence, no fuse.js), `stripHtml` (from scratch — drops script/style, block→newline, decodes named + numeric entities), icon mapping.
- `ai/mention-setup.ts` — `resolveAllMentions`/`buildContextBlock`, `getWorkspaceIndex` (recursive `listDirectory` walk, cached per root, ignores `node_modules`/`.git`/etc., 5000-entry cap), per-type resolvers. Best-effort: a failed mention injects `[unavailable: …]`.
- `components/MentionMenu.svelte` (dropdown) + `MentionPill.svelte` (pill; file click → `openFile` + `selectRange`).
- Six providers: `@file` (≤500 lines), `@file:start-end` (range), `@folder/` (tree + file contents, ≤200 lines/file, ≤40 files), `@git` (`gitStatus` + per-file `gitDiffFile`), `@diagnostics` (LSP store), `@terminal` (last 50 PTY lines), `@web <url>` (frontend `fetch()` + `stripHtml`, ≤20k chars).
- Editor/terminal helpers added: `selectRange(start, end)` in `editor/active-editor.ts`; `readBuffer(maxLines)` on the terminal handle (`terminal-registry.ts` + `TerminalInstance.svelte`, reads xterm `buffer.active`).

### GWEN-334 — Per-workspace persona & system prompt

Persona + system prompt live in `.gwenland/GwenLand.md` (the file `/setup` writes). The persona name shows in the AI panel header and updates reactively.

- `stores/workspace-persona.ts` — `PersonaConfig`/`Persona`/`Tone`, `TONE_PRESETS` (professional/casual/teacher/silent, each with a directive), `parseGwenLandMd`/`serializeGwenLandMd` (pure, tolerant of `[placeholder]` lines and missing sections, preserves non-managed sections on save), the `persona` store, and `loadPersona`/`savePersona`/`resetPersona`.
- `ai/persona-setup.ts` — `buildSystemPrefix(config)` composes system prompt + persona voice + custom instructions; `activeSystemPrefix()` reads the store (`''` = engine default only).
- `components/PersonaPicker.svelte` (`/persona`: name + tone) + `SystemPromptEditor.svelte` (`/system`: textarea + Save/Cancel); `/reset-system` clears the custom prompt. Inline, float above the composer like the other pickers.
- **Engine:** `ai_send` gained an optional `system_prefix` param. New `compose_system_prompt(prefix)` **layers** the persona on top of the base prompt (`{prefix}\n\n---\n\n{GWENLAND_SYSTEM_PROMPT}`) so the always-on `<think>` + unified-diff protocol the UI parses is never lost; a blank prefix falls back to the base unchanged.
- `/setup` template rewritten to the canonical `GwenLand.md` format (Workspace / AI Persona / System Prompt / Custom Instructions / Workspace Context).

## Why this approach

All three features reuse one composer pattern, so the dropdown anchoring, keyboard nav, and inline pickers are consistent and the pure logic is unit-testable without Tauri. `@web` and `stripHtml` are hand-rolled to honor the no-new-deps rule (the WebView already has `fetch`). The persona is **layered over** the engine prompt rather than replacing it because the base prompt encodes the `<think>`/diff contract the thinking-parser and diff-review depend on — replacing it would break parsing. The workspace file index is built lazily on first `@` and cached per root to keep typing responsive. Persona scope is intentionally the chat path (`ai_send`); the agentic ReAct loop keeps its own protocol-heavy prompts.

## Impact

- **New (frontend):** `stores/slash-commands.ts`, `stores/mention-providers.ts`, `stores/workspace-persona.ts`; `ai/slash-command-setup.ts`, `ai/mention-setup.ts`, `ai/persona-setup.ts`; `components/SlashCommandMenu.svelte`, `MentionMenu.svelte`, `MentionPill.svelte`, `PersonaPicker.svelte`, `SystemPromptEditor.svelte`; tests `slash-commands.test.ts`, `mention-providers.test.ts`, `workspace-persona.test.ts`.
- **Changed (frontend):** `AiPanel.svelte` (slash/@/persona wiring, inline pickers, pills, header name), `ai/ai-chat-setup.ts` (pass `systemPrefix`), `tauri/commands.ts` (`AiSendArgs.systemPrefix`), `editor/active-editor.ts` (`selectRange`), `terminal/terminal-registry.ts` + `TerminalInstance.svelte` (`readBuffer`).
- **Changed (Tauri):** `frontend/src/main.rs` — `ai_send` `system_prefix` param + `compose_system_prompt` helper (+ unit test). No new Rust or frontend dependencies.
- **Verification:** `svelte-check` 0 errors / 0 warnings (276 files); `vitest run` **72 passing** (slash + mention + persona suites); `vite build` succeeds; `cargo test compose_system_prompt` passes; `cargo check` on the Tauri crate is clean.
- **Things to keep in mind:**
  - Icons use **Iconoir** (the project's set), not Phosphor — there are no per-language file glyphs, so code files share one icon.
  - The task referenced commands `get_git_diff`/`get_diagnostics`/`list_dir`/`open_file` that don't exist; the real surfaces (`gitStatus`+`gitDiffFile`, the `lsp` store, `listDirectory`, `openFile`) are used instead.
  - The workspace file index is cached; create/delete during a session needs `invalidateWorkspaceIndex()` (exported, not yet auto-wired to file mutations).
  - Persona applies to chat (`ai_send`), not the agentic loop, and takes effect on the next message (no restart).
  - `/persona` and `/system` save via inline editors with explicit Save/Cancel; they are not closed by an outside click (to avoid discarding edits).
