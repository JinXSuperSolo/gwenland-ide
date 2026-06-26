//! Generic LSP client and manager (Milestone 6).
//!
//! [`LspClient`] owns one [`ServerProcess`] and drives the initialize handshake,
//! status tracking, and (in later waves) document sync + completion. The
//! language-agnostic [`LspManager`] owns the client map keyed by
//! `(server_key, root)` — TypeScript and JavaScript share one client per root
//! (Requirement 2.7) — plus a uri→client index for document routing.
//!
//! Diagnostics and status reach the Tauri layer through two callbacks supplied
//! at construction (the same `Box<dyn Fn>` push pattern the PTY uses), so this
//! module stays free of any `tauri::` import.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};

use super::completion::{LspCompletionOption, normalize_completion};
use super::config::LspSettings;
use super::definition::{LspDefinitionLocation, normalize_definition};
use super::diagnostics::{DiagnosticsStore, LspDiagnostic, parse_publish_diagnostics};
use super::error::LspError;
use super::language::LanguageId;
use super::process::{ServerProcess, resolve_command};
use super::root::{detect_root, file_uri_to_path, path_to_file_uri};

/// `initialize` must respond within this window. Servers send the response
/// before any background indexing, so this is generous, not tight.
const INIT_TIMEOUT: Duration = Duration::from_secs(15);
/// Graceful-shutdown budget before the child is force-killed.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);
/// Completion deadline — fail-soft so typing never freezes (Requirement 11.9).
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(2);
const DEFINITION_TIMEOUT: Duration = Duration::from_secs(2);

/// Per-file/per-language LSP state shown in the editor (Requirement 12.2). The
/// serde tag is `state` so the UI can discriminate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum LspStatus {
    /// No LSP involvement (e.g. before any detection).
    PlainText,
    /// The file extension has no M6 language mapping.
    UnsupportedLanguage,
    /// The language is disabled in settings.
    Disabled { language: LanguageId },
    /// The configured command could not be found.
    MissingServer {
        language: LanguageId,
        command: String,
    },
    /// The server is starting / not yet connected.
    Starting { language: LanguageId },
    /// Connected after a successful initialize handshake.
    Connected {
        language: LanguageId,
        server_name: Option<String>,
    },
    /// The server process exited unexpectedly.
    Crashed {
        language: LanguageId,
        message: String,
    },
}

/// Payload pushed to the Tauri layer on every status transition.
#[derive(Debug, Clone, Serialize)]
pub struct StatusUpdate {
    pub language: Option<LanguageId>,
    pub workspace_root: Option<String>,
    #[serde(flatten)]
    pub status: LspStatus,
}

/// Payload pushed to the Tauri layer when diagnostics change for a document.
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticsUpdate {
    pub uri: String,
    pub path: String,
    pub language: LanguageId,
    pub workspace_root: String,
    pub diagnostics: Vec<LspDiagnostic>,
}

/// Callback the manager invokes when a server publishes diagnostics.
pub type DiagnosticsCallback = Arc<dyn Fn(DiagnosticsUpdate) + Send + Sync>;
/// Callback the manager invokes on every status transition.
pub type StatusCallback = Arc<dyn Fn(StatusUpdate) + Send + Sync>;

/// `(server_key, root)` — the identity of one server process.
type ClientKey = (String, PathBuf);

/// State shared between the reader-thread notification handler and the client's
/// document methods: the set of open documents (with last version) and the
/// latest diagnostics per URI. Sharing lets the handler drop diagnostics that
/// arrive for a closed document (Requirement 5.4) without a back-reference into
/// [`LspClient`].
struct ClientShared {
    /// open uri → last version (presence == document is open).
    versions: Mutex<HashMap<String, i32>>,
    diagnostics: Mutex<DiagnosticsStore>,
}

/// One connected (or connecting) language server.
pub struct LspClient {
    process: ServerProcess,
    /// Shared so the process's unexpected-exit handler can flip it to `Crashed`.
    status: Arc<Mutex<LspStatus>>,
    shared: Arc<ClientShared>,
}

