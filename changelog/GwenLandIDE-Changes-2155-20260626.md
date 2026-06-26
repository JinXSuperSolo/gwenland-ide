# GwenLand IDE — Session Changes
**Date:** 2026-06-26 · **Session end:** ~21:55 · **Version bump:** 0.1.10 → 0.1.12

---

## What changed this session

Two focused follow-ups to the M18 feature push: a size fix and a mermaid replacement.

---

### Context menu appearance fixed

The context menu hover was using a faint rgba tint — didn't match the reference screenshot where the active item fills with a solid warm block. Fixed:

- Hover background is now `var(--primary)` (solid warm sand `#c28a64`), with `var(--primary-foreground)` (`#1f1e1e` dark) as the text color on the highlighted row
- Container radius switched from the old hardcoded `0.5rem` to `var(--radius)` (`1rem` from global tokens) — gives it the big rounded pill look from the screenshot
- No border on the container (was previously orange-tinted outline)
- Item rows get `calc(var(--radius) - 6px)` radius so the hover block sits cleanly inside the rounded shell
- Danger items (Move to Trash, Delete Permanently) keep their red `#e06c75` label and red-tinted hover — those still work on the dark background

---

### Exe bloat fixed: 10 MB → 4.95 MB

The `mermaid` npm package was being embedded into the Tauri exe via the `dist/` bundle even though it's only needed when a `.md` file with a diagram block is open. It pulled in `cytoscape` (435 KB), `dagre`, `swimlanes`, KaTeX, and 20+ diagram-type chunks — over 3 MB of the 5.2 MB dist.

**What was removed:** `mermaid` package uninstalled from `package.json`. All diagram-related code paths in `MarkdownPreview.svelte` referencing the library removed.

**KaTeX made lazy:** `katex` was statically imported in `markdown.ts`, meaning it always ended up in the cold-start bundle even if you never opened a math file. Converted to a dynamic import — only fetched on first render of a file containing `$...$` or `$$...$$`. Vite `manualChunks` config splits it into a separate `katex-*.js` chunk.

**Result:**

| | Before | After |
|---|---|---|
| `dist/` total | 5.2 MB | 1.9 MB |
| Release `.exe` | **10.3 MB** | **4.95 MB** |

---

### Mermaid rebuilt from scratch

Rather than leaving diagrams broken, we wrote `mermaid-lite.ts` — a ~300-line zero-dependency SVG renderer that covers the three diagram types that appear in real-world READMEs and docs:

**Flowchart / graph** (`graph TD`, `LR`, `TB`, `BT`, `RL`)
- Topological layered layout (Kahn's algorithm, handles cycles gracefully)
- Node shapes: `[rect]`, `(round)`, `{diamond}`, `([stadium])`
- Edge styles: `-->` solid, `---` open, `-.->` dashed, `==>` thick
- Edge labels, arrowheads, direction reversal for BT/RL

**sequenceDiagram**
- `participant` / `actor` declarations
- Solid (`->>`) and dashed (`-->>`) arrows
- Self-call loops
- `note over` blocks
- `loop` / `alt` / `opt` / `par` section dividers
- Actor boxes repeated at top and bottom

**pie**
- `"label": value` data entries
- Arc slices with percentage labels inside
- Right-side legend with value
- Optional `title` line

Everything renders as inline SVG directly from the markdown pipeline — synchronous, no DOM dependency, no lazy load needed. Uses the GwenLand dark palette throughout (`#1e1d1d` background, `#c28a64` primary accent). Unknown diagram types surface a styled SVG error banner instead of throwing.

Bundle cost: ~10 KB added to the main chunk (pure TS string construction, no imports).

---

## Files changed

| File | Change |
|------|--------|
| `frontend/ui/src/lib/preview/mermaid-lite.ts` | New — ~300-line from-scratch renderer |
| `frontend/ui/src/lib/preview/markdown.ts` | KaTeX lazy import; mermaid-lite wired in; `renderMarkdown` async |
| `frontend/ui/src/lib/components/MarkdownPreview.svelte` | Removed mermaid library dependency; lazy KaTeX CSS inject; mermaid-diagram CSS |
| `frontend/ui/src/lib/context-menu/ContextMenuPortal.svelte` | `--cm-bg` → dark, `--cm-item-hover` → solid primary, `--cm-radius` → `var(--radius)` |
| `frontend/ui/src/lib/context-menu/ContextMenuItem.svelte` | Item radius; danger color corrected for dark bg |
| `frontend/ui/package.json` | Removed `mermaid`; version `0.1.10` → `0.1.12` |
| `frontend/ui/pnpm-lock.yaml` | Lockfile updated |
| `frontend/ui/vite.config.ts` | `manualChunks` for katex lazy splitting |
| `frontend/tauri.conf.json` | Version `0.1.10` → `0.1.12` |
| `frontend/Cargo.toml` | Version `0.1.10` → `0.1.12` |
| `CHANGELOG.md` | `[Unreleased]` promoted to `[0.1.12]` |

## Test counts
- Frontend: **82 / 82** passed (9 suites)
- Engine: **455 / 455** passed
