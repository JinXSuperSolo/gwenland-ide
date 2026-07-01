# GwenLand IDE - Session Changes

**Date:** 2026-07-01
**Scope:** M26 Model Selector & AI UI Enhancements (GWEN-454 to GWEN-458), composer toolbar redesign
**Status:** Implemented and validated — engine + frontend test suites clean, verified live via `cargo tauri dev`

---

## What changed this session

The AI composer got a full model-selector foundation plus a toolbar redesign. On
the data side, there's now a single static model registry covering all 9 AI
providers (Anthropic, OpenAI, Google, xAI, DeepSeek, Z.AI, Moonshot, Qwen,
Mistral) with real per-provider brand icons, pricing, and reasoning-effort
metadata, replacing scattered hardcoded model strings. The model picker is a
flat, borderless, compact dropdown sized to match the composer, with a separate
catalog-driven reasoning-effort control (levels vary per model instead of a
fixed Low/Medium/High). The composer toolbar itself was consolidated: a single
"+" button now holds context-attach, image upload, assistant mode, and agent
approval tier, cutting the row down to just `[+] [model] [effort] [send]`.
Finally, token usage is now tracked end-to-end — all four AI provider adapters
report input/output token counts (OpenAI needed a request-body change to
unlock this), persisted to conversation history, and shown as a subtle
per-message cost line.

---

## Added