impl LspClient {
    /// Spawn the server, run `initialize`/`initialized`, and return a connected
    /// client. Diagnostics notifications are routed to `on_diagnostics`; an
    /// unexpected process exit flips status to `Crashed` and notifies
    /// `on_status`.
    fn connect(
        language: LanguageId,
        root: PathBuf,
        program: &Path,
        args: &[String],
        on_diagnostics: DiagnosticsCallback,
        on_status: StatusCallback,
    ) -> Result<LspClient, LspError> {
        let status = Arc::new(Mutex::new(LspStatus::Starting { language }));
        let shared = Arc::new(ClientShared {
            versions: Mutex::new(HashMap::new()),
            diagnostics: Mutex::new(DiagnosticsStore::new()),
        });

        // Notification router (reader thread). Diagnostics are forwarded (and
        // stored) only for still-open documents; window/logMessage,
        // window/showMessage, and $/progress are ignored in M6 (Requirement 7.6).
        let notif = {
            let on_diagnostics = on_diagnostics.clone();
            let root_str = root.to_string_lossy().into_owned();
            let shared = shared.clone();
            Box::new(move |method: String, params: Value| {
                if method != "textDocument/publishDiagnostics" {
                    return;
                }
                let Some((uri, diagnostics)) = parse_publish_diagnostics(&params) else {
                    return;
                };
                // Drop diagnostics for a document that is not open (closed or
                // never opened) so stale markers never reach the UI.
                if !shared
                    .versions
                    .lock()
                    .map(|v| v.contains_key(&uri))
                    .unwrap_or(false)
                {
                    return;
                }
                if let Ok(mut store) = shared.diagnostics.lock() {
                    store.set(uri.clone(), diagnostics.clone());
                }
                let path = file_uri_to_path(&uri)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| uri.clone());
                on_diagnostics(DiagnosticsUpdate {
                    uri,
                    path,
                    language,
                    workspace_root: root_str.clone(),
                    diagnostics,
                });
            })
        };

        // Unexpected-exit handler (reader thread): mark crashed + notify.
        let on_exit = {
            let status = status.clone();
            let on_status = on_status.clone();
            let root_str = root.to_string_lossy().into_owned();
            Box::new(move || {
                let crashed = LspStatus::Crashed {
                    language,
                    message: "language server exited unexpectedly".into(),
                };
                if let Ok(mut s) = status.lock() {
                    *s = crashed.clone();
                }
                on_status(StatusUpdate {
                    language: Some(language),
                    workspace_root: Some(root_str.clone()),
                    status: crashed,
                });
            })
        };

        let process = ServerProcess::spawn(program, args, &root, notif, on_exit)?;

        // initialize → store serverInfo.name → initialized.
        let init_result = process.request("initialize", initialize_params(&root), INIT_TIMEOUT)?;
        let server_name = init_result
            .get("serverInfo")
            .and_then(|i| i.get("name"))
            .and_then(Value::as_str)
            .map(str::to_string);
        process.notify("initialized", json!({}))?;

        if let Ok(mut s) = status.lock() {
            *s = LspStatus::Connected {
                language,
                server_name,
            };
        }

