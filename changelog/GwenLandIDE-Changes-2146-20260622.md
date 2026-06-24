# M8 (revision): how the polished AI pane, thinking blocks, and diff review actually work — plus post-M8 fixes and the warm-sand recolor

- **Date:** 2026-06-22
- **Revision of:** `GwenLandIDE-Changes-2119-20260622.md` (M8 — AI Chat Pane Polishing)
- **Issue:** GWEN-314 … GWEN-318
- **Milestone:** Milestone 8 — AI Chat Pane Polishing (usage-focused revision)

## Why this revision

The first M8 changelog documented *what was built and why* at the file level. This revision is the companion: it explains **how each feature is used** from the seat of someone driving the IDE, and folds in the changes made after that changelog landed — the "Open Folder" reactivity fix, the custom checkbox, the minimalist Settings panel, and the new warm-sand color palette. Treat the earlier entry as the engineering record and this one as the "what's new and how to use it" guide.

## How the features are used

### 1. Opening the AI pane

Toggle the pane from the **AI** button in the status bar (bottom-right). With **no folder open**, the pane shows a sparkle icon, *"Open a folder to start"*, and an **Open Folder** button — clicking it opens the same native folder dialog as File → Open Folder. With a **folder open but no conversation**, it shows *"Ask anything about your code"* and three suggestion chips.

- **Suggestion chips** (`Explain this file`, `Find bugs`, `Write a test`): click one and it creates a conversation if needed, drops that text into the composer, and focuses it so you can hit Enter (or keep typing).
- The pane is borderless and padded now; sections are separated by spacing/elevation rather than lines.

### 2. Choosing provider, model, and reasoning — the composer pill

All model controls live in **one pill in the composer toolbar** (bottom of the pane), next to the attach `+` and the send arrow. It reads like `Claude Opus 4.8 · High`. Click it (or focus it and press ↑/↓/Enter) to open one popup with three sections:

- **Reasoning** — Low / Medium / High / Extra High. *Only appears for thinking-capable models* (Anthropic `claude-3-7`+ / Claude 4 family; any provider whose model id contains `deepseek-r1`, `qwen3`, or `qwq`).
- **Provider** — Anthropic, OpenAI, Gemini, plus any OpenAI-compatible providers you've configured in Settings. Picking one refreshes the key status and model list; the popup stays open so you can then pick a model.
- **Model** — the provider's models, or a manual model-id input if the provider doesn't list any.

Keyboard: ↑/↓ move, Enter selects, Esc closes, click-outside closes. Everything persists through the existing settings, and **no API keys live in the UI** — only a "stored/missing" status (keys are entered in Settings and kept in the OS keychain).

### 3. Reading the model's reasoning — thinking blocks

When a model streams reasoning (either inline `<think>…</think>` text from local models like DeepSeek-R1/QwQ, or structured reasoning from Anthropic/Ollama), it renders as a **collapsible block above the answer**:

- While the model is thinking it shows **"Thinking…"** with a live, auto-scrolling, height-capped trace.
- When it finishes, the block **auto-collapses** to **"Thought for 4.2s"** (the measured duration). Click the header to expand/collapse it again.
- The answer renders below it as normal — raw `<think>` tags never appear in the answer.

Nothing to configure; it just shows up when the model produces reasoning, and stays out of the way when it doesn't.

### 4. Reviewing AI code changes — the diff review flow

This is the headline feature: **AI proposes, you decide.** Nothing is written to disk until you accept it.