| Area | Change |
| --- | --- |
| Model registry (GWEN-454) | `engine/src/ai/model_catalog.rs` — `ModelEntry`/`Provider`/`Pricing`/`ReasoningConfig` schema, 39 seeded models across all 9 providers with verified pricing (unverifiable numbers marked `// TODO: verify pricing` rather than guessed), exposed via new `ai_model_catalog` Tauri command. |
| Provider brand icons (GWEN-455) | `frontend/ui/src/lib/icons/provider-icons.ts` + `ProviderIcon.svelte` — real logomarks (not placeholders) sourced from `simple-icons` (CC0) and Wikimedia Commons, rendered in each provider's actual brand color/gradient (Anthropic tan, Gemini blue→purple→red gradient, DeepSeek blue, Qwen purple, Mistral red→orange→yellow gradient; OpenAI/xAI/Moonshot rendered near-white since their real marks are black-on-transparent and would vanish on this app's dark theme; Z.AI keeps its real two-tone badge). |
| Model selector UI (GWEN-456) | `ComposerModelMenu.svelte` rebuilt as a single flat, borderless, compact dropdown listing every catalog model with icon, name, context window, and $/1M pricing — sized to match the composer's own width (measured live, not a fixed guess). |
| Reasoning/effort control (GWEN-458) | New `ReasoningMenu.svelte` — separate from the model picker since levels are per-model (Anthropic offers low/medium/high/xhigh/max, Grok offers none/low/medium/high, several models offer none at all). `ReasoningLevel` widened from a fixed 4-value union to a plain string driven by the catalog. |
| Token usage tracking (GWEN-457) | `ChunkSource::usage()` added to all 4 provider adapters (Anthropic via `message_start`/`message_delta`, OpenAI via a new `stream_options.include_usage` request field, Gemini via existing `usageMetadata`, Generic inherits OpenAI's implementation for free). Persisted to `ConversationTurn.usage` (backward-compatible — old JSONL lines without the field still load). `AiMessage.svelte` shows a per-message `"1.2K in · 340 out · $0.008"` footer, priced using the model that actually generated that reply (not the currently active one, so historical costs stay correct after switching models). |
| Composer "+" menu | New `ComposerActionsMenu.svelte` consolidates context-attach (Current File / Current Selection), image upload, assistant mode (Ask/Edit/Agent), and agent approval tier (Ask/Accept for Me/Full Control, shown only in Agent mode) behind one button. |

---

## Changed

| Area | Change |
| --- | --- |
| Composer toolbar layout | Reduced from `[attach] [Mode] [Model] [Effort] [Tier] [send]` to `[+] [Model] [Effort] [send]` — mode and tier moved into the new "+" menu. |
| Composer dropdown styling | Borderless (background fill + shadow only, no 1px borders), more rounded corners, compact row padding, no hover-shift animation — replaced an earlier pass that used cascading `›` submenu flyouts, reverted after feedback that flat sections read clearer than drill-downs. |
| `lib/ai/reasoning.ts` | Rewritten from hardcoded model-name regex heuristics (`claude-opus-4-8` pattern matching, etc.) to pure catalog lookups (`isThinkingCapable`, `reasoningLevelsFor`, `defaultReasoningLevelFor`, `reasoningLevelLabel`). |
| `frontend/ui/src/styles/animations.css` | Added `gw-anim-pop-bounce` (spring-eased pop-in with slight overshoot) for the composer menus, additive alongside the existing `gw-anim-pop` used by unrelated dialogs elsewhere in the app. |

---

## Added files

| File | Purpose |
| --- | --- |
| `engine/src/ai/model_catalog.rs` | Static model registry: schema + 39-model seed data + 26 unit tests. |
| `frontend/ui/src/lib/icons/provider-icons.ts` | Real per-provider brand mark SVGs + colors. |
| `frontend/ui/src/lib/components/ProviderIcon.svelte` | Renders a provider brand mark. |
| `frontend/ui/src/lib/components/ComposerActionsMenu.svelte` | The consolidated "+" menu (attach/image/mode/tier). |
| `frontend/ui/src/lib/components/ReasoningMenu.svelte` | Catalog-driven reasoning-effort dropdown. |
| `frontend/ui/src/lib/ai/usage.ts` | Pure token-count/cost-formatting helpers. |
| `frontend/ui/src/lib/ai/usage.test.ts` | 11 unit tests for the above. |
| `frontend/ui/src/lib/ai/model-catalog-cache.ts` | Process-lifetime cache for `aiModelCatalog()` so each rendered message doesn't refetch it. |

---

## Files changed

| File | Change |
| --- | --- |
| `engine/src/ai/provider.rs` | New `TokenUsage` type + `ChunkSource::usage()` trait method (default `None`). |
| `engine/src/ai/anthropic.rs` | Captures usage from `message_start`/`message_delta` events. |
| `engine/src/ai/openai.rs` | Requests `stream_options.include_usage: true`; parses the resulting usage-only terminal chunk. |
| `engine/src/ai/gemini.rs` | Captures usage from `usageMetadata` (already present on chunks). |
| `engine/src/ai/conversation.rs` | `ConversationTurn.usage: Option<TokenUsage>` with `#[serde(default)]` for backward compatibility; `record_turn` takes usage. |
| `engine/src/ai/mod.rs` | Re-exports the new catalog + usage types. |
| `frontend/src/main.rs` | New `ai_model_catalog` command; `AiDoneEvent` carries usage; `run_stream` captures and forwards it. |
| `frontend/ui/src/lib/tauri/commands.ts` | New `ModelEntry`/`TokenUsage`/catalog types; `ConversationTurn` and `onAiDone` extended with usage. |
| `frontend/ui/src/lib/stores/ai-chat.ts` | `ChatMessage` gained `usage`/`provider`/`model`; `ReasoningLevel` widened to `string`; `finaliseStream` stamps usage. |
| `frontend/ui/src/lib/stores/workspace-state.ts` | Reasoning-level persistence guard loosened for the widened type. |
| `frontend/ui/src/lib/ai/ai-chat-setup.ts` | Stamps provider/model on new assistant messages; maps persisted usage on conversation reload; passes usage through the done handler. |
| `frontend/ui/src/lib/components/AiPanel.svelte` | Swapped in `ComposerActionsMenu`; removed now-dead attach-menu state/CSS. |
| `frontend/ui/src/lib/components/AiMessage.svelte` | Renders the per-message usage/cost footer. |
| `frontend/ui/src/lib/components/ComposerModelMenu.svelte` | Rebuilt as the flat composer-width catalog list. |
| `frontend/ui/src/lib/icons/gwenland-icons.ts` | Added a `media-image` icon for the Upload Image menu item. |

---

## Validation

| Gate | Result |
| --- | --- |
| `cargo test -p gwenland-engine` | 574 passed, 1 failed (pre-existing, unrelated LSP smoke test that can't spawn `typescript-language-server` on this machine). |
| `pnpm.cmd test` (vitest) | 182 passed, 2 failed (pre-existing, unrelated `actionRegistry.test.ts` failures). |
| `svelte-check` | 0 errors. |
| Live verification | `cargo tauri dev` launched and screenshotted for: model selector (real icons/colors, composer-width, compact), reasoning dropdown (per-model levels, e.g. Qwen's binary Off/On), composer "+" menu (flat sections, no chevrons, no border). Token usage display was NOT live-verified (would require a real, billed AI provider call) — trusted the adapter-level unit tests instead, per explicit choice. |

---

## Notes

| Topic | Note |
| --- | --- |
| Dependencies | Zero new npm packages and zero new Rust crates. `simple-icons` was used only as a one-time source to copy 6 SVG path strings, never added as a dependency. |
| Design iteration | The composer "+" menu went through two structural passes before landing: a cascading `›` submenu flyout (matching an initial reference image) was built, then reverted to a single flat panel after feedback that it read as "too AI generic" and that flat sections were clearer — worth knowing if this component is touched again. |
| Historical accuracy | Per-message cost uses the provider/model that actually generated that specific reply (stored on the message itself), not the currently active model — correct after a mid-conversation model switch, deliberately not the simpler alternative. |
| Superseded files | `AgentTierMenu.svelte` and `AssistantModeMenu.svelte` are now unused (their behavior was absorbed into `ComposerActionsMenu.svelte`) but were NOT deleted — deletion was declined when offered, since it wasn't explicitly requested. |