        Ok(LspClient {
            process,
            status,
            shared,
        })
    }

    pub fn status(&self) -> LspStatus {
        self.status
            .lock()
            .map(|s| s.clone())
            .unwrap_or(LspStatus::PlainText)
    }

    pub fn is_alive(&mut self) -> bool {
        self.process.is_alive()
    }

    /// Send `textDocument/didOpen` with the full document text (Requirement
    /// 8.3/9.1/9.2). `language_id` is the *file's* LSP id (so a `.js` file on the
    /// shared TypeScript server still opens as `javascript`).
    fn did_open(
        &self,
        uri: &str,
        language_id: &str,
        version: i32,
        text: &str,
    ) -> Result<(), LspError> {
        if let Ok(mut v) = self.shared.versions.lock() {
            v.insert(uri.to_string(), version);
        }
        self.process.notify(
            "textDocument/didOpen",
            did_open_params(uri, language_id, version, text),
        )
    }

    /// Send `textDocument/didChange` with the full new text (full-document sync,
    /// Requirement 9.3/9.6). Stale or out-of-order versions are dropped to keep
    /// the per-document version monotonic (Requirement 9.4).
    fn did_change(&self, uri: &str, version: i32, text: &str) -> Result<(), LspError> {
        {
            let mut v = self
                .shared
                .versions
                .lock()
                .map_err(|_| LspError::transport("versions poisoned"))?;
            // Only sync changes for an open document with a newer version.
            match v.get(uri) {
                None => return Ok(()), // not open: no-op
                Some(prev) if !is_newer_version(Some(*prev), version) => return Ok(()),
                _ => {}
            }
            v.insert(uri.to_string(), version);
        }
        self.process.notify(
            "textDocument/didChange",
            did_change_params(uri, version, text),
        )
    }

    /// Send `textDocument/didClose`, forget the document version, and clear its
    /// diagnostics (Requirement 8.5/9.9/10.10).
    fn did_close(&self, uri: &str) -> Result<(), LspError> {
        let was_open = self
            .shared
            .versions
            .lock()
            .map(|mut v| v.remove(uri).is_some())
            .unwrap_or(false);
        if let Ok(mut store) = self.shared.diagnostics.lock() {
            store.clear_uri(uri);
        }
        if was_open {
            self.process
                .notify("textDocument/didClose", did_close_params(uri))
        } else {
            Ok(())
        }
    }

    /// Request completions at a position. Requires a connected server and an
    /// open document (Requirement 11.3); otherwise returns no options. A timeout
    /// or transport error surfaces as `Err` (the manager maps it to empty).
    fn completion(
        &self,
        uri: &str,
        line: u32,
        character: u32,
        timeout: Duration,
    ) -> Result<Vec<LspCompletionOption>, LspError> {
        if !matches!(self.status(), LspStatus::Connected { .. }) {
            return Err(LspError::NotInitialized);
        }
        let is_open = self
            .shared
            .versions
            .lock()
            .map(|v| v.contains_key(uri))
            .unwrap_or(false);
        if !is_open {
            return Ok(Vec::new());
        }
        let params = json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character },
        });
        let result = self
            .process
            .request("textDocument/completion", params, timeout)?;
        Ok(normalize_completion(&result))
    }

    fn definition(
        &self,
        uri: &str,
        line: u32,
        character: u32,
        timeout: Duration,
    ) -> Result<Option<LspDefinitionLocation>, LspError> {
        if !matches!(self.status(), LspStatus::Connected { .. }) {
            return Err(LspError::NotInitialized);
        }
        let is_open = self
            .shared
            .versions
            .lock()
            .map(|v| v.contains_key(uri))
            .unwrap_or(false);
        if !is_open {
            return Ok(None);
        }
        let params = json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character },
        });
        let result = self
            .process
            .request("textDocument/definition", params, timeout)?;
        Ok(normalize_definition(&result))
    }

    fn shutdown(&mut self) {
        self.process.shutdown(SHUTDOWN_TIMEOUT);
    }
}

/// Returns true if `next` monotonically advances past `prev` (Requirement 9.4).
fn is_newer_version(prev: Option<i32>, next: i32) -> bool {
    match prev {
        Some(p) => next > p,
        None => true,
    }
}

/// `textDocument/didOpen` params with full text.
fn did_open_params(uri: &str, language_id: &str, version: i32, text: &str) -> Value {
    json!({
        "textDocument": {
            "uri": uri,
            "languageId": language_id,
            "version": version,
            "text": text,
        }
    })
}

/// `textDocument/didChange` params — full-document sync sends the entire new
/// text as a single content change with no range.
fn did_change_params(uri: &str, version: i32, text: &str) -> Value {
    json!({
        "textDocument": { "uri": uri, "version": version },
        "contentChanges": [ { "text": text } ],
    })
}

/// `textDocument/didClose` params.
fn did_close_params(uri: &str) -> Value {
    json!({ "textDocument": { "uri": uri } })
}

/// Build LSP `initialize` params (Requirement 6.3/8.1). Advertises full-document
/// sync intent via client capabilities and disables snippet support so servers
/// return plain insert text (matches the CodeMirror integration choice).
fn initialize_params(root: &Path) -> Value {
    let root_uri = path_to_file_uri(root);
    let name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace")
        .to_string();
    json!({
        "processId": std::process::id(),
        "clientInfo": { "name": "GwenLand IDE", "version": env!("CARGO_PKG_VERSION") },
        "rootPath": root.to_string_lossy(),
        "rootUri": root_uri,
        "capabilities": {
            "textDocument": {
                "synchronization": { "dynamicRegistration": false, "didSave": false },
                "publishDiagnostics": { "relatedInformation": false },
                "completion": {
                    "dynamicRegistration": false,
                    "completionItem": {
                        "snippetSupport": false,
                        "documentationFormat": ["plaintext"]
                    }
                }
            },
            "workspace": { "workspaceFolders": true }
        },
        "workspaceFolders": [ { "uri": root_uri, "name": name } ],
    })
}

