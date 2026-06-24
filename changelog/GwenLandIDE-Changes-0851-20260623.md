# M8 follow-ups: rich Markdown (tables, from-scratch math + graphs), reliable diff review, a Codex-style proposal card, always-on thinking, pane resize limits, and image upload

- **Date:** 2026-06-23
- **Issue:** GWEN-314 … GWEN-318 (Milestone 8 follow-ups, from live testing)
- **Milestone:** Milestone 8 — AI Chat Pane Polishing (post-revision fixes + multimodal)

## Problem / Context

After the M8 revision (`GwenLandIDE-Changes-2146-20260622.md`), hands-on use surfaced a string of issues and gaps in the AI pane. This entry covers everything done since:

1. A real diff that should have been reviewable showed *"This looked like a diff but couldn't be parsed for review."*
2. Assistant replies rendered raw Markdown (literal `###`, `**bold**`, `| tables |`), and had no math (LaTeX), tables, or diagrams.
3. The "Proposed changes" notice was a thin one-line pill; the user wanted the richer Codex-style file-list card.
4. Thinking blocks only appeared for models that natively emit reasoning.
5. Panes (file tree / workspace / terminal / AI) had no resize limits and their dividers were invisible after the recolor.
6. No way to upload images to multimodal models — the composer was text-only.

The constraints from M8 still held throughout: keep the engine/Tauri/UI split, dark-theme only, **no new dependencies** (the user explicitly asked for math/graphs "from scratch"), don't store secrets in the frontend, and keep the release binary lean.

## Change

### 1. Diff review no longer blocked by example/template diffs

Assistants often include a *format example* — a fenced block with a literal `@@ -line,count +line,count @@` header — before the real diff. The parser treated that malformed header as fatal and aborted the **entire** parse, so the real change below was never detected. The engine parser is now **lenient**: a malformed/template hunk header is treated as prose and skipped, so a valid diff elsewhere still parses (`engine/src/ai/diff.rs`). Detection (`diff-detection.ts`) also drops files with zero hunks, so a placeholder file never becomes a phantom proposal. Added tests covering the exact "template header then real diff" case.

### 2. AI responses render Markdown

A new dependency-free renderer (`frontend/ui/src/lib/ai/markdown.ts`) turns assistant prose into HTML — headings, bold/italic, inline code, bullet/ordered lists, blockquotes, links (http(s)/mailto only), and **GFM tables** with column alignment. Fenced code blocks still go through the existing highlighter. Security: the source is HTML-escaped first, then formatting is applied to the escaped text (the same trusted-output approach as the code highlighter), so `{@html}` is safe.

### 3. LaTeX math — from scratch

`frontend/ui/src/lib/ai/math.ts` is a small LaTeX→HTML renderer (no KaTeX) wired into the Markdown for `$…$` / `\(…\)` inline and `$$…$$` / `\[…\]` display. It covers Greek + ~120 symbols/operators, super/subscripts, `\frac`, `\sqrt` (with index), `\text`/`\mathrm`/`\mathbf`/…, function names, and spacing — laid out with HTML/CSS (italic variables, real fraction bars, radical overlines). Single-`$` is only treated as math when it contains math characters, so prices like "$5 … $10" aren't swallowed; unknown commands degrade to their name (never throws).

### 4. Diagrams/graphs — from scratch

`frontend/ui/src/lib/components/MermaidGraph.svelte` parses simple Mermaid flowcharts (`graph`/`flowchart TD|LR`, node shapes `[] () {} (()) ([])`, `-->`/`---`/`==>` edges with `|labels|`), does a longest-path **layered layout**, and draws **SVG** (boxes/diamonds/stadiums, arrowed edges, labels). ` ```mermaid `/` ```graph ` code blocks now render as diagrams. No Mermaid dependency; anything it can't parse falls back to a clean source box.

### 5. Codex-style proposal card

The "Proposed changes" notice in `AiMessage.svelte` became a card: a header (icon + title + total `+added`/`−removed` + a **Review** button), an expandable **per-file list** with each file's path (directory ellipsized so the filename stays visible) and per-file `+/−` counts, and a Collapse toggle. (No "Undo" — unlike Codex, GwenLand's card is a *proposal*; nothing is written until you Review → accept.)

### 6. Always-on thinking via the system prompt

The GwenLand system prompt (`frontend/src/main.rs`) now instructs the model to **always** reason inside `<think>…</think>` and write the final answer (and any diff) *after* the closing tag. This makes the collapsible thinking block appear for every provider — not just natively-reasoning models — while keeping the diff in the answer where detection runs.

