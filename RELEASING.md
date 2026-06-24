# Releasing GwenLand IDE

This project splits its pipeline in two:

- **CI runs on GitHub Actions** ([.github/workflows/ci.yml](.github/workflows/ci.yml)) — on every push to `main` and every PR it runs rustfmt, clippy (`-D warnings`), the full test suite, a release build, and a binary-size check.
- **CD runs locally** — the full Tauri bundle (MSI/NSIS installers) is produced on a developer machine with [scripts/release.ps1](scripts/release.ps1) (Windows) or [scripts/release.sh](scripts/release.sh) (macOS/Linux), then published to a GitHub Release.

## Why CD is local, not in GitHub Workflows

The `cargo tauri build` bundle step needs the platform's native installer toolchain (WiX/NSIS on Windows, etc.) and produces large signed artifacts. We keep that off hosted GitHub runners and run it locally instead. The tag-triggered workflows under `.github/workflows/*.yml` (e.g. `windows-x64.yml`) only ever uploaded the bare `gwenland.exe`, **not** the installers — so the canonical, distributable release is the one these scripts build.

> If you ever want to move CD onto your own hardware, point those workflows at a **self-hosted runner** (`runs-on: self-hosted`) and have it invoke the same `cargo tauri build`. The scripts here are the source of truth for the steps.

## Prerequisites (one-time)

| Tool | Install |
| --- | --- |
| Rust (stable) | <https://rustup.rs> |
| Tauri CLI v2 | `cargo install tauri-cli --version "^2"` |
| pnpm 9 | `npm i -g pnpm@9` |
| GitHub CLI (only for `--publish`) | <https://cli.github.com>, then `gh auth login` |

The scripts verify all of these and fail early with a clear message if one is missing.

## Cutting a release

1. **Bump the version** in [frontend/tauri.conf.json](frontend/tauri.conf.json) (`"version"`). The scripts require the requested version to match this file.
2. **Commit** the bump (the scripts refuse a dirty tree unless `--skip-checks`).
3. **Build the installers** (and optionally publish):

   Windows (PowerShell):
   ```powershell
   # Build only — installers land in target/release/bundle/{msi,nsis}/
   ./scripts/release.ps1

   # Build, tag vX.Y.Z, and create a GitHub Release with the installers attached
   ./scripts/release.ps1 -Version 0.1.0 -Publish
   ```

   macOS / Linux (bash):
   ```bash
   ./scripts/release.sh                       # build only
   ./scripts/release.sh --version 0.1.0 --publish
   ```

### What the scripts do

1. Verify the toolchain (`cargo`, `pnpm`, `cargo tauri`).
2. Guard: the git tree is clean and the target version matches `tauri.conf.json` (skip with `--skip-checks` / `-SkipChecks` for test builds).
3. Run `cargo tauri build` from `frontend/` — its `beforeBuildCommand` runs `pnpm build` first, then the bundle targets in `tauri.conf.json` (`msi`, `nsis`) are produced.
4. Sanity-check that the bare binary is under the **6.5 MB** budget (warns, does not fail).
5. With `--publish` / `-Publish`: create and push the `vX.Y.Z` tag, then `gh release create` with the installers and auto-generated notes.

## Output locations

- Installers: `target/release/bundle/msi/*.msi` and `target/release/bundle/nsis/*-setup.exe`
- Bare executable: `target/release/gwenland.exe` (Windows) / `target/release/gwenland`

## Publishing manually (without `gh`)

If you skip `--publish`, build locally and then on GitHub: create a release for tag `vX.Y.Z` and drag the files from `target/release/bundle/...` into it. Keep the tag name `vX.Y.Z` so it lines up with the existing `.github/workflows/*.yml` tag triggers.

## Release checklist

- [ ] `cargo test --workspace` green (CI also enforces this)
- [ ] Version bumped in `frontend/tauri.conf.json` and committed
- [ ] Changelog entry added under [changelog/](changelog/)
- [ ] `scripts/release.{ps1,sh}` run; installers produced under `target/release/bundle/`
- [ ] Binary under the 6.5 MB budget
- [ ] GitHub Release created (via `--publish` or manually) with installers attached