/// Owns every live [`LspClient`] and routes commands by language/path. Held as
/// Tauri managed state by the frontend, which supplies the diagnostics/status
/// callbacks (event emitters).
pub struct LspManager {
    clients: Mutex<HashMap<ClientKey, LspClient>>,
    /// uri → owning client key, so change/close/completion route without
    /// re-detecting the root (populated by document sync in Wave 3).
    doc_index: Mutex<HashMap<String, ClientKey>>,
    on_diagnostics: DiagnosticsCallback,
    on_status: StatusCallback,
}

impl LspManager {
    pub fn new(on_diagnostics: DiagnosticsCallback, on_status: StatusCallback) -> Self {
        Self {
            clients: Mutex::new(HashMap::new()),
            doc_index: Mutex::new(HashMap::new()),
            on_diagnostics,
            on_status,
        }
    }

    fn emit_status(&self, language: Option<LanguageId>, root: Option<&Path>, status: LspStatus) {
        (self.on_status)(StatusUpdate {
            language,
            workspace_root: root.map(|r| r.to_string_lossy().into_owned()),
            status,
        });
    }

    /// Ensure a connected client exists for `path`, spawning + initializing on
    /// demand. Returns the resulting status; missing/disabled/unsupported are
    /// non-blocking states, not errors (Requirement 12.x). Reused by document
    /// open in Wave 3.
    pub fn ensure_client(
        &self,
        path: &Path,
        workspace_root: Option<&Path>,
        settings: &LspSettings,
    ) -> LspStatus {
        let Some(language) = LanguageId::from_path(path) else {
            return LspStatus::UnsupportedLanguage;
        };
        let ls = settings.for_language(language);
        if !ls.enabled {
            let st = LspStatus::Disabled { language };
            self.emit_status(Some(language), None, st.clone());
            return st;
        }
        let server_key = language.server_key();
        let (command, args) = ls.effective(server_key);
        let Some(program) = resolve_command(&command) else {
            let st = LspStatus::MissingServer { language, command };
            self.emit_status(Some(language), None, st.clone());
            return st;
        };

        let root = detect_root(path, language, workspace_root);
        let key = (server_key.to_string(), root.clone());

        // Reuse a live client; drop a dead one so it respawns below.
        {
            let mut clients = self.clients.lock().expect("clients poisoned");
            if let Some(client) = clients.get_mut(&key) {
                if client.is_alive() {
                    return client.status();
                }
                clients.remove(&key);
            }
        }

        self.emit_status(
            Some(language),
            Some(&root),
            LspStatus::Starting { language },
        );

        match LspClient::connect(
            language,
            root.clone(),
            &program,
            &args,
            self.on_diagnostics.clone(),
            self.on_status.clone(),
        ) {
            Ok(client) => {
                let st = client.status();
                self.clients
                    .lock()
                    .expect("clients poisoned")
                    .insert(key, client);
                self.emit_status(Some(language), Some(&root), st.clone());
                st
            }
            Err(e) => {
                let st = LspStatus::Crashed {
                    language,
                    message: e.to_string(),
                };
                self.emit_status(Some(language), Some(&root), st.clone());
                st
            }
        }
    }

    /// Open `path` for LSP: ensure (spawn/initialize) the client, then send
    /// `textDocument/didOpen` with the full text and register the document for
    /// routing. Unsupported/disabled/missing/crashed states return without
    /// opening (Requirement 8.3/9.1/12.5/12.6).
    pub fn open_document(
        &self,
        path: &Path,
        text: &str,
        version: i32,
        workspace_root: Option<&Path>,
        settings: &LspSettings,
    ) -> LspStatus {
        let status = self.ensure_client(path, workspace_root, settings);
        if !matches!(status, LspStatus::Connected { .. }) {
            return status;
        }
        // Connected implies a supported language.
        let Some(language) = LanguageId::from_path(path) else {
            return status;
        };
        let root = detect_root(path, language, workspace_root);
        let key = (language.server_key().to_string(), root);
        let uri = path_to_file_uri(path);

        {
            let mut clients = self.clients.lock().expect("clients poisoned");
            if let Some(client) = clients.get_mut(&key) {
                let _ = client.did_open(&uri, language.as_lsp_language_id(), version, text);
            }
        }
        self.doc_index
            .lock()
            .expect("doc_index poisoned")
            .insert(uri, key);
        status
    }

    /// Push a document change (full text) to its server. No-op when the document
    /// is not open or its server is gone — typing must never fail (Req 9.3/9.8).
    pub fn change_document(&self, path: &Path, text: &str, version: i32) -> Result<(), LspError> {
        let uri = path_to_file_uri(path);
        let key = self
            .doc_index
            .lock()
            .expect("doc_index poisoned")
            .get(&uri)
            .cloned();
        if let Some(key) = key {
            let mut clients = self.clients.lock().expect("clients poisoned");
            if let Some(client) = clients.get_mut(&key) {
                return client.did_change(&uri, version, text);
            }
        }
        Ok(())
    }

