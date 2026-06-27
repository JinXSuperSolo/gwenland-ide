# GwenLand IDE Roadmap

GwenLand IDE aims to be a lightweight, local-first developer workspace that feels fast on modest hardware while still covering the core IDE loop: editing, terminal work, Git, language support, AI assistance, agentic coding, memory, safety, and performance-aware project handling. The roadmap favors durable foundations over feature volume: every addition must preserve the small binary, the Rust-owned core, and the ability to run without sign-in, telemetry, or cloud services.

## Where We Are Today

The current release target described for this roadmap is **0.1.14**, aligned with **milestone M19**. The shipped product surface already includes the major pillars that define GwenLand IDE: editor, terminal, Git, LSP, AI assistant, agentic coding, memory, safety, and performance/scalability work.

| Pillar | Current Status | Roadmap Meaning |
|---|---|---|
| Editor | Shipped | Future work should deepen editing quality without turning the editor into a heavyweight runtime. |
| Terminal | Shipped | Future work should improve throughput, buffering, safety routing, and reliability. |
| Git | Shipped | Future work should improve everyday source-control workflows without introducing dependency bloat. |
| LSP | Shipped | Future work should mature language support while keeping protocol handling lean and local. |
| AI assistant | Shipped | Future work should make assistance more contextual, safer, and more useful offline/local-first where possible. |
| Agentic coding | Shipped | Future work should strengthen planning, preview, patch application, and rollback behavior. |
| Memory | Shipped | Future work should make workspace and assistant memory more explainable, inspectable, and local. |
| Safety | Shipped | Future work should keep destructive action control centralized and consistent across UI, agent, terminal, Git, and tools. |
| Performance/scalability | Shipped | Future work should treat perceived speed, low memory use, and graceful degradation as product features. |

The roadmap below only describes future direction. Items already shipped in the current release line are treated as foundation, not as planned work.

## Guiding Principles

### Local-first core

GwenLand IDE must not require mandatory sign-in, telemetry, cloud sync, or cloud accounts for the core IDE experience. Networked or remote capabilities may only be considered when they are optional and do not compromise local operation.

### Zero dependency growth

The project constraint is **zero new Rust crates** and **zero new npm packages**. New capabilities must be built from the existing stack and from scratch where necessary. This constraint keeps the system understandable, auditable, and small.

### Binary budget discipline

The release executable must stay at or below **7 MB**. The current budget position is approximately **4.8 MB**, so new work must earn its weight. Features that require heavyweight bundled runtimes, large native libraries, or dependency chains are not compatible with the core roadmap.

### Rust owns state

Rust owns durable state, workspace state, safety decisions, filesystem work, terminal process handling, Git interaction, and core orchestration. The Svelte UI renders diffs, patches, and compact state views. The engine must remain independent of Tauri so it can stay portable, testable, and reusable.

### Performance is a feature

The reference machine is an **11th-generation Intel i3, 8 GB RAM, no GPU, Windows 11**. Smoothness on that target matters more than impressive behavior on high-end hardware. The IDE should prefer incremental rendering, bounded memory, lazy initialization, and visible progress over blocking startup or expensive background work.

## Roadmap by Horizon

### Near-term (next few releases)

| Area | Item | Value |
|---|---|---|
| Editor | Multi-cursor fundamentals | Add dependable multi-cursor editing for common workflows such as repeated edits, aligned changes, and quick structural cleanup. This fits the constraints because it can be implemented inside the existing editor model without new packages or heavyweight services. |
| Editor | Large-file guardrails | Strengthen large-file behavior with clearer thresholds, feature degradation, and visible status when expensive editor features are disabled. This protects the reference hardware and keeps editing usable under memory pressure. |
| File tree / workspace | Incremental tree updates | Improve workspace tree updates so file changes are applied as small patches instead of broad refreshes. This matches the Rust-state/Svelte-diff architecture and directly improves perceived speed. |
| Git | Interactive staging basics | Add a focused staging flow for selecting files and hunks before commit. The value is high for daily development, and the implementation can stay local by using existing Git integration and internal diff handling. |
| Git | Safer diff review flow | Make staged, unstaged, and agent-proposed changes easier to inspect before execution. This strengthens the safety model without requiring cloud services or new dependencies. |
| Terminal | Output buffering and backpressure | Improve terminal smoothness under heavy output by batching UI updates, capping costly redraws, and preserving responsiveness. This is directly aligned with low-end hardware support. |
| AI assistant | Better plan-preview-apply loop | Improve agentic coding so the assistant plans changes, previews patches, and applies only approved work through the safety layer. This increases trust while preserving local control. |
| Safety | Unified action audit view | Provide a readable local view of risky actions, approvals, snapshots, patches, and rollbacks. This makes the safety system visible without sending telemetry or requiring accounts. |
| Performance | Startup readiness milestones | Make startup stages clearer: window ready, workspace loaded, editor ready, Git ready, terminal ready, and assistant ready. This improves perceived performance and helps diagnose regressions. |
| UI / accessibility | Keyboard navigation pass | Tighten keyboard access across tree, tabs, command surfaces, dialogs, terminal focus, and safety prompts. This is high value and mostly implementation discipline rather than dependency growth. |

### Mid-term

