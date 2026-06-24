#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

# Bundle the CM6 entry into a single committed IIFE on the GwenEditorBundle global.
npx esbuild entry.js --bundle --minify --outfile=dist/codemirror.bundle.js --format=iife --global-name=GwenEditorBundle

# The Tauri webview only serves files inside frontend/dist (frontendDist), so the
# served index.html loads the bundle from frontend/dist/editor/. Copy it there.
# Both the source-of-truth (editor/dist) and the served copy are committed.
mkdir -p ../dist/editor
cp dist/codemirror.bundle.js ../dist/editor/codemirror.bundle.js