    /// Close an LSP-backed document: send `didClose`, drop its diagnostics, and
    /// forget its routing entry (Requirement 8.5/9.9/10.10).
    pub fn close_document(&self, path: &Path) -> Result<(), LspError> {
        let uri = path_to_file_uri(path);
        let key = self
            .doc_index
            .lock()
            .expect("doc_index poisoned")
            .remove(&uri);
        if let Some(key) = key {
            let mut clients = self.clients.lock().expect("clients poisoned");
            if let Some(client) = clients.get_mut(&key) {
                return client.did_close(&uri);
            }
        }
        Ok(())
    }

    /// Request completions for `path` at `line`/`character` (zero-based, UTF-16).
    /// Fail-soft: returns no options when the document is not LSP-backed, the
    /// server is missing/crashed, or the request times out (Requirement 11.9).
    pub fn completion(&self, path: &Path, line: u32, character: u32) -> Vec<LspCompletionOption> {
        let uri = path_to_file_uri(path);
        let key = self
            .doc_index
            .lock()
            .expect("doc_index poisoned")
            .get(&uri)
            .cloned();
        let Some(key) = key else {
            return Vec::new();
        };
        let mut clients = self.clients.lock().expect("clients poisoned");
        let Some(client) = clients.get_mut(&key) else {
            return Vec::new();
        };
        client
            .completion(&uri, line, character, COMPLETION_TIMEOUT)
            .unwrap_or_default()
    }

