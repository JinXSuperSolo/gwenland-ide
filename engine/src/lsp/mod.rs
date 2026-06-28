//! LSP Bridge (Milestone 6).
//!
//! A single generic Language Server Protocol client for Rust, TypeScript/
//! JavaScript, and Python. Like [`crate::ai`], this module — and everything
//! under it — contains ZERO `tauri::` imports (Requirement 1.2); all Tauri
//! command/event plumbing lives in `frontend/src/main.rs`.
//!
//! # Wave 1 research decisions (locked)
//!
//! **Protocol types & transport (task 1.1).** We do *not* depend on `lsp-types`
//! or `tower-lsp`. `tower-lsp` is server-oriented (it builds a *server*'s tower
//! service stack) and would pull a large async dependency tree for no client
//! benefit. `lsp-types` is client-usable but large and churny, and M6 only
//! touches a handful of messages. Instead we hand-roll a tiny transport in
//! [`json_rpc`] (Content-Length framing + JSON-RPC envelopes over `serde_json`)
//! and define our own stable, UI-facing DTOs in [`diagnostics`] and
//! [`completion`]. This keeps the release binary small (`opt-level = "z"`,
//! Requirement 3.1/3.2) and gives full control over the protocol surface.
//!
//! No new crates are added to `engine/Cargo.toml` (task 1.2): `serde`/
//! `serde_json` already exist, the stdio process uses `std::process`, and
//! `PATH`/`PATHEXT` resolution is implemented in [`process`] without the `which`
//! crate. (Rust's std runs `.cmd`/`.bat` shims via `cmd.exe` automatically since
//! 1.77.2, which covers npm-installed servers on Windows.)
//!
//! **Transport runtime.** The client uses a dedicated std reader thread plus
//! `std::sync::mpsc` for request/response correlation (mirroring the existing
//! PTY reader-thread pattern in [`crate::terminal`]). No async runtime is needed
//! inside the LSP module; completion uses `recv_timeout` for its fail-soft
//! deadline.
//!
//! **Document sync (task 1.1 / Requirement 9.5).** M6 uses **full-document
//! sync**: the client advertises `TextDocumentSyncKind::Full` (1) and sends the
//! complete document text on every `didChange`. This is simpler to get correct
//! against three different servers and is acceptable for a foundation milestone.
//! The UI debounces changes (Requirement 9.8). Future incremental sync is left
//! open: switch the advertised sync kind and send range deltas in
//! [`client`]/`didChange` — no other module shape needs to change.
//!
//! **CodeMirror packages (task 1.1).** Diagnostics need `@codemirror/lint` and
//! completion needs `@codemirror/autocomplete`; both are added to
//! `frontend/ui/package.json` in Waves 4 and 5 respectively.

pub mod client;
pub mod completion;
pub mod config;
pub mod definition;
pub mod diagnostics;
pub mod error;
pub mod hover;
pub mod json_rpc;
pub mod language;
pub mod process;
pub mod root;

pub use client::{
    DiagnosticsCallback, DiagnosticsUpdate, LspClient, LspManager, LspStatus, StatusCallback,
    StatusUpdate,
};
pub use completion::LspCompletionOption;
pub use config::{LanguageServerSettings, LspSettings};
pub use definition::LspDefinitionLocation;
pub use diagnostics::{DiagnosticSeverity, LspDiagnostic, LspRange};
pub use error::LspError;
pub use hover::LspHover;
pub use language::LanguageId;
pub use process::resolve_command;