1. **Ask for a change** ("refactor this function", "fix this bug"). GwenLand's system prompt asks the model to reply with a unified diff.
2. When the answer contains a valid diff, the assistant message shows a **"Proposed changes: N files · M hunks"** button. Click **Review**.
3. The first file opens in the editor with an **inline overlay**: removed lines are tinted red (with a red gutter), the proposed added lines appear as a green block under each hunk, and every hunk has inline **Accept** / **Reject** buttons.
4. A floating **Review Changes** bar sits over the editor with totals (files · hunks · +added · −removed) and **Accept All / Reject All / Cancel**. The AI pane shows a **Proposed changes** panel listing every file with per-file progress — click a file there to jump to it.
5. **Decide per hunk** (mouse, or keyboard while the editor is focused):
   - `]` next hunk, `[` previous hunk
   - `a` accept the active hunk, `r` reject it
   - `Esc` cancel the whole review
   (Shortcuts are suppressed while you're typing in the composer or model inputs.)
6. **Applying:** decisions are *staged* — when every hunk is resolved (or you hit **Accept All**), accepted hunks are applied in one pass and only the files that changed are saved. Hunks are matched by content, so if the file moved or you edited it, they still land in the right place; a hunk that no longer matches is marked **failed** and its file is left untouched. **Cancel** / **Reject All** exit without writing anything.

Multi-file proposals work even for files you haven't opened — those are written directly; open files refresh; the file you're viewing applies through the live editor so your undo history and unsaved edits are preserved.

### 5. Settings — theme, fonts, keys (now custom + minimalist)

Open Settings from the gear icon in the AI pane header (or the command palette). Post-M8 the panel is flatter and quieter: no divider lines, calmer preset chips, and the theme swatch is a round dot.

- **Theme preset** (Gwen Dark / Midnight / Slate) and **accent color** apply live.
- **Monospace font** preview updates as you pick.
- **Provider keys** are write-only inputs that save to the OS keychain and only ever show *stored/missing*.
- **Checkboxes** (training opt-in, LSP enable toggles) are now a **custom component** — a styled box with a check, fully keyboard-operable (Tab to it, Space to toggle).

## Post-M8 revisions included here

- **Fixed: AI pane stuck on "Open Folder" after opening a folder.** The empty-state check read the workspace with a one-shot `get()` inside a `$derived`, so it never re-evaluated. It now reads the workspace store reactively, so the pane switches to the conversation UI the instant a folder loads. (Pre-existing latent bug, not introduced by M8.)
- **New: custom `Checkbox` component** replacing the three native checkboxes (global training opt-in, per-conversation training opt-in, per-language LSP enable).
- **Minimalist Settings panel:** removed the panel border + header divider, flattened the preset chips, softened the font controls, and turned the theme swatch into a dot.
- **New color palette ("warm-sand"):** the whole app — global tokens, the Gwen Dark preset, and the AI pane — moved to a warmer hex palette (`#1f1e1e` background, `#c28a64` primary, `#fafafa` text). The AI pane's `--ai-*` tokens now derive from the global theme, so the pane follows both the palette **and** the accent picker. The theme storage key was bumped (`v2 → v3`) so a previously-saved accent won't override the new default.

## Good to know / limits

- **Reasoning level is a stored preference, not yet sent to providers.** It governs whether the selector shows and remembers your choice; wiring it into requests (where a provider supports it safely) is a follow-up — so no unsupported fields are ever sent.
- **Diff detection runs when a response finishes streaming**, not when you reload an old conversation. Re-opening a past chat won't re-offer review for a diff it contained.
- **Review keyboard shortcuts** work while the editor is focused (where you're reviewing) but are intentionally muted while a text field (composer/model input) has focus. The on-screen Accept/Reject buttons always work.
- **Dark theme only.** The recolor used the dark half of the supplied palette; there's no light-mode toggle yet.

## Verification (this revision)

- `cargo test -p gwenland-engine` → **240 passing**, now green **in parallel** (the conversation-test isolation was fixed); `cargo check --workspace` clean.
- `svelte-check` → 0 errors / 0 warnings (152 files); `vite build` succeeds.
- Release build (`cargo tauri build`, x64): `GwenLand-IDE.exe` **≈ 5.04 MB** (under the 7 MB budget); MSI ≈ 2.47 MB, NSIS setup ≈ 1.85 MB.
- **Still pending: a hands-on GUI pass.** The flows above are verified by tests, type-check, and a release build, but the visual smoke (empty states → chips, the composer popup, a live `<think>` answer, a real diff → review → accept/reject/cancel with no leftover overlay, and M6 diagnostics still rendering) is the remaining manual confirmation.