### 7. Pane resize limits + visible dividers

Per-panel `[min, max]` bounds in `stores/panels.ts` (File Tree 160–560, AI pane 300–680, Terminal 100–720 px), and `ResizeHandle.svelte` also caps to 60% width / 70% height of the window — so no pane can be dragged uselessly small or large enough to swallow the workspace. The resize strip (transparent at rest, invisible against the recolored palette) now renders a persistent 1px divider line that highlights to the accent on hover/drag.

### 8. Image upload — multimodal across all vision providers

The composer can now attach images (file picker via the attach menu, **paste** a screenshot, or **drag-and-drop**), with thumbnail previews + remove and an 8 MB/image cap; image-only turns are allowed. Engine support: a new `ImageAttachment { mime, data }` DTO and `MessageRequest.images`, with each adapter rendering the image on the current user turn in its own format — Anthropic `image` blocks, OpenAI/generic `image_url` data-URLs, Gemini `inline_data` parts. Images attach to the message being sent only — **not persisted to JSONL, not re-sent from history** — which keeps the change safe and contained. Sent images render as thumbnails in the user message.

### Also

- Fixed the engine `recent_projects`/`conversation` tests' parallel flakiness pattern earlier; `conversation` tests now pass in parallel. (`recent_projects` still shares a global dir — green single-threaded; noted for a later isolation fix.)

## Why this approach

The recurring theme was **dependency-free by request**. Math and diagrams would normally pull KaTeX (~0.3 MB) and Mermaid (~1 MB), which — because Tauri embeds the frontend bundle into the binary — would eat into the 7 MB budget M8 protected. Building lightweight, subset renderers from scratch added only **~14 KB** total while covering what assistant replies actually use, and the call sites are clean enough to swap in a real library later if a complex case ever justifies the size.

The diff-parser leniency reflects how models actually reply (a format example, then the change): one malformed example header must never veto a valid diff. Making thinking always-on via the prompt (rather than per-provider adapter work) gives a consistent reasoning UI everywhere with zero engine risk, and the "answer after `</think>`" rule keeps diff detection working. Image support deliberately attaches to the *current* turn only — avoiding a full multimodal refactor of the text-based message/persistence model while still letting users send screenshots to vision models today.

## Impact

- The AI pane now renders proper Markdown (incl. tables), LaTeX math, and simple flowcharts; surfaces AI edits as a clear per-file review card; shows reasoning for any model; accepts image input for vision models; and has resizable, clearly-divided panes with sane limits.
- **New (frontend):** `ai/markdown.ts`, `ai/math.ts`, `components/MermaidGraph.svelte`. **Changed (frontend):** `components/AiMessage.svelte` (Markdown + math + graphs + proposal card + image thumbs), `AiPanel.svelte` (image upload UI), `ResizeHandle.svelte`, `stores/panels.ts`, `stores/ai-chat.ts`, `ai/ai-chat-setup.ts`, `ai/diff-detection.ts`, `tauri/commands.ts`.
- **Changed (engine):** `ai/diff.rs` (lenient hunk header), `ai/provider.rs` (`ImageAttachment` + `MessageRequest.images`), `ai/mod.rs`, `ai/anthropic.rs` / `ai/openai.rs` / `ai/gemini.rs` (image branch). `frontend/src/main.rs` (system prompt + `ai_send` images param). **No new dependencies** (Rust or frontend).
- **Verification:** `cargo test -p gwenland-engine` → **243 passing** (single-threaded; new diff + image tests included); `cargo check --workspace` clean; `svelte-check` 0 errors / 0 warnings (155 files); `vite build` succeeds; JS bundle ≈ 939 kB (≈ 273 kB gzip), up only ~14 kB for the from-scratch math/graph code.
- **Things to keep in mind:**
  - **From-scratch limits:** math has no matrices / `align` / sized integral bounds; graphs handle simple layered flowcharts only (subgraphs and dense cross-edges may overlap → falls back to a source box).
  - **Images are current-turn only** — not stored or replayed; reloading a conversation won't show past images.
  - **No model-capability gating for images** — sending an image to a non-vision model returns a provider error in the warning banner. Use a vision model (Gemini Flash/Pro, Claude vision, GPT-4o, etc.).
  - **Verified by tests + type-check + build, not yet a full GUI pass.** The remaining manual smoke: a `$$…$$` equation, a ` ```mermaid ` graph, a real diff → review card → accept/reject, and pasting a screenshot to a vision model.
