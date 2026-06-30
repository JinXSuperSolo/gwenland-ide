# M23: Binary Diet ships, sidebar becomes tabbed, AI reviews your diffs, and windows multiply

- **Date:** 2026-06-29
- **Issue:** GWEN-425 … GWEN-431
- **Milestone:** Milestone 23 — Binary Diet + Feature Wave

## Why this milestone

M23 had two jobs. The first was structural: cut the release binary size by removing JavaScript dependencies that had no business being there (`marked`, `katex`, `iconoir`, `reqwest`, `rustls`) and replacing them with purpose-built Rust or inline alternatives. The second was experiential: three new features — a tabbed sidebar, an AI diff reviewer, and multi-window support — that change how the IDE feels to use daily.

Both waves shipped. Binary dropped from 7.3 MB to **6.0 MB** with the full feature set added on top.

---

## Wave 1 — Binary Diet

### GwenSyntaxRendering: Rust → WASM (GWEN-425)

`marked` and `katex` are gone. In their place is `gwenland-syntax-renderer` — a new Rust crate compiled to WebAssembly via `wasm-pack`. It exposes a single function: `render_markdown(source: &str) -> String`.

Internally it handles three things:

- **Markdown:** CommonMark + GFM subset — headings, paragraphs, bold/italic, code fences, tables, task lists, strikethrough, blockquotes, links.
- **Math:** LaTeX inline (`$...$`) and block (`$$...$$`) parsed directly to **MathML** — `<math>`, `<mfrac>`, `<msup>`, `<msub>`, Greek letters, `\frac`, `\sum`, `\int`, superscript, subscript. Because GwenLand runs in a Tauri webview (Chromium-based), MathML renders natively with zero runtime CSS. No KaTeX stylesheet, no fonts, no JS.
- **Code blocks:** basic token coloring — keywords, strings, comments, numbers, operators — for Rust and TypeScript/JavaScript.

The WASM module loads **lazily**: the first call to `render_markdown` in a session triggers the async fetch of `gwenland_syntax_renderer_bg.wasm`; subsequent calls use the cached instance. App startup is not affected.

Markdown and math output is injected into Svelte components via `{@html html}` — same pattern as before, different (faster, smaller) source.

### GwenIconRegistry: iconoir removed (GWEN-426)

All 45 icons previously imported from `iconoir` are now inline SVG strings in `gwenland-icons.ts`. The registry exports `getIcon(name, size?, color?)` which returns a formatted SVG string with correct `width`, `height`, and stroke/fill values applied via string replacement.

`Icon.svelte` now calls `getIcon` instead of importing from `iconoir`. The `iconoir` package is gone from `package.json`. Visual output is identical.

### reqwest + rustls removed (GWEN-427)

The engine no longer depends on `reqwest` or `rustls`. HTTP streaming to AI providers now goes through **system `curl`** — spawned as a child process via `tokio::process::Command`, with the request body piped to stdin and the SSE response streamed from stdout via `tokio::io::BufReader`.

A few things this required:

- Headers (including `HTTP/1.1` status and `retry-after`) are parsed from stdout before the body stream begins.
- On Windows, curl is spawned with `CREATE_NO_WINDOW` (`0x0800_0000`) to prevent a console window from flashing.
- All four provider adapters (Anthropic, OpenAI, Gemini, generic) were updated to use the new `curl_client.rs` wrapper.

The result: no TLS library linked into the binary, no HTTP framework. curl is present on every supported platform (Windows 10+, macOS, Linux).

### withGlobalTauri false + minimal installer (GWEN-428)

`withGlobalTauri` is now `false` in `tauri.conf.json`. All Tauri API calls in the Svelte frontend use direct imports from `@tauri-apps/api/*` — no more `window.__TAURI__` global. This unlocks better tree-shaking of the Tauri JS bindings.

A minimal NSIS installer stub was also added: the stub is ~50 KB and downloads the release binary from GitHub Releases on first run, rather than bundling it. The stub handles download, extraction, and shortcut registration.

---

## Wave 2 — Feature Wave

### Sidebar Tabbed Pane (GWEN-429)

The left sidebar is now a tabbed pane. Three tabs live at the **bottom of the sidebar**:

- **Files** (default) — the file tree, unchanged from M21.
- **Agent** — the AI chat panel, moved here from its previous right-side position. The right column is gone.
- **Agent 2.0** — disabled, greyed out (opacity 0.4, `cursor: not-allowed`), tooltip reads "Coming Soon". This slot is reserved for the full agentic interface planned in a later milestone.

Tab switching is instant — no animation delay, just a clean content swap. The active tab is persisted to `localStorage` and restored on next launch.

**Styling:** active tab = solid `#b56936` background with white text. Inactive tabs = plain text, no background, muted color. No borders on the tab bar itself — a single hairline divider separates it from the content area above.

The `aiChat.isOpen` toggle now expands the sidebar and switches to the Agent tab instead of opening a right panel. The status-bar AI button follows the same logic.

### AI Diff Agent (GWEN-430)

The Git Diff view now has a **"Review with AI"** button in its toolbar (sparks icon, right side). Clicking it:

1. Pipes the output of `git diff` for the current file to the active AI provider (BYOK — whatever you've configured in Settings).
2. Streams the review back as a formatted response.
3. Displays the result in a **bottom drawer inside the Git Diff tab** — not a new tab, not a popup.

The review is structured per-hunk: each section explains what changed, flags potential bugs or issues, and where relevant suggests an improvement. Output is rendered through `gwenland-syntax-renderer` (the WASM module from GWEN-425), so code blocks in the review are syntax-highlighted.

A spinner shows during inference. A **Copy** button appears once the review lands. Closing the drawer and re-clicking "Review with AI" triggers a fresh review.

### Multi Window — Open New Window (GWEN-431)

`File → New Window` (also `Ctrl+Shift+N`) spawns a new editor window via Tauri's `WebviewWindowBuilder`. Each window:

- Opens with a fresh Welcome Screen and its own folder dialog.
- Maintains a fully independent workspace — separate file tree, tabs, terminal sessions, and editor state.
- Shares `settings.toml` — theme, fonts, provider keys, and all preferences are consistent across windows.
- Gets a unique runtime label (`window-1`, `window-2`, ...) to prevent collisions.

No IPC between windows is required or implemented. Windows are naturally isolated because each `WebviewWindow` runs as a separate Chromium process — Svelte stores and DOM state don't leak between them.

There's no limit on how many windows you can open. Closing one doesn't affect others.

---

## Good to know / limits

- **curl must be available on PATH.** On Windows 10+ and macOS 10.15+ it is bundled with the OS. On Linux it is almost always present; if not, `apt install curl` / `pacman -S curl`. GwenLand will surface a clear error if curl is missing at the point of first AI request.
- **MathML rendering requires Chromium 109+.** Tauri's embedded webview meets this requirement on all supported platforms. If you're testing in an older browser outside of Tauri, math won't render.
- **Agent 2.0 is a placeholder.** The tab exists to establish the layout slot. The full agentic interface (full-screen takeover, history + chat input only, no file tree) is scoped for a later milestone.
- **AI Diff review is per-file.** Reviewing all changed files in one shot is not yet supported — click "Review with AI" in each file's diff tab separately.
- **Multi-window settings sync is eventual.** If you change a setting in Window 1, Window 2 picks it up on next launch (both read from the same `settings.toml`), but live sync between open windows is not implemented.

## Verification

- `cargo test -p gwenland-engine` → **493 passing**; `cargo check --workspace` clean.
- `cargo tree` → no `reqwest`, no `rustls` nodes.
- `svelte-check` → 0 errors / 0 warnings; `vite build` succeeds.
- `package.json` → no `marked`, `katex`, `iconoir` entries.
- Sidebar tabs: Files → file tree loads; Agent → AI chat panel loads; Agent 2.0 → disabled, tooltip shows.
- AI Diff Agent: "Review with AI" button visible in Git Diff toolbar; review drawer renders on click.
- Multi Window: `Ctrl+Shift+N` spawns independent window; settings shared; closing one doesn't affect other.
- Release build: `GwenLand-IDE.exe` **≈ 6.0 MB** (down from 7.3 MB); MSI and NSIS stub both build clean.