| Area | Item | Value |
|---|---|---|
| Editor | Lightweight refactor actions | Add carefully scoped refactor actions where language support already exposes safe edits. The work should stay protocol-driven and avoid building a heavy language intelligence layer inside the UI. |
| Editor | Symbol and outline depth | Improve project navigation through symbols, outline, and file-local structure views. This gives users faster movement through code while fitting the existing LSP-centered direction. |
| Git | Blame and history views | Add lightweight blame and file history views for local repositories. This provides important source context without cloud accounts or hosted Git integrations. |
| Git | Merge conflict UI | Provide a compact merge-conflict resolution surface with clear local file state and explicit apply actions. It fits the roadmap if built on existing diff/editor primitives and routed through safety checks. |
| LSP | More robust server lifecycle | Improve language-server start, stop, restart, timeout, and failure reporting. This improves reliability without bundling more language servers or package managers into the core. |
| Plugin system | Local plugin permissions | Mature plugin capabilities through a local permission model for filesystem, terminal, Git, and agent actions. This allows extensibility while keeping Rust-owned safety as the gate. |
| AI assistant | Tool reliability and context control | Improve how the assistant selects files, diffs, terminal output, diagnostics, and memory as context. The value is better answers with less noise, while keeping all project context under user-visible control. |
| AI assistant | Local-model integration path | Explore optional local model support if it fits the binary and dependency budget. This should not become a bundled heavyweight runtime in the core IDE. |
| Search | Faster local search UX | Improve search progress, cancellation, ignore handling, and streamed results. This keeps search responsive on large folders without adding cloud indexing. |
| Theming | Compact theme polish | Expand theming through small token-level controls and stable light/dark behavior. This must remain simple and should not become a full visual builder. |
| Packaging | Windows polish | Improve installer/startup polish, file associations, shell integration, and update ergonomics where possible within the binary budget. Windows remains the reference environment. |

### Long-term / exploratory

| Area | Item | Value |
|---|---|---|
| Debugging | Minimal debugger foundation | Explore a small debugging surface for launch, stop, step, variables, and console where it can be done without heavyweight bundled runtimes. This is valuable but should remain exploratory until the protocol and binary impact are proven. |
| Remote workspaces | Optional SSH-style workflow | Consider remote workspace support only if it can remain optional and local-first in the core. It must not introduce mandatory accounts, telemetry, cloud sync, or large runtime dependencies. |
| Plugin system | Stable extension API | Explore a stable local extension API with strict permissions and bounded execution. This can make GwenLand more capable, but only if extension power stays behind the safety engine. |
| AI assistant | Offline-first agent mode | Explore richer local/offline agent behavior, including local planning memory and local model routing. This is only acceptable if it preserves user control, avoids mandatory cloud services, and fits the size budget. |
| Performance | Adaptive low-resource mode | Explore automatic degradation when memory, CPU, file count, or terminal output becomes expensive. The goal is to keep the IDE interactive instead of chasing maximum feature activation. |
| Accessibility | Full accessibility audit | Perform deeper accessibility work across keyboard use, contrast, focus states, screen-reader labeling, and reduced-motion behavior. This should be treated as core quality, not visual polish. |
| Cross-platform | Linux and macOS packaging polish | Improve non-Windows packaging once the Windows reference path remains stable. Cross-platform work should not compromise the small binary or Rust-owned engine separation. |
| Workspace scale | Huge-repo strategy | Explore more advanced handling for very large repositories, including partial indexing, smarter excludes, and visible-first file metadata. This must be incremental and measurable on the reference hardware. |

## Explicit Non-Goals

| Non-goal | Reason |
|---|---|
| Mandatory cloud accounts | Breaks the local-first core and makes basic IDE use dependent on external services. |
| Telemetry in the core IDE | Conflicts with the privacy and local-first direction. Performance diagnostics should remain local and user-visible. |
| Mandatory cloud sync | Adds network dependency and state complexity that does not belong in the core IDE. |
| Heavy bundled runtimes | Risks breaking the 7 MB executable budget and increasing startup/memory pressure. |
| Dependency bloat | The project explicitly follows zero new Rust crates and zero new npm packages. New feature work must respect that gate. |
| Marketplace-first extension strategy | A marketplace model would push the IDE toward account, network, security, and dependency complexity too early. Local plugins and permissions should mature first. |
| AI with unrestricted execution | Agentic coding must remain gated by preview, safety classification, confirmation, snapshots, and rollback where needed. |
| Silent destructive actions | Deletion, overwrite, reset, risky terminal commands, and agent-applied patches must not bypass the safety layer. |
| GPU-dependent UI performance | The reference machine has no GPU requirement. Smoothness must come from efficient architecture, not assumed acceleration. |
| Full visual builder in the core IDE | A heavy builder would expand scope, size, and UI complexity. The IDE should prioritize coding workflows and compact workspace control. |
| Bundled remote/cloud development platform | Remote development may be explored only as optional capability. The core product is not a cloud IDE. |
| Unlimited language-server bundling | More language support is useful, but bundling many servers would threaten size, maintenance, and startup cost. The core should manage LSP well rather than ship everything. |

## How Priorities Are Decided

This roadmap is a direction, not a fixed commitment. GwenLand IDE should continue to prioritize the work that improves everyday coding confidence on the reference hardware.

Every addition is gated by three questions: does it preserve local-first operation, does it respect the zero-dependency rule, and does it keep the executable within the 7 MB budget. If a feature is useful but makes the IDE slower, larger, less auditable, or dependent on cloud services, it should be deferred, redesigned, or excluded from the core.

Performance, safety, and maintainability are not separate tracks. They are the product filter for every roadmap decision.