    /// Request the first definition location for `path` at `line`/`character`.
    /// Fail-soft: missing servers, unsupported files, and timeouts return None.
    pub fn definition(
        &self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Option<LspDefinitionLocation> {
        let uri = path_to_file_uri(path);
        let key = self
            .doc_index
            .lock()
            .expect("doc_index poisoned")
            .get(&uri)
            .cloned()?;
        let mut clients = self.clients.lock().expect("clients poisoned");
        let client = clients.get_mut(&key)?;
        client
            .definition(&uri, line, character, DEFINITION_TIMEOUT)
            .unwrap_or_default()
    }

    /// Current status for a file without spawning a server. Prefers the live
    /// client of an open document; otherwise reports the pre-flight state.
    pub fn status_for_path(&self, path: &Path, settings: &LspSettings) -> LspStatus {
        let Some(language) = LanguageId::from_path(path) else {
            return LspStatus::UnsupportedLanguage;
        };

        // Live status for an already-open document.
        let uri = path_to_file_uri(path);
        let key_opt = self
            .doc_index
            .lock()
            .expect("doc_index poisoned")
            .get(&uri)
            .cloned();
        if let Some(key) = key_opt {
            let live = {
                let mut clients = self.clients.lock().expect("clients poisoned");
                match clients.get_mut(&key) {
                    Some(c) => {
                        if c.is_alive() {
                            Some(c.status())
                        } else {
                            None
                        }
                    }
                    None => None,
                }
            };
            if let Some(status) = live {
                return status;
            }
        }

        // Pre-flight (no spawn).
        let ls = settings.for_language(language);
        if !ls.enabled {
            return LspStatus::Disabled { language };
        }
        let (command, _) = ls.effective(language.server_key());
        if resolve_command(&command).is_none() {
            return LspStatus::MissingServer { language, command };
        }
        LspStatus::Starting { language }
    }

    /// Manually restart all clients for a server bucket (`"rust"`,
    /// `"typescript"`, `"python"`). Tears the old clients down and clears their
    /// document index; the UI re-opens active documents to reconnect. Never
    /// loops automatically (Requirement 6.6/6.7).
    pub fn restart(&self, server_key: &str, settings: &LspSettings) -> LspStatus {
        // Tear down matching clients.
        {
            let mut clients = self.clients.lock().expect("clients poisoned");
            let keys: Vec<ClientKey> = clients
                .keys()
                .filter(|(k, _)| k == server_key)
                .cloned()
                .collect();
            for k in keys {
                if let Some(mut c) = clients.remove(&k) {
                    c.shutdown();
                }
            }
        }
        // Forget those documents (separate lock scope: no nested locking).
        {
            let mut idx = self.doc_index.lock().expect("doc_index poisoned");
            idx.retain(|_, key| key.0 != server_key);
        }

        // Report a fresh pre-flight status for the language.
        let language = LanguageId::from_server_key(server_key).unwrap_or(LanguageId::Rust);
        let st = match settings.for_language(language) {
            ls if !ls.enabled => LspStatus::Disabled { language },
            ls => {
                let (command, _) = ls.effective(server_key);
                if resolve_command(&command).is_none() {
                    LspStatus::MissingServer { language, command }
                } else {
                    LspStatus::Starting { language }
                }
            }
        };
        self.emit_status(Some(language), None, st.clone());
        st
    }

    /// Gracefully stop every client (IDE shutdown / workspace close,
    /// Requirement 6.8/6.10).
    pub fn shutdown_all(&self) {
        let mut clients = self.clients.lock().expect("clients poisoned");
        for (_, mut client) in clients.drain() {
            client.shutdown();
        }
        self.doc_index.lock().expect("doc_index poisoned").clear();
    }

    /// Number of live clients (test/inspection helper).
    pub fn client_count(&self) -> usize {
        self.clients.lock().expect("clients poisoned").len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn noop_manager() -> LspManager {
        LspManager::new(Arc::new(|_| {}), Arc::new(|_| {}))
    }

    /// Build a manager that counts status emissions, for transition assertions.
    fn counting_manager() -> (LspManager, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        let mgr = LspManager::new(
            Arc::new(|_| {}),
            Arc::new(move |_| {
                c.fetch_add(1, Ordering::SeqCst);
            }),
        );
        (mgr, count)
    }

    #[test]
    fn unsupported_file_is_unsupported_language() {
        let mgr = noop_manager();
        let st = mgr.status_for_path(Path::new("notes.txt"), &LspSettings::default());
        assert_eq!(st, LspStatus::UnsupportedLanguage);
    }

    #[test]
    fn disabled_language_reports_disabled() {
        let mgr = noop_manager();
        let mut settings = LspSettings::default();
        settings.rust.enabled = false;
        let st = mgr.status_for_path(Path::new("src/main.rs"), &settings);
        assert_eq!(
            st,
            LspStatus::Disabled {
                language: LanguageId::Rust
            }
        );
    }

    #[test]
    fn missing_command_reports_missing_server() {
        let mgr = noop_manager();
        let mut settings = LspSettings::default();
        settings.python.command = "definitely-not-a-real-pyls".into();
        let st = mgr.status_for_path(Path::new("app.py"), &settings);
        match st {
            LspStatus::MissingServer { language, command } => {
                assert_eq!(language, LanguageId::Python);
                assert_eq!(command, "definitely-not-a-real-pyls");
            }
            other => panic!("expected MissingServer, got {other:?}"),
        }
    }

    #[test]
    fn ensure_client_disabled_emits_status_and_makes_no_client() {
        let (mgr, count) = counting_manager();
        let mut settings = LspSettings::default();
        settings.typescript.enabled = false;
        let st = mgr.ensure_client(Path::new("app.ts"), None, &settings);
        assert_eq!(
            st,
            LspStatus::Disabled {
                language: LanguageId::TypeScript
            }
        );
        assert_eq!(mgr.client_count(), 0);
        assert!(count.load(Ordering::SeqCst) >= 1, "a status was emitted");
    }

    #[test]
    fn ensure_client_missing_emits_status_and_makes_no_client() {
        let (mgr, _count) = counting_manager();
        let mut settings = LspSettings::default();
        settings.rust.command = "no-such-rust-analyzer-xyz".into();
        let st = mgr.ensure_client(Path::new("src/lib.rs"), None, &settings);
        assert!(matches!(st, LspStatus::MissingServer { .. }));
        assert_eq!(mgr.client_count(), 0);
    }

    #[test]
    fn restart_with_missing_command_returns_fresh_missing_status() {
        // Restart on an empty manager "creates a new attempt": it recomputes
        // pre-flight status. With a bogus command that is MissingServer.
        let mgr = noop_manager();
        let mut settings = LspSettings::default();
        settings.rust.command = "no-such-server".into();
        let st = mgr.restart("rust", &settings);
        assert!(matches!(st, LspStatus::MissingServer { .. }));
        assert_eq!(mgr.client_count(), 0);
    }

    #[test]
    fn restart_disabled_returns_disabled() {
        let mgr = noop_manager();
        let mut settings = LspSettings::default();
        settings.python.enabled = false;
        let st = mgr.restart("python", &settings);
        assert_eq!(
            st,
            LspStatus::Disabled {
                language: LanguageId::Python
            }
        );
    }

    #[test]
    fn status_serializes_with_state_tag() {
        let v = serde_json::to_value(LspStatus::Connected {
            language: LanguageId::Rust,
            server_name: Some("rust-analyzer".into()),
        })
        .unwrap();
        assert_eq!(v["state"], "connected");
        assert_eq!(v["language"], "rust");
        assert_eq!(v["server_name"], "rust-analyzer");

        let v = serde_json::to_value(LspStatus::MissingServer {
            language: LanguageId::Python,
            command: "pyright-langserver".into(),
        })
        .unwrap();
        assert_eq!(v["state"], "missing_server");
        assert_eq!(v["command"], "pyright-langserver");
    }

    #[test]
    fn status_update_flattens_status_fields() {
        let upd = StatusUpdate {
            language: Some(LanguageId::Rust),
            workspace_root: Some("/proj".into()),
            status: LspStatus::Starting {
                language: LanguageId::Rust,
            },
        };
        let v = serde_json::to_value(&upd).unwrap();
        assert_eq!(v["state"], "starting");
        assert_eq!(v["workspace_root"], "/proj");
    }

    // --- Document sync: version tracking + payload construction (task 5.7) ---

    #[test]
    fn version_is_monotonic() {
        assert!(is_newer_version(None, 1));
        assert!(is_newer_version(Some(1), 2));
        assert!(!is_newer_version(Some(2), 2)); // equal is not newer
        assert!(!is_newer_version(Some(5), 3)); // older dropped
    }

    #[test]
    fn did_open_payload_has_uri_language_version_text() {
        let p = did_open_params("file:///x.rs", "rust", 1, "fn main() {}");
        assert_eq!(p["textDocument"]["uri"], "file:///x.rs");
        assert_eq!(p["textDocument"]["languageId"], "rust");
        assert_eq!(p["textDocument"]["version"], 1);
        assert_eq!(p["textDocument"]["text"], "fn main() {}");
    }

    #[test]
    fn did_change_payload_is_full_document() {
        let p = did_change_params("file:///x.ts", 4, "const a = 1");
        assert_eq!(p["textDocument"]["uri"], "file:///x.ts");
        assert_eq!(p["textDocument"]["version"], 4);
        // Full-document sync: exactly one change, whole text, no range.
        let changes = p["contentChanges"].as_array().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0]["text"], "const a = 1");
        assert!(changes[0].get("range").is_none());
    }

