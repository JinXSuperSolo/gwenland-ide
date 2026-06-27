# GwenLand IDE — Session Changes
**Date:** 2026-06-27 · **Commits:** 1630424 → e1becbc

---

## What changed this session

Two things: a serious LSP bug that was silently broken, and a round of rendering performance fixes.

---

### LSP diagnostics fixed — squiggles were never showing on Windows

The red underlines (and warnings/errors) in the editor were completely broken and nobody would have noticed just from looking at the UI — it failed silently every time.

The cause was a path separator mismatch. When the LSP server (rust-analyzer, tsserver, etc.) sends back a `publishDiagnostics` notification, the Rust backend decodes the `file://` URI and converts it to a plain path using forward slashes — `C:/Users/…`. The frontend file tree, on the other hand, gives files their paths with Windows backslashes — `C:\Users\…`. These were used as dictionary keys on both sides, and since `C:/foo/bar.ts` ≠ `C:\foo\bar.ts` as a string, the lookup always missed and the editor cleared any squiggles rather than showing them.

Fixed by adding a `normPath()` helper in `lsp.ts` that folds every backslash to a forward slash. It's now applied at every point a path is written to or read from the diagnostics store, so both sides always agree on the key.

---

### Rendering performance — second pass

After the first perf fix landed, a second audit found more hot paths:

**AI composer — triple-fired on every keystroke.** The `@mention` search (`refreshMentions`) was wired to `oninput`, `onkeyup`, AND `onclick` — meaning every keystroke triggered it at least twice in a row, and each call hit an async workspace file index scan. Removed the redundant `onkeyup` handler and debounced the remaining calls to 80ms so rapid typing coalesces into a single lookup.

**File tree folder badges — O(n) scan per node per render.** Each folder node in the tree was checking whether any of its children were dirty by calling `state.files.some(f => f.path.startsWith(prefix))` on every render. With a repo that has 50 dirty files and 30 expanded folders, that's 1500 comparisons per git poll. Fixed by precomputing a `Set<string>` of dirty directory prefixes once per git update in the store — folder nodes now do a single `Set.has()` call.

**Terminal minimap CSS token.** Same `getComputedStyle` on every frame issue that was fixed in the editor minimap last session — just wasn't caught in the first pass. Now cached after the first draw.

---

## Files changed

| File | Change |
|------|--------|
| `frontend/ui/src/lib/stores/lsp.ts` | `normPath()` helper; applied at all store key reads/writes |
| `frontend/ui/src/lib/components/Editor.svelte` | `applyDiagnosticsToView` uses `normPath(mountedPath)` for lookup |
| `frontend/ui/src/lib/components/AiPanel.svelte` | Removed redundant `onkeyup`; debounced `refreshMentions` to 80ms |
| `frontend/ui/src/lib/stores/git.ts` | `gitDirtyPrefixes` Set precomputed on each git update |
| `frontend/ui/src/lib/components/TreeNode.svelte` | Folder dirty check: `Set.has()` instead of `files.some()` |
| `frontend/ui/src/lib/components/TerminalInstance.svelte` | Cache `--primary` CSS token after first minimap draw |
