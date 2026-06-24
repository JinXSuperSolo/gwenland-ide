# Web Preview (M5): dev-server ready detection pipeline + generalized tab model with a preview surface

- **Date:** 2026-06-21
- **Issue:** GWEN-252 onward (M5 — Web Preview)
- **Milestone:** Milestone 5 — Web Preview (first slices)

## Problem / Context

M5 is the in-IDE web preview: one surface, two source kinds — a local static HTML file loaded over `file://`, and a running dev server (Vite/Next/CRA/etc.) loaded over `http://localhost:PORT`. The spec settled the engine question up front: render through the host's native OS webview (WebView2 on Windows, WebKit elsewhere) via Tauri/WRY, not an embedded Chromium/CEF — that is what has kept the binary inside its budget through M1–M4, and a second rendering pipeline was explicitly rejected.

The spec was drafted and the Linear issues already existed, so this session was about starting the implementation. Rather than build the whole feature in one go, the work was taken as two coherent slices that each end somewhere the app still compiles and the existing behaviour is untouched:

1. **The detection pipeline** — how the IDE notices a dev server is ready, end to end from the engine to a typed front-end API. This reuses the exact shape of the M3/M4 terminal-error bridge.
2. **The tab model generalization** — making room in the center workspace for a non-editor tab, plus the preview surface itself.

The repo layout is unchanged from M3/M4: `engine/` is the zero-Tauri core (pure logic, fully unit-tested), `frontend/src/main.rs` is the thin Tauri command-and-event bridge, and the Svelte app lives in `frontend/ui/`. pnpm throughout.

## Change

### Slice 1 — Dev-server ready detection (engine → Tauri → front-end API)

**`engine/src/devserver_detect.rs` (new).** A reactive, per-chunk scanner for the line a dev server prints once it has bound its port — Vite's `➜  Local: http://localhost:5173/`, Next's `- Local: http://localhost:3000`, CRA's `Local: http://localhost:3000`. It is deliberately the twin of `error_detect.rs` from M3 Wave 6: stateful with a one-line carry-over, it inspects only newly completed lines (never a re-scan of the whole scrollback), and it is dependency-free — no `regex` crate.

The key observation that kept it simple: the framework-specific patterns in the spec (Vite/Next/CRA, each written as its own regex) all reduce to the same thing — *a loopback host with an explicit port*. So instead of three regexes there is one hand-rolled extractor that scans an ANSI-stripped line for `localhost`, `127.0.0.1`, or `0.0.0.0` immediately followed by `:` and 1–5 digits, parses and range-checks the port (1–65535), and reads the scheme from a preceding `https://` (defaulting to `http`). A `0.0.0.0` bind address is normalised to `localhost` because browsers cannot navigate to `0.0.0.0`. The result is a `DevServerSignal { url, port }` with a browsable base URL.

Two behavioural decisions are baked in. First, the detector **latches**: it reports the first ready URL and then stays quiet (a `reset()` re-arms it). That suits the use case — auto-open the preview once — and means a server that re-announces after a hot reload will not keep firing. Second, ANSI stripping is shared rather than duplicated: `error_detect::strip_ansi` was made `pub(crate)` and reused, since dev servers colourise their URLs and the same colour-stripping is needed to match them. The module ships with 16 unit tests covering each framework's line, the bare-host-no-scheme case, https preservation, `0.0.0.0` normalisation, detection through colour codes, chunk-boundary reassembly, the newline requirement, the latch and reset, earliest-match-on-a-line, and the false-positive guards (no port, out-of-range port, six-digit number, clean output).

**`engine/src/terminal.rs`.** The detector was wired into the existing reader thread next to the error detector, mirroring it exactly: a new `DevServerCallback` type, a latched `Arc<Mutex<Option<DevServerSignal>>>` on `PtySession`, a `dev_server_signal()` accessor, and the third optional callback threaded through `spawn_with_callback` / `spawn_inner`. Because the detector latches, the per-session work is "scan until found, then nothing." A session-level integration test (`echo`-ing a Vite-style line into a real shell, mirroring the existing error-flag test) confirms the URL and port come back through the whole stack.