    #[test]
    fn did_close_payload_has_uri_only() {
        let p = did_close_params("file:///x.py");
        assert_eq!(p["textDocument"]["uri"], "file:///x.py");
        assert_eq!(p["textDocument"].as_object().unwrap().len(), 1);
    }

    #[test]
    fn change_and_close_unopened_document_are_noops() {
        let mgr = noop_manager();
        // No document opened, no client: these must succeed without error.
        assert!(
            mgr.change_document(Path::new("src/main.rs"), "x", 2)
                .is_ok()
        );
        assert!(mgr.close_document(Path::new("src/main.rs")).is_ok());
    }

    #[test]
    fn completion_for_unopened_document_is_empty() {
        // No client / not opened: fail-soft to no options (Requirement 11.9).
        let mgr = noop_manager();
        assert!(mgr.completion(Path::new("src/main.rs"), 0, 0).is_empty());
        assert!(mgr.completion(Path::new("notes.txt"), 0, 0).is_empty());
    }

    #[test]
    fn definition_for_unopened_document_is_none() {
        let mgr = noop_manager();
        assert_eq!(mgr.definition(Path::new("src/main.rs"), 0, 0), None);
    }

    #[test]
    fn open_unsupported_document_returns_unsupported_without_client() {
        let mgr = noop_manager();
        let st = mgr.open_document(
            Path::new("notes.txt"),
            "hello",
            1,
            None,
            &LspSettings::default(),
        );
        assert_eq!(st, LspStatus::UnsupportedLanguage);
        assert_eq!(mgr.client_count(), 0);
    }

    // --- Gated real-server smoke checks (Requirement 14.7) -------------------
    // These run only when a server is installed *and functional* and skip
    // cleanly otherwise. A "present" command can still be non-functional (e.g.
    // a rustup proxy whose component is not installed), so we assert only that
    // resolution yields a spawnable executable, and log the --version result.

