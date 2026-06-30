# M22: LSP restored, Codex-like syntax highlighting, and the network-dependency root cause

- **Date:** 2026-06-29
- **Issue:** GWEN-406 … GWEN-410
- **Milestone:** Milestone 22 — Stability, LSP Fix & Syntax Polish

## Why this milestone

M21 restructured the engine from 24 flat files into 7 domain folders. That reorganisation was clean and necessary — but it quietly surfaced a pre-existing condition that made LSP look broken: the language servers never spawned at all, leaving the editor with zero diagnostics, zero red underlines, and no hover info across every language. At the same time, syntax highlighting was functional but visually flat — the token colors didn't communicate hierarchy, making code harder to read at a glance. M22 fixed both, in that order.

## Root cause: network dependency, not spawn logic

The post-M21 investigation (GWEN-406) found that LSP spawn logic itself was intact. The real culprit was a **network dependency on first launch**: `rust-analyzer`, `typescript-language-server`, and `pyright` all attempt to download components, indexes, or type stubs on initial setup — and without an active network connection, they silently exited before attaching. The engine's spawn path consumed the failure without surfacing it, so the editor presented a clean UI over a completely dead LSP layer.

The fix (GWEN-407) moved error handling to the surface: failed spawns now emit a visible status-bar notification ("LSP unavailable — check network or run server setup manually") instead of disappearing. A retry path was also added for transient failures. With connectivity present, all three servers attach correctly and stay attached across workspace switches.

## How the features work

### 1. LSP — diagnostics, hover, go-to-definition

With M22 shipped, LSP behaves as originally designed in M6:

- **Red underlines** appear for type errors, missing imports, and syntax issues. They update within ~500ms of a save, sooner on fast machines.
- **Hover info** shows on cursor rest — type signatures, doc comments, and inferred types render in a small popover above the word. No click required.
- **Go to Definition** is bound to `F12` (or right-click → Go to Definition). Cross-file navigation works; the target file opens in the active editor group.
- **Inline diagnostics** appear in the gutter as colored dots (red = error, yellow = warning). Hovering a dot expands the full message.

If LSP is unavailable (no network on first run, or server not installed), the status bar shows a muted "LSP off" indicator. Clicking it shows the reason and a "Retry" button. Nothing crashes; editing continues without LSP features until the server attaches.

Supported languages: Rust (`rust-analyzer`), TypeScript and JavaScript (`typescript-language-server`), Python (`pyright`). Adding more follows the same registration pattern in the LSP config.

### 2. Syntax highlighting — Codex-like warm palette

The previous highlight style used CodeMirror's default token colors — functional, but cold and undifferentiated. M22 replaces it with a custom `HighlightStyle` tuned for GwenLand's warm dark background (`#1b1918`):

| Token | Color | Rationale |
|---|---|---|
| Keywords (`fn`, `let`, `if`, `return`) | `#b56936` / `#d68d5c` | GwenLand orange — primary brand, high contrast |
| Strings | `#89b96e` | Warm green — immediately distinct from keywords |
| Comments | `#5a5a5a` | Muted grey — present but not competing |
| Functions / Methods | `#dcdcaa` | Warm yellow — readable, distinct from types |
| Types / Classes | `#4ec9b0` | Teal — cool contrast against warm bg |
| Numbers / Constants | `#b5cea8` | Soft green — quiet but findable |
| Variables | `#9cdcfe` | Light blue — the most common token, calm |
| Operators / Punctuation | `#cccccc` | Light grey — structural, not decorative |
| Plain text | `#d4d4d4` | Off-white — comfortable reading base |

The palette is applied globally — the same `HighlightStyle` covers Rust, TypeScript, JavaScript, Python, JSON, TOML, and Markdown. No per-language overrides needed; token categories map consistently across grammars.

The result is closer to the Codex / VS Code Dark+ aesthetic: warm, high-contrast, calm. Code reads as structured rather than rainbow-painted.

## Post-M22 notes

- LSP retry logic fires automatically when network becomes available if the initial spawn failed. No manual restart needed.
- Hover popovers respect the active editor group — if you have a split view open, hover shows relative to the focused panel.
- Syntax highlight changes apply immediately on file open; no restart required after the M22 update.

## Good to know / limits

- **LSP on first-ever launch still requires network.** If `rust-analyzer` or `typescript-language-server` has never been run on the machine, it needs to download components once. Subsequent launches work offline.
- **Go to Definition across workspaces** is not supported — navigation is scoped to the currently open folder.
- **Markdown syntax highlighting** uses a simplified token set — headings, code fences, bold/italic, and links are colored; inline HTML inside Markdown is not.
- **Light mode** is not yet supported. The highlight palette is dark-only and assumes `#1b1918` background.

## Verification

- `cargo test -p gwenland-engine` → **493 passing**; `cargo check --workspace` clean.
- `svelte-check` → 0 errors / 0 warnings; `vite build` succeeds.
- LSP smoke test: Rust, TypeScript, JavaScript — diagnostics visible, hover works, F12 navigates correctly.
- Syntax highlight smoke test: Rust, TypeScript, JavaScript, Python, JSON, TOML, Markdown — token colors match palette table above.
- Release build: `GwenLand-IDE.exe` **≈ 6.0 MB**; no regression from M21 feature set.