**`frontend/src/main.rs`.** A new `terminal://devserver-ready` event carrying `{ id, url, port }`, emitted at most once per session. It is a *separate* event from `terminal://error` by deliberate choice — "a server is ready" and "a line looked like an error" are different concerns and overloading the error bus would have muddied both. The callback was added to `terminal_create` alongside the existing output and error callbacks (each now gets its own cloned `AppHandle`).

**`frontend/ui/src/lib/tauri/commands.ts`.** Typed wrappers `onDevServerReady` (session-scoped) and `onAnyDevServerReady` (all sessions, for a not-yet-session-scoped preview controller), plus the `DevServerReady` interface and event constant — the same pattern as the terminal-output and terminal-error wrappers.

### Slice 2 — Generalizing the tab model

The center workspace was tab-based but every tab was an editor: the `Tab` interface hard-coded `path`, `baseline`, a CodeMirror `EditorState`, and `dirty`. A preview tab does not fit that shape, so before anything could be previewed the model had to grow a second kind. Two options were on the table — generalize `Tab` into a discriminated union, or stand up a separate preview surface beside the tabs. The union was chosen so a preview is "just another tab," matching the Cursor-style center-panel goal in the spec.

**`frontend/ui/src/lib/stores/tabs.ts`.** `Tab` is now `EditorTab | PreviewTab`, discriminated on a `kind` field. Shared fields (`id`, `name`) live on a common base; the editor-only state (`path`/`baseline`/`state`/`dirty`) is confined to `EditorTab`, and `PreviewTab` carries only a `PreviewSource` (`{ kind: 'static-file', path } | { kind: 'dev-server', url, port }`, mirroring the engine's spec enum). Two type guards, `isEditorTab` / `isPreviewTab`, refine the type. A new `openPreview(source)` action opens or focuses a preview tab, deduping by source key (file path / server URL) and updating the source in place on re-open — so a dev server that comes back on a different port reloads the existing tab rather than spawning a duplicate.

The important part of this refactor is that **editor behaviour is byte-for-byte unchanged.** Every editor-only function now narrows with a guard before touching editor fields: `openFile` and `newUntitledFile` construct `EditorTab`s and dedup/count only editor tabs; `persistTabState`, `recomputeDirty`, `saveTab`, `saveActiveTab`, and `closeActiveTab` all no-op or skip cleanly when handed a preview tab.

**The consumers** were updated to narrow rather than assume:

- `Workspace.svelte` now branches the surface below the tab strip on the active tab's kind — `Editor` for an editor tab, `PreviewPane` for a preview tab, the empty state otherwise. Editor-to-editor switches still swap *inside* the `Editor` component (no remount, so cursor/scroll/undo are preserved as before); only an editor-to-preview switch unmounts the editor, which persists its state to the store on the way out. Its close handler gates the unsaved-changes confirm behind `isEditorTab`.
- `Editor.svelte`'s `mountTab` guards on `isEditorTab` and cleans up the active-editor handle defensively, so a preview tab can never be treated as a document regardless of render ordering.
- `Tabs.svelte` gates the dirty dot to editor tabs and shows the right tooltip per kind (path for editors, source target for previews).
- `ai-chat-setup.ts`'s `currentFilePath` returns null for a preview tab, so the AI "attach current file" feature ignores previews.

**`frontend/ui/src/lib/components/PreviewPane.svelte` (new).** The preview surface: a toolbar (a globe icon, the current URL/path, a reload button) over an iframe filling the rest. It is one pipeline for both source kinds — a dev server loads its `http://` URL directly; a static file loads through Tauri's asset protocol via `convertFileSrc`. The iframe is rendered by the host's native WRY webview, so this honours the spec's "single native pipeline, no second engine" decision — an iframe is the same WebView2/WebKit engine, not embedded Chromium. Reload re-keys the iframe element to force a fresh load, because the previewed origin (localhost / asset) differs from the app's and so `iframe.contentWindow.location.reload()` is blocked cross-origin. Two icons (`globe`, `refresh`) were added to the `Icon.svelte` registry to support the toolbar.

## Why this approach

Modelling dev-server detection on the existing error detector — same per-chunk reactive scan, same dependency-free substring matching, same callback-into-the-bridge wiring — meant the engine piece was correct and fully tested before any UI existed, and the next reader of the code finds one consistent pattern rather than two. Collapsing the spec's three framework regexes into a single loopback-host extractor is less code and covers more (any dev server that prints a localhost URL is caught by the generic case), while the latch keeps the blast radius of the heuristic small. Keeping the new event distinct from `terminal://error` avoids coupling two unrelated signals.

For the tab model, the discriminated union is the change that lets a preview be a first-class tab while leaving the editor path provably untouched — the type guards make "is this an editor?" a compiler-checked question at every site that used to assume it. Choosing an iframe over a positioned native child-webview for now keeps the work inside the existing webview and the binary budget; it is the same rendering engine and can be upgraded to a dedicated WRY child-webview later if iframe framing limits ever bite.

## Impact

- The IDE can now detect a dev server's ready URL from terminal output, end to end, and exposes it as a typed `terminal://devserver-ready` event. The center workspace can hold a working web-preview tab alongside file editors, rendered through the native webview.
- **New:** `engine/src/devserver_detect.rs`; `frontend/ui/src/lib/components/PreviewPane.svelte`. **Changed:** `engine/src/terminal.rs` (detector wired into the reader thread + accessor), `engine/src/error_detect.rs` (`strip_ansi` shared), `engine/src/lib.rs`; `frontend/src/main.rs` (event + payload + callback); `frontend/ui/src/lib/tauri/commands.ts` (wrappers), `stores/tabs.ts` (union + guards + `openPreview`), `components/Workspace.svelte`, `Editor.svelte`, `Tabs.svelte`, `Icon.svelte`, `ai/ai-chat-setup.ts`.
- The engine test suite stands at 134 passing (17 added this session: 16 dev-server unit tests + 1 PTY integration test), with the full terminal module re-run green to confirm the reader-thread change introduced no regression. The UI type-check is clean (`svelte-check`, 0 errors / 0 warnings, 136 files), the production build succeeds (`vite build`, 230 modules), and the frontend Rust crate compiles (`cargo check -p GwenLand-IDE`).
- **Things to keep in mind:**
  - **`openPreview` has no caller yet.** A preview tab can be created and rendered, but nothing triggers one. The obvious next step connects the two slices: an app-init controller listening on `onAnyDevServerReady` → `openPreview({ kind: 'dev-server', … })`, which the spec wants to auto-open. This was deliberately left out — it is a visible startup-behaviour change and a distinct open item.
  - **Static-file preview needs an asset-protocol scope.** `convertFileSrc` is wired, but `assetProtocol` in `tauri.conf.json` is enabled without a scope, so arbitrary local files may not load through it yet. The dev-server case (plain `http://localhost`) needs no capability and works as-is. An "Open Preview" context-menu trigger for `.html` files is also still to come.
  - **Verified by tests, build, and type-check — not yet by running the app.** Nobody has watched a real dev server auto-appear in a preview pane or dragged a preview tab around; that manual smoke test is still worth doing once the auto-open wiring lands.
  - **Deferred M5 items** (per the spec, untouched here): the static-file `notify` watcher + debounced live-reload, the port-poll fallback for when no ready line is printed, the "server stopped" state in the pane, and the benchmark numbers (binary-size delta, cold preview open, reload latency, idle RAM, and the retroactive IDE cold-start measurement).
  - The iframe currently sets no `sandbox` attribute by design (a local dev server / file is trusted content the user owns), so the preview behaves like a real browser tab.
