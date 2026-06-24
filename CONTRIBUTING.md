# Contributing to GwenLand IDE

First off, thank you for considering contributing to GwenLand IDE! It's people like you that make GwenLand IDE such a great tool.

## Where do I go from here?

If you've noticed a bug or have a feature request, make sure to check our [Issues](../../issues) to see if someone has already created an issue for it. If not, go ahead and [create one](../../issues/new).

## Development Setup

1. **Prerequisites**
   - Rust toolchain (stable)
   - Node.js (v20+) and pnpm (v9+)
   - Tauri dependencies (see Tauri prerequisites for your OS)

2. **Setup**
   ```bash
   # Install JS dependencies
   cd frontend/ui
   pnpm install
   ```

3. **Running Locally**
   ```bash
   # Make sure you run Tauri dev from the frontend/ui or frontend root directory
   # depending on your setup. Typically, you can use Cargo directly:
   cargo run --bin gwenland
   ```
   *Note: SvelteKit should be running in dev mode. You can start it via `pnpm dev` inside `frontend/ui`.*

## Pull Request Process

1. Ensure any install or build dependencies are removed before the end of the layer when doing a build.
2. Update the `README.md` with details of any breaking changes, new environment variables, or new CLI arguments.
3. Make sure all CI checks pass (Rustfmt, Clippy, tests, and binary size limits).
4. You may merge the Pull Request in once you have the sign-off of at least one other developer.
