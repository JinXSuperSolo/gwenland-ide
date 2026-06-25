Cancel Rollback, Inline Tree Input, and M13 Self-Improving Memory Agent

- **Date:** 2026-06-25
- **Issue:** Bug fixes + UX (cancel rollback, inline file-tree input) + M13 (GWEN-335 → 339)
- **Milestone:** M13 — Self-Improving Memory Agent + post-M10 UX fixes

---

## 1. Cancel Rollback (Chat + Agent modes)

**Problem:** Pressing Stop/Cancel while a stream was in flight left a blank assistant message and its preceding user message permanently in the conversation list. In Agent mode this showed "Cancelled." underneath a duplicate user turn on the next send.

**Fixes — `frontend/ui/src/lib/stores/ai-chat.ts`**

- `setStreamError`: when `error.kind === 'cancelled'` and the streaming assistant message has no content yet, the blank assistant turn and its preceding user turn are removed atomically. If the stream did produce partial content the partial message is kept (just stopped).
- `rollbackEmptyAgentTurn(runId)` *(new export)*: finds the live (un-snapshotted) agent turn for a run id and removes it together with the preceding user turn — but only when the agent produced zero tool steps and no final answer.

**Fixes — `frontend/ui/src/lib/agentic/agentic-setup.ts`**

- `cancelAgentSession`: after tearing down listeners and ending the tool loop, checks whether the current run has any `toolLog` entries or a `toolFinal`. If not, calls `rollbackEmptyAgentTurn` to clean up the UI. Runs that produced partial progress still transition to the `'cancelled'` phase and remain visible.

---

## 2. Inline File-Tree Input (replaces modal)

**Problem:** New File, New Folder, and Rename triggered a modal (`PromptDialog`) that broke immersion. VS Code uses an inline input row directly in the tree.

**New store — `frontend/ui/src/lib/stores/tree-input.ts`**

Promise-based store (same pattern as `promptDialog`) with `openTreeInput` / `confirmTreeInput` / `cancelTreeInput`. Carries `kind` (`file | folder | rename`), `targetDir`, `initialValue`, and `icon`.

**Updated — `frontend/ui/src/lib/components/FileTree.svelte`**

- Header now shows four icon buttons when a folder is open: **New File** (`page-plus`), **New Folder** (`folder-plus`), **Collapse All** (`collapse`), **Refresh** (`refresh`). Open Folder button only shown when no folder is open.
- Inline input row renders at the top of the tree body when `treeInput.open` is true: 24 px height, file/folder icon, blue-bordered input, auto-focused. Enter → confirm, Escape/blur → cancel.
- For Rename: selection range pre-selects the name without the extension.
- Header buttons for New File/Folder target the workspace root; context-menu actions still target the right-clicked folder (handled in `fileActions.ts`).

**Updated — `frontend/ui/src/lib/actions/fileActions.ts`**

- All three `openPrompt` calls replaced with `openTreeInput`:
  - `file.newFile` → `openTreeInput({ kind: 'file', ... })`
  - `file.newFolder` → `openTreeInput({ kind: 'folder', ... })`
  - `file.rename` → `openTreeInput({ kind: 'rename', initialValue: current, ... })`
- Import changed from `prompt-dialog` → `tree-input`. `PromptDialog` component is untouched.

**Updated — `frontend/ui/src/lib/components/Icon.svelte`**

- Added `collapse` icon from `iconoir/icons/regular/collapse.svg` for the Collapse All header button.

---

## 3. M13 — Self-Improving Memory Agent (GWEN-335 → 339)

All five waves completed. Zero new Rust or npm dependencies.

### Wave 1 — Engine memory module (GWEN-335)
**New file: `engine/src/agentic/memory.rs`** (34 tests)

Pure-Rust local memory layer. Key pieces:
- `sanitize_segment` / `sanitize_note_filename` — safe kebab-case, blocks path traversal
- `project_name_from_root` / `memory_project_dir` / `memory_conversation_dir` — canonical paths under `.gwenland/agent/memory/<project>/<conversation>/`
- `search_memory` — walks up to 500 `.md` files, multi-keyword grep, weighted scoring (filename hit = 10, heading hit = 5, line hit = 2, multi-keyword coverage bonus), returns top results sorted by score
- `render_memory_block` — wraps results in `<memory>…</memory>` with a 3 000-char / 6-lines-per-file budget
- `write_memory_note` — creates dirs, writes new or appends with `---` separator, caps at 20 lines per note
- `parse_keyword_array` / `parse_memory_note` — tolerant JSON parsers, strip code fences