    fn smoke_version(command: &str) {
        let Some(program) = resolve_command(command) else {
            eprintln!("[smoke] skip: {command} not installed");
            return;
        };
        // Detection must return something we can actually spawn.
        let spawned = std::process::Command::new(&program)
            .arg("--version")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        match spawned {
            Ok(s) => eprintln!(
                "[smoke] {command} -> {} (--version exit: {:?})",
                program.display(),
                s.code()
            ),
            Err(e) => panic!("[smoke] {command} resolved to {program:?} but failed to spawn: {e}"),
        }
    }

    #[test]
    fn smoke_rust_analyzer_version() {
        smoke_version("rust-analyzer");
    }

    #[test]
    fn smoke_typescript_language_server_version() {
        smoke_version("typescript-language-server");
    }

    #[test]
    fn smoke_pyright_langserver_version() {
        smoke_version("pyright-langserver");
    }

    #[test]
    fn smoke_pylsp_version() {
        smoke_version("pylsp");
    }

    /// Full connect → restart cycle against rust-analyzer when it is installed
    /// *and* functional. Skips cleanly if absent or if the binary does not
    /// connect (e.g. a rustup proxy without the component installed).
    #[test]
    fn smoke_connect_rust_analyzer_when_present() {
        if resolve_command("rust-analyzer").is_none() {
            eprintln!("[smoke] skip: rust-analyzer not installed");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname=\"x\"\nversion=\"0.1.0\"\n",
        )
        .unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        let file = src.join("main.rs");
        std::fs::write(&file, "fn main() {}\n").unwrap();

        let mgr = noop_manager();
        let st = mgr.open_document(
            &file,
            "fn main() {\n    \n}\n",
            1,
            Some(tmp.path()),
            &LspSettings::default(),
        );
        let LspStatus::Connected { .. } = st else {
            eprintln!("[smoke] skip: rust-analyzer present but did not connect ({st:?})");
            mgr.shutdown_all();
            return;
        };
        assert_eq!(mgr.client_count(), 1);

        // Completion is best-effort here (depends on indexing timing): just
        // exercise the path and log how many options came back.
        let opts = mgr.completion(&file, 1, 4);
        eprintln!("[smoke] rust-analyzer completion options: {}", opts.len());

        // Restart tears down the live client (Requirement 6.6/6.7).
        let st = mgr.restart("rust", &LspSettings::default());
        assert!(matches!(
            st,
            LspStatus::Starting { .. } | LspStatus::Connected { .. }
        ));
        assert_eq!(mgr.client_count(), 0, "restart clears old clients");

        mgr.shutdown_all();
    }

    /// Shared connect + completion smoke for a language. Skips cleanly when the
    /// server is absent or non-functional (task 11.2 / Requirement 14.7).
    fn smoke_connect_language(
        command: &str,
        project_files: &[(&str, &str)],
        source_rel: &str,
        completion_line: u32,
        completion_char: u32,
    ) {
        if resolve_command(command).is_none() {
            eprintln!("[smoke] skip: {command} not installed");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        for (rel, content) in project_files {
            let p = tmp.path().join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&p, content).unwrap();
        }
        let file = tmp.path().join(source_rel);
        let text = std::fs::read_to_string(&file).unwrap();

        let mgr = noop_manager();
        let st = mgr.open_document(&file, &text, 1, Some(tmp.path()), &LspSettings::default());
        let LspStatus::Connected { .. } = st else {
            eprintln!("[smoke] skip: {command} present but did not connect ({st:?})");
            mgr.shutdown_all();
            return;
        };
        eprintln!("[smoke] {command} connected");
        let opts = mgr.completion(&file, completion_line, completion_char);
        eprintln!("[smoke] {command} completion options: {}", opts.len());
        mgr.shutdown_all();
    }

    #[test]
    fn smoke_connect_typescript_when_present() {
        smoke_connect_language(
            "typescript-language-server",
            &[
                (
                    "package.json",
                    "{ \"name\": \"x\", \"version\": \"0.0.0\" }",
                ),
                (
                    "tsconfig.json",
                    "{ \"compilerOptions\": { \"strict\": true } }",
                ),
                ("index.ts", "const value = 1;\nvalue.\n"),
            ],
            "index.ts",
            1,
            6,
        );
    }

    #[test]
    fn smoke_connect_python_when_present() {
        smoke_connect_language(
            "pyright-langserver",
            &[
                ("pyproject.toml", "[project]\nname = \"x\"\n"),
                ("main.py", "import os\nos.\n"),
            ],
            "main.py",
            1,
            3,
        );
    }
}