**`engine/src/agentic/mod.rs`** — added `pub mod memory;` and re-exports.

### Wave 2 — Keyword extractor (GWEN-336)
**`frontend/src/main.rs`**

- `complete_once` — bounded, non-persisted, non-streaming mini LLM call (used by W2/W4/W5)
- `extract_keywords` — mini-call with max 80 tokens, returns ≤7 deduplicated keywords
- `ai_complete` refactored to route through `complete_once`

### Wave 3 — Memory context injection (GWEN-337)
**`frontend/src/main.rs`**

- `ai_send` made `async` (Tauri v2 supports async commands)
- `retrieve_memory_block` — orchestrates keyword extraction + grep + render
- Memory block prepended to the provider-only user message; the JSONL-persisted `expanded` form is unchanged
- Agent tool loop injects memory into context only on first iteration (`iteration == 0`)

### Wave 4 — Memory write-back (GWEN-338)
**`frontend/src/main.rs`**

- `run_memory_writeback` — post-response mini-call, max 150 tokens, parses `{filename, content}` JSON, calls `write_memory_note`
- Called from `run_stream` after `ai://done`; silent on failure, never runs on cancelled/errored streams
- Conversation name sourced from `meta.title` (falls back to `conversation_id`)

### Wave 5 — AI self-search (GWEN-339)
**`frontend/src/main.rs`**

- `detect_search_trigger` — looks for `Let me search <topic> for more detail...` in streamed text
- `AiManager.search_resolvers` — `HashMap<String, oneshot::Sender<String>>` for stream parking
- `run_stream` detects trigger, parks a oneshot sender, emits `ai://search_request`, awaits result (30 s timeout), builds a second provider request with a `<search query="…">…</search>` continuation block, resumes on the same `stream_id`
- `ai_search_result` Tauri command — frontend feeds the web result back to unblock the parked stream
- `GWENLAND_SYSTEM_PROMPT` extended with the search trigger instruction

**New file: `frontend/ui/src/lib/ai/search-setup.ts`**

- `fetchSearchText(query)` — browser `fetch` to DuckDuckGo HTML endpoint, `stripHtml`, 2 000-char cap
- `initSearchListener()` — persistent `ai://search_request` Tauri event listener; calls `aiSearchResult` to resume. Returns unlisten fn.
- Registered in `initAiChat()` as an app-level listener

**`frontend/ui/src/lib/tauri/commands.ts`** — added `aiSearchResult(streamId, resultText)` typed wrapper.

---

## Files Changed

| File | Change |
|------|--------|
| `engine/src/agentic/memory.rs` | New — 34 tests, complete M13 engine module |
| `engine/src/agentic/mod.rs` | Added `pub mod memory;` + re-exports |
| `frontend/src/main.rs` | async `ai_send`, `complete_once`, `extract_keywords`, `retrieve_memory_block`, `run_memory_writeback`, `detect_search_trigger`, `ai_search_result` command, `search_resolvers` in `AiManager`, extended `run_stream` |
| `frontend/ui/src/lib/stores/ai-chat.ts` | `setStreamError` cancel rollback, `rollbackEmptyAgentTurn` |
| `frontend/ui/src/lib/agentic/agentic-setup.ts` | `cancelAgentSession` rollback logic |
| `frontend/ui/src/lib/stores/tree-input.ts` | New — inline tree input store |
| `frontend/ui/src/lib/components/FileTree.svelte` | Header buttons + inline input row |
| `frontend/ui/src/lib/components/Icon.svelte` | Added `collapse` icon |
| `frontend/ui/src/lib/actions/fileActions.ts` | `openPrompt` → `openTreeInput` |
| `frontend/ui/src/lib/ai/search-setup.ts` | New — web search helper + listener |
| `frontend/ui/src/lib/tauri/commands.ts` | Added `aiSearchResult` |
| `changelog/SUMMARY.md` | New — full changelog summary (all 18 prior entries) |

---

## Test Gates

- `cargo test -p gwenland-engine agentic` — 106 tests pass
- `cargo check --workspace` — clean
- `pnpm check` — 0 errors, 0 warnings
- `pnpm test` — 72 tests pass
- `pnpm build` — succeeds
- `git diff -- Cargo.toml engine/Cargo.toml frontend/ui/package.json` — empty (zero new dependencies)
