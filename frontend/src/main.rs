// Use the Windows GUI subsystem in release builds so launching the app doesn't
// also pop a console window. Kept off in debug so println!/logs stay visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use gwenland_engine::agentic::{
    AgentPhase, AgentSession, ApplyOutcome, ApplyReport, ApprovalKind, ApprovalRecord,
    ApprovalState, ChangeSet, ContextItem, ContextItemKind, ContextOmission, ContextPreview,
    FileChangeKind, OmissionReason, ProposedFileChange, ValidationRun, ValidationStatus,
};
use gwenland_engine::ai::{AiError, AiProvider, ChatMessage, ContextAttachment, MessageRequest};
use gwenland_engine::lsp::{DiagnosticsUpdate, LspManager, LspStatus, MessageUpdate, StatusUpdate};
use gwenland_engine::terminal::PtySession;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

// ---------------------------------------------------------------------------
// Terminal I/O bridge (Milestone 3, Wave 2)
//
// The engine's `PtySession` is a zero-tauri byte pipe. This is the Tauri side:
// a manager holding the live sessions keyed by id, plus the commands the
// frontend calls. Output is *pushed* to the webview as `terminal://output`
// events from the session's reader thread (no polling), satisfying Wave 2's
// non-blocking-streaming requirement.
// ---------------------------------------------------------------------------

/// Per-chunk output event payload. `data` is the raw PTY bytes (which may not be
/// valid UTF-8 mid-escape-sequence), so it is sent as a byte array and decoded
/// by the terminal renderer on the JS side.
#[derive(Clone, Serialize)]
struct TerminalOutput {
    id: String,
    data: Vec<u8>,
}

/// Error-detected event payload (Wave 6). Emitted when the engine's detector
/// flags a line. `label` is the matched signature class (e.g. "rust-panic"),
/// `line` the offending (ANSI-stripped) text. Exposed for a future AI "explain
/// this error" feature; nothing consumes it in the UI yet.
#[derive(Clone, Serialize)]
struct TerminalErrorEvent {
    id: String,
    label: String,
    line: String,
}

/// Dev-server-ready event payload (M5 — Web Preview). Emitted once per session
/// when the engine's detector spots a dev server's ready URL in the terminal
/// output (Vite/Next/CRA/etc.). The frontend uses this to auto-open a web
/// preview pointed at `url`. `port` is parsed from the same line.
#[derive(Clone, Serialize)]
struct TerminalDevServerEvent {
    id: String,
    url: String,
    port: u16,
}

#[derive(Clone, Serialize)]
struct TerminalShellInfo {
    id: String,
    label: String,
    command: String,
}

/// Holds every open PTY session, keyed by a string id handed back to the
/// frontend at creation. Guarded by a `Mutex` since Tauri commands may run on
/// different threads. Registered as Tauri managed state.
#[derive(Default)]
struct TerminalManager {
    sessions: Arc<Mutex<HashMap<String, PtySession>>>,
    next_id: AtomicU64,
}

impl TerminalManager {
    fn alloc_id(&self) -> String {
        format!("term-{}", self.next_id.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone)]
struct ActiveSearch {
    id: String,
    cancel: Arc<AtomicBool>,
}

#[derive(Default, Clone)]
struct SearchManager {
    active: Arc<Mutex<Option<ActiveSearch>>>,
}

#[derive(Clone, Serialize)]
struct SearchResultEvent {
    search_id: String,
    result: gwenland_engine::search::SearchResult,
}

#[derive(Clone, Serialize)]
struct SearchDoneEvent {
    search_id: String,
    summary: Option<gwenland_engine::search::SearchSummary>,
    error: Option<String>,
}

// ---------------------------------------------------------------------------
// AI System (Milestone 4)
//
// All provider/keychain logic lives in the engine (`gwenland_engine::ai`). This
// is the Tauri side: managed state tracking active AI streams so they can be
// cancelled, plus the thin command wrappers. API keys never appear here beyond
// being passed straight through to the engine's keychain wrapper in `ai_set_key`.
// ---------------------------------------------------------------------------

/// Tracks the in-flight AI stream tasks, keyed by the frontend-generated
/// `stream_id`, so `ai_cancel` can abort the matching one. The map is behind an
/// `Arc` so the spawned stream task can remove its own entry on completion.
/// M4 supports one active stream per window, but the map generalizes cleanly.
/// M13 adds `search_resolvers`: when a stream detects a search trigger it parks
/// a oneshot `Sender<String>` here keyed by `stream_id`; `ai_search_result`
/// looks it up and delivers the search text to unblock the continuation.
#[derive(Default, Clone)]
struct AiManager {
    active_streams: Arc<Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>>,
    search_resolvers: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<String>>>>,
}

static NEXT_WINDOW_LABEL_ID: AtomicU64 = AtomicU64::new(1);

fn allocate_window_label(app: &AppHandle) -> String {
    loop {
        let id = NEXT_WINDOW_LABEL_ID.fetch_add(1, Ordering::Relaxed);
        let label = format!("window-{id}");
        if app.get_webview_window(&label).is_none() {
            return label;
        }
    }
}

#[tauri::command]
async fn open_new_window(app: AppHandle) -> Result<String, String> {
    let label = allocate_window_label(&app);
    tauri::WebviewWindowBuilder::new(
        &app,
        label.clone(),
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("GwenLand IDE")
    .inner_size(1280.0, 800.0)
    .min_inner_size(800.0, 600.0)
    .resizable(true)
    .decorations(true)
    .build()
    .map_err(|e| e.to_string())?;
    Ok(label)
}

// --- AI streaming event payloads -------------------------------------------
// Every event for a request carries its `stream_id` so the UI can correlate.

/// `ai://chunk` — one streamed token fragment.
#[derive(Clone, Serialize)]
struct AiChunkEvent {
    stream_id: String,
    text: String,
}

/// `ai://done` — successful completion (exactly one per stream, terminal).
/// `usage` is present when the provider reported token counts (GWEN-457) —
/// absent for providers/streams where it never arrived.
#[derive(Clone, Serialize)]
struct AiDoneEvent {
    stream_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<gwenland_engine::ai::TokenUsage>,
}

/// `ai://error` — normalized failure or cancellation (terminal). The `error`
/// serializes as `{ "kind", "data" }` so the UI can branch on `kind`.
#[derive(Clone, Serialize)]
struct AiErrorEvent {
    stream_id: String,
    error: AiError,
}

/// M13 — `ai://search_request` — emitted when the assistant writes the exact
/// search trigger phrase. The UI calls `ai_search_result` to resume the stream.
#[derive(Clone, Serialize)]
struct AiSearchRequestEvent {
    stream_id: String,
    query: String,
}

fn emit_ai_error(app: &AppHandle, stream_id: &str, error: AiError) {
    let _ = app.emit(
        "ai://error",
        AiErrorEvent {
            stream_id: stream_id.to_string(),
            error,
        },
    );
}

async fn run_blocking<T, F>(caller: &'static str, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|e| format!("{caller} blocking task failed: {e}"))?
}

fn hide_child_window(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

/// Creates a PTY session running the platform default shell and starts
/// streaming its output to the frontend as `terminal://output` events. Returns
/// the new session id.
#[tauri::command]
async fn terminal_create(
    app: AppHandle,
    manager: State<'_, TerminalManager>,
    rows: u16,
    cols: u16,
    cwd: Option<String>,
    shell: Option<String>,
) -> Result<String, String> {
    let id = manager.alloc_id();
    let sessions = manager.sessions.clone();
    run_blocking("terminal_create", move || {
        let output_id = id.clone();
        let output_app = app.clone();
        let error_id = id.clone();
        let error_app = app.clone();
        let devserver_id = id.clone();
        let devserver_app = app.clone();
        // Open the shell in the project folder when one is provided; the engine
        // ignores a non-existent path and falls back to the default directory.
        let cwd_path = cwd.as_deref().map(PathBuf::from);
        let cwd_ref = cwd_path.as_deref();
        let session_result = PtySession::spawn_shell_with_callback(
            rows,
            cols,
            cwd_ref,
            shell.as_deref(),
            Box::new(move |chunk: &[u8]| {
                // Emit failures (e.g. window gone) are not fatal to the PTY; ignore.
                let _ = output_app.emit(
                    "terminal://output",
                    TerminalOutput {
                        id: output_id.clone(),
                        data: chunk.to_vec(),
                    },
                );
            }),
            // Wave 6: forward each detected error as a `terminal://error` event.
            Some(Box::new(
                move |sig: &gwenland_engine::error_detect::ErrorSignal| {
                    let _ = error_app.emit(
                        "terminal://error",
                        TerminalErrorEvent {
                            id: error_id.clone(),
                            label: sig.label.clone(),
                            line: sig.line.clone(),
                        },
                    );
                },
            )),
            // M5: forward the detected dev-server URL as a `terminal://devserver-ready`
            // event (fires at most once per session) so the UI can auto-open a preview.
            Some(Box::new(
                move |sig: &gwenland_engine::devserver_detect::DevServerSignal| {
                    let _ = devserver_app.emit(
                        "terminal://devserver-ready",
                        TerminalDevServerEvent {
                            id: devserver_id.clone(),
                            url: sig.url.clone(),
                            port: sig.port,
                        },
                    );
                },
            )),
        );

        match session_result {
            Ok(session) => {
                sessions
                    .lock()
                    .map_err(|_| "terminal manager lock poisoned".to_string())?
                    .insert(id.clone(), session);
                Ok(id)
            }
            Err(e) => Err(e.to_string()),
        }
    })
    .await
}

#[tauri::command]
async fn terminal_detect_shells() -> Result<Vec<TerminalShellInfo>, String> {
    run_blocking("terminal_detect_shells", move || {
        let candidates: &[(&str, &str, &str)] = if cfg!(windows) {
            &[
                ("powershell", "PowerShell", "powershell.exe"),
                ("pwsh", "PowerShell 7", "pwsh.exe"),
                ("cmd", "Command Prompt", "cmd.exe"),
                ("wsl", "WSL", "wsl.exe"),
                ("bun", "Bun", "bun.exe"),
                ("node", "Node", "node.exe"),
                ("python", "Python", "python.exe"),
            ]
        } else {
            &[
                ("bash", "Bash", "bash"),
                ("zsh", "Zsh", "zsh"),
                ("fish", "Fish", "fish"),
                ("sh", "Sh", "sh"),
                ("bun", "Bun", "bun"),
                ("node", "Node", "node"),
                ("python", "Python", "python3"),
            ]
        };

        let mut shells = Vec::new();
        let mut seen = std::collections::HashSet::<String>::new();
        for (id, label, command) in candidates {
            if let Some(path) = gwenland_engine::lsp::resolve_command(command) {
                let key = path.to_string_lossy().to_lowercase();
                if seen.insert(key) {
                    shells.push(TerminalShellInfo {
                        id: (*id).to_string(),
                        label: (*label).to_string(),
                        command: path.to_string_lossy().into_owned(),
                    });
                }
            }
        }
        Ok(shells)
    })
    .await
}

/// Sends raw input bytes (keystrokes / pasted text) to a session's PTY.
#[tauri::command]
async fn terminal_write(
    manager: State<'_, TerminalManager>,
    id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let sessions = manager.sessions.clone();
    run_blocking("terminal_write", move || {
        let mut sessions = sessions
            .lock()
            .map_err(|_| "terminal manager lock poisoned".to_string())?;
        let session = sessions
            .get_mut(&id)
            .ok_or_else(|| format!("no terminal session {id}"))?;
        session.write_input(&data).map_err(|e| e.to_string())
    })
    .await
}

/// Resizes a session's PTY (cols/rows) as the panel is resized.
#[tauri::command]
async fn terminal_resize(
    manager: State<'_, TerminalManager>,
    id: String,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let sessions = manager.sessions.clone();
    run_blocking("terminal_resize", move || {
        let sessions = sessions
            .lock()
            .map_err(|_| "terminal manager lock poisoned".to_string())?;
        let session = sessions
            .get(&id)
            .ok_or_else(|| format!("no terminal session {id}"))?;
        session.resize(rows, cols).map_err(|e| e.to_string())
    })
    .await
}

/// Kills a session and removes it from the manager. Dropping the removed
/// `PtySession` runs its kill+reap teardown. Killing an unknown id is a no-op.
#[tauri::command]
async fn terminal_kill(manager: State<'_, TerminalManager>, id: String) -> Result<(), String> {
    let sessions = manager.sessions.clone();
    run_blocking("terminal_kill", move || {
        let mut sessions = sessions
            .lock()
            .map_err(|_| "terminal manager lock poisoned".to_string())?;
        if let Some(mut session) = sessions.remove(&id) {
            session.kill().map_err(|e| e.to_string())?;
        }
        Ok(())
    })
    .await
}

#[tauri::command]
async fn get_app_data_path() -> Result<String, String> {
    run_blocking("get_app_data_path", move || {
        gwenland_engine::app_data::get_app_data_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn load_settings() -> Result<gwenland_engine::settings::Settings, String> {
    run_blocking("load_settings", move || {
        gwenland_engine::settings::load_settings().map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn save_settings(settings: gwenland_engine::settings::Settings) -> Result<(), String> {
    run_blocking("save_settings", move || {
        gwenland_engine::settings::save_settings(&settings).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn get_recent_projects()
-> Result<Vec<gwenland_engine::recent_projects::RecentProject>, String> {
    run_blocking("get_recent_projects", move || {
        gwenland_engine::recent_projects::get_recent_projects().map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn add_recent_project(path: String) -> Result<(), String> {
    run_blocking("add_recent_project", move || {
        gwenland_engine::recent_projects::add_recent_project(Path::new(&path))
            .map_err(|e| e.to_string())
    })
    .await
}

// Folder-dialog plumbing lives here in the frontend (not engine) so the engine
// crate stays free of any `tauri` dependency. The body exceeds the usual 2-line
// wrapper budget because the native dialog is inherently Tauri-side plumbing.
#[tauri::command]
async fn open_folder_dialog(app: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |folder| {
        let _ = tx.send(folder);
    });

    match rx.await {
        Ok(Some(path)) => path
            .into_path()
            .map_err(|e| e.to_string())?
            .to_str()
            .map(|s| s.to_string())
            .ok_or_else(|| gwenland_engine::fs::FsError::InvalidUtf8.to_string()),
        // User dismissed the dialog: surface the engine's cancellation variant.
        Ok(None) => Err(gwenland_engine::fs::FsError::DialogCancelled.to_string()),
        Err(_) => Err(gwenland_engine::fs::FsError::DialogCancelled.to_string()),
    }
}

#[tauri::command]
async fn list_directory(path: String) -> Result<Vec<gwenland_engine::fs::DirEntry>, String> {
    run_blocking("list_directory", move || {
        gwenland_engine::fs::list_directory(Path::new(&path)).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn read_file(path: String) -> Result<String, String> {
    run_blocking("read_file", move || {
        gwenland_engine::fs::read_file(Path::new(&path)).map_err(|e| e.to_string())
    })
    .await
}

/// Return a file's size + line count (M19 Wave 3). The UI uses this to pick
/// Large File Mode before building the editor. Line counting is skipped for very
/// large files (reported as `u64::MAX`).
#[tauri::command]
async fn get_file_meta(path: String) -> Result<gwenland_engine::fs::FileMeta, String> {
    run_blocking("get_file_meta", move || {
        gwenland_engine::fs::file_meta(Path::new(&path)).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn write_file(path: String, content: String) -> Result<(), String> {
    run_blocking("write_file", move || {
        gwenland_engine::fs::write_file(Path::new(&path), &content).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn path_exists(path: String) -> bool {
    tauri::async_runtime::spawn_blocking(move || Path::new(&path).exists())
        .await
        .unwrap_or(false)
}

// --- Workspace-scoped file operations (Milestone 9 — Context Menu System) ----
// Every right-click file mutation goes through these. The engine rejects any
// target outside `workspace_root`, so the context menu can never touch files
// beyond the open project (Requirement 5.3 / 8.4).

/// Create an empty file (New File). Rejected outside the workspace.
#[tauri::command]
async fn create_file(path: String, workspace_root: String) -> Result<(), String> {
    run_blocking("create_file", move || {
        gwenland_engine::fs::create_file(Path::new(&path), Path::new(&workspace_root))
            .map_err(|e| e.to_string())
    })
    .await
}

/// Create a directory (New Folder). Rejected outside the workspace.
#[tauri::command]
async fn create_dir(path: String, workspace_root: String) -> Result<(), String> {
    run_blocking("create_dir", move || {
        gwenland_engine::fs::create_dir(Path::new(&path), Path::new(&workspace_root))
            .map_err(|e| e.to_string())
    })
    .await
}

/// Rename/move a path. Both source and destination must be inside the workspace.
#[tauri::command]
async fn rename_path(old: String, new: String, workspace_root: String) -> Result<(), String> {
    run_blocking("rename_path", move || {
        gwenland_engine::fs::rename_path(
            Path::new(&old),
            Path::new(&new),
            Path::new(&workspace_root),
        )
        .map_err(|e| e.to_string())
    })
    .await
}

/// Delete a file or directory (recursive). Rejected outside the workspace.
#[tauri::command]
async fn delete_path(path: String, workspace_root: String) -> Result<(), String> {
    run_blocking("delete_path", move || {
        gwenland_engine::fs::delete_path(Path::new(&path), Path::new(&workspace_root))
            .map_err(|e| e.to_string())
    })
    .await
}

/// Duplicate a file/directory next to itself; returns the new path. Rejected
/// outside the workspace.
#[tauri::command]
async fn duplicate_path(path: String, workspace_root: String) -> Result<String, String> {
    run_blocking("duplicate_path", move || {
        gwenland_engine::fs::duplicate_path(Path::new(&path), Path::new(&workspace_root))
            .map_err(|e| e.to_string())
    })
    .await
}

/// Reveal a path in the OS file manager (Explorer/Finder/xdg). Read-only, but
/// still workspace-checked so the action respects project boundaries. Platform
/// launchers return success codes inconsistently, so a non-zero exit is ignored.
#[tauri::command]
async fn reveal_in_explorer(path: String, workspace_root: String) -> Result<(), String> {
    run_blocking("reveal_in_explorer", move || {
        let target = std::path::Path::new(&path);
        gwenland_engine::fs::check_within_workspace(target, std::path::Path::new(&workspace_root))
            .map_err(|e| e.to_string())?;

        #[cfg(target_os = "windows")]
        let result = {
            let mut cmd = std::process::Command::new("explorer");
            cmd.arg(format!("/select,{path}"));
            hide_child_window(&mut cmd);
            cmd.spawn()
        };

        #[cfg(target_os = "macos")]
        let result = {
            let mut cmd = std::process::Command::new("open");
            cmd.arg("-R").arg(&path);
            cmd.spawn()
        };

        #[cfg(all(unix, not(target_os = "macos")))]
        let result = {
            // No portable "select file" on Linux; open the containing directory.
            let dir = target.parent().unwrap_or(target);
            let mut cmd = std::process::Command::new("xdg-open");
            cmd.arg(dir);
            cmd.spawn()
        };

        result.map(|_| ()).map_err(|e| e.to_string())
    })
    .await
}

fn command_status(
    name: &str,
    status: std::io::Result<std::process::ExitStatus>,
) -> Result<(), String> {
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("{name} exited with status {status}")),
        Err(err) => Err(format!("{name} failed: {err}")),
    }
}

#[cfg(target_os = "windows")]
fn move_target_to_os_trash(target: &std::path::Path) -> Result<(), String> {
    let path = target
        .to_str()
        .ok_or_else(|| "path is not valid UTF-8".to_string())?;
    let script = r#"
Add-Type -AssemblyName Microsoft.VisualBasic
$p = $args[0]
if (Test-Path -LiteralPath $p -PathType Container) {
  [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory($p, [Microsoft.VisualBasic.FileIO.UIOption]::OnlyErrorDialogs, [Microsoft.VisualBasic.FileIO.RecycleOption]::SendToRecycleBin)
} else {
  [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile($p, [Microsoft.VisualBasic.FileIO.UIOption]::OnlyErrorDialogs, [Microsoft.VisualBasic.FileIO.RecycleOption]::SendToRecycleBin)
}
"#;
    command_status("PowerShell recycle-bin move", {
        let args = vec![
            "-NoProfile".to_string(),
            "-NonInteractive".to_string(),
            "-Command".to_string(),
            script.to_string(),
            path.to_string(),
        ];
        let mut cmd = std::process::Command::new("powershell");
        cmd.args(&args);
        hide_child_window(&mut cmd);
        cmd.status()
    })
}

#[cfg(target_os = "macos")]
fn move_target_to_os_trash(target: &std::path::Path) -> Result<(), String> {
    let path = target
        .to_str()
        .ok_or_else(|| "path is not valid UTF-8".to_string())?;
    let args = vec![
        "-e".to_string(),
        "on run argv".to_string(),
        "-e".to_string(),
        "tell application \"Finder\" to delete POSIX file (item 1 of argv)".to_string(),
        "-e".to_string(),
        "end run".to_string(),
        path.to_string(),
    ];
    let mut cmd = std::process::Command::new("osascript");
    cmd.args(&args);
    let result = cmd.status();
    command_status("Finder trash move", result)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn move_target_to_os_trash(target: &std::path::Path) -> Result<(), String> {
    let attempts = [
        ("gio trash", vec!["gio", "trash"]),
        ("kioclient5 trash", vec!["kioclient5", "move"]),
        ("kioclient trash", vec!["kioclient", "move"]),
        ("trash-put", vec!["trash-put"]),
    ];
    let mut errors = Vec::new();
    for (name, command) in attempts {
        let mut cmd = std::process::Command::new(command[0]);
        for arg in command.iter().skip(1) {
            cmd.arg(arg);
        }
        cmd.arg(target);
        if name.starts_with("kioclient") {
            cmd.arg("trash:/");
        }
        let result = cmd.status();
        match command_status(name, result) {
            Ok(()) => return Ok(()),
            Err(err) => errors.push(err),
        }
    }
    Err(format!(
        "no OS trash command succeeded (tried gio, kioclient5, kioclient, trash-put): {}",
        errors.join("; ")
    ))
}

/// Move a path to the OS-native trash/recycle bin. This is for manual user
/// file-tree deletion; agent deletes still use `.gwenland/trash/` recovery.
#[tauri::command]
async fn move_path_to_os_trash(path: String, workspace_root: String) -> Result<(), String> {
    run_blocking("move_path_to_os_trash", move || {
        let target = Path::new(&path);
        gwenland_engine::fs::check_within_workspace(target, Path::new(&workspace_root))
            .map_err(|e| e.to_string())?;
        if !target.exists() {
            return Err(format!("path not found: {path}"));
        }
        move_target_to_os_trash(target)
    })
    .await
}

// --- AI key commands (Milestone 4, Wave 1) ---------------------------------
// Write-only/status-only by design: `ai_check_key` returns a bool, never the
// value; `ai_set_key` receives a key and hands it to the OS keychain.

/// Store (or replace) an API key for `provider` in the OS keychain.
#[tauri::command]
async fn ai_set_key(provider: String, api_key: String) -> Result<(), String> {
    run_blocking("ai_set_key", move || {
        gwenland_engine::ai::keychain::set_api_key(&provider, &api_key).map_err(|e| e.to_string())
    })
    .await
}

/// Delete a provider's stored key. Idempotent (deleting an absent key is Ok).
#[tauri::command]
async fn ai_delete_key(provider: String) -> Result<(), String> {
    run_blocking("ai_delete_key", move || {
        gwenland_engine::ai::keychain::delete_api_key(&provider).map_err(|e| e.to_string())
    })
    .await
}

/// Report only whether a key is stored for `provider` — never the value.
#[tauri::command]
async fn ai_check_key(provider: String) -> Result<bool, String> {
    run_blocking("ai_check_key", move || {
        gwenland_engine::ai::keychain::has_api_key(&provider).map_err(|e| e.to_string())
    })
    .await
}

/// List models for `provider`. Resolves the provider via the engine registry
/// (loading current AI settings for generic provider config) and returns the
/// adapter's model list, or `None` when listing is unsupported.
#[tauri::command]
async fn ai_list_models(
    provider: String,
) -> Result<Option<Vec<gwenland_engine::ai::ModelInfo>>, String> {
    let settings = run_blocking("ai_list_models.load_settings", move || {
        gwenland_engine::settings::load_settings().map_err(|e| e.to_string())
    })
    .await?;
    let adapter = gwenland_engine::ai::registry::resolve_provider(&provider, &settings.ai)
        .map_err(|e| e.to_string())?;
    adapter.list_models().await.map_err(|e| e.to_string())
}

/// The full static model catalog (Milestone 26): every model GwenLand knows
/// about across all providers, with pricing and reasoning capability. Pure
/// data, no I/O — used by the model selector, effort toggle, and token
/// tracker.
#[tauri::command]
fn ai_model_catalog() -> Vec<gwenland_engine::ai::ModelEntry> {
    gwenland_engine::ai::all_models()
}

// --- Conversation commands (Milestone 4, Wave 3) ---------------------------
// Thin wrappers over `gwenland_engine::ai::conversation`; errors stringified at
// the boundary.

#[tauri::command]
async fn conversation_new(
    project_root: String,
    title: String,
    provider: String,
    model: String,
) -> Result<gwenland_engine::ai::conversation::ConversationMeta, String> {
    run_blocking("conversation_new", move || {
        gwenland_engine::ai::conversation::new_conversation(
            Path::new(&project_root),
            &title,
            &provider,
            &model,
        )
        .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn conversation_list()
-> Result<Vec<gwenland_engine::ai::conversation::ConversationMeta>, String> {
    run_blocking("conversation_list", move || {
        gwenland_engine::ai::conversation::list_conversations().map_err(|e| e.to_string())
    })
    .await
}

/// Parse assistant text into structured diff files (Milestone 8, Wave 5). Pure
/// wrapper over the engine parser; prose/fences are ignored, malformed hunks
/// surface as a stringified error the UI shows as a non-destructive notice.
#[tauri::command]
async fn parse_diff(text: String) -> Result<Vec<gwenland_engine::ai::DiffFile>, String> {
    run_blocking("parse_diff", move || {
        gwenland_engine::ai::parse_unified_diff(&text).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn conversation_load(
    conversation_id: String,
) -> Result<Vec<gwenland_engine::ai::conversation::ConversationTurn>, String> {
    run_blocking("conversation_load", move || {
        gwenland_engine::ai::conversation::load_turns(&conversation_id).map_err(|e| e.to_string())
    })
    .await
}

/// Truncate a conversation to its first `keep_count` turns (GWEN-326 message
/// edit/rollback). Returns the surviving turns so the UI can re-sync.
#[tauri::command]
async fn conversation_truncate(
    conversation_id: String,
    keep_count: usize,
) -> Result<Vec<gwenland_engine::ai::conversation::ConversationTurn>, String> {
    run_blocking("conversation_truncate", move || {
        gwenland_engine::ai::conversation::truncate_turns(&conversation_id, keep_count)
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn conversation_rename(conversation_id: String, title: String) -> Result<(), String> {
    run_blocking("conversation_rename", move || {
        gwenland_engine::ai::conversation::rename_conversation(&conversation_id, &title)
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn conversation_delete(conversation_id: String) -> Result<(), String> {
    run_blocking("conversation_delete", move || {
        gwenland_engine::ai::conversation::delete_conversation(&conversation_id)
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn conversation_set_training_opt_in(
    conversation_id: String,
    opt_in: bool,
) -> Result<(), String> {
    run_blocking("conversation_set_training_opt_in", move || {
        gwenland_engine::ai::conversation::set_training_opt_in(&conversation_id, opt_in)
            .map_err(|e| e.to_string())
    })
    .await
}

// --- AI streaming (Milestone 4, Waves 3-4) ---------------------------------

/// Drive one provider stream to completion, emitting `ai://chunk` per token and
/// exactly one terminal `ai://done`/`ai://error`. On clean completion the
/// finished turn is appended to the conversation JSONL (cancelled/failed streams
/// are not persisted). After JSONL persistence, triggers M13 memory write-back
/// best-effort (silent on failure).
///
/// M13: detects the first `Let me search <topic> for more detail...` trigger per
/// response, pauses the stream, emits `ai://search_request`, awaits the UI's
/// `ai_search_result` call, then continues with a second provider request.
#[allow(clippy::too_many_arguments)]
async fn run_stream(
    app: AppHandle,
    ai_manager: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<String>>>>,
    adapter: Box<dyn AiProvider>,
    request: MessageRequest,
    expanded_user_message: String,
    provider_id: String,
    model_id: String,
    conversation_id: String,
    project_path: String,
    conversation_name: String,
) {
    let stream_id = request.stream_id.clone();
    // Clone the prior messages so we can rebuild a continuation request if needed.
    let prior_messages = request.messages.clone();
    let system_prompt = request.system.clone();
    let model = request.model.clone();

    let mut stream = match adapter.send_message(request).await {
        Ok(s) => s,
        Err(e) => return emit_ai_error(&app, &stream_id, e),
    };

    let mut assistant = String::new();
    // Token usage for the completed request (GWEN-457), taken from whichever
    // stream actually finished — the continuation stream's usage supersedes
    // the original's if an M13 search round-trip occurred (the original was
    // dropped mid-read at that point, so its own usage would be incomplete).
    // Every loop exit that reaches the code below sets this before use.
    let usage: Option<gwenland_engine::ai::TokenUsage>;

    loop {
        match stream.next_chunk().await {
            Ok(Some(chunk)) => {
                assistant.push_str(&chunk.text);
                let _ = app.emit(
                    "ai://chunk",
                    AiChunkEvent {
                        stream_id: stream_id.clone(),
                        text: chunk.text.clone(),
                    },
                );

                // M13: detect the first search trigger per response.
                // We break out of the outer loop after the continuation so the
                // guard `detect_search_trigger` is only reached once.
                if let Some(query) = detect_search_trigger(&assistant) {
                    drop(stream); // stop reading the current stream

                    // Park a oneshot resolver so `ai_search_result` can wake us.
                    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
                    if let Ok(mut resolvers) = ai_manager.lock() {
                        resolvers.insert(stream_id.clone(), tx);
                    }

                    // Tell the UI to fetch the search result.
                    let _ = app.emit(
                        "ai://search_request",
                        AiSearchRequestEvent {
                            stream_id: stream_id.clone(),
                            query: query.clone(),
                        },
                    );

                    // Wait for result (or timeout / cancellation).
                    let search_text = tokio::time::timeout(std::time::Duration::from_secs(30), rx)
                        .await
                        .unwrap_or_else(|_| Ok(String::new()))
                        .unwrap_or_default();

                    // Clean up resolver (may already be gone if cancelled).
                    let _ = ai_manager.lock().map(|mut r| r.remove(&stream_id));

                    let search_block = if search_text.trim().is_empty() {
                        format!("<search query=\"{query}\">\n[unavailable]\n</search>")
                    } else {
                        let capped = truncate_chars(&search_text, 2000);
                        format!("<search query=\"{query}\">\n{capped}\n</search>")
                    };

                    // Build continuation: original messages + assistant partial + search observation.
                    let settings = gwenland_engine::settings::load_settings()
                        .map_err(|e| gwenland_engine::ai::AiError::ProviderError(e.to_string()));
                    let adapter2 = settings.and_then(|s| {
                        gwenland_engine::ai::registry::resolve_provider(&provider_id, &s.ai)
                            .map_err(|e| gwenland_engine::ai::AiError::ProviderError(e.to_string()))
                    });
                    let adapter2 = match adapter2 {
                        Ok(a) => a,
                        Err(e) => return emit_ai_error(&app, &stream_id, e),
                    };

                    let mut cont_messages = prior_messages.clone();
                    cont_messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: assistant.clone(),
                    });
                    cont_messages.push(ChatMessage::user(search_block));

                    let cont_request = MessageRequest {
                        stream_id: stream_id.clone(),
                        messages: cont_messages,
                        system: system_prompt.clone(),
                        attachments: Vec::new(),
                        images: Vec::new(),
                        model: model.clone(),
                        max_tokens: None,
                    };

                    let mut cont_stream = match adapter2.send_message(cont_request).await {
                        Ok(s) => s,
                        Err(e) => return emit_ai_error(&app, &stream_id, e),
                    };

                    // Continue emitting chunks on the same stream_id.
                    loop {
                        match cont_stream.next_chunk().await {
                            Ok(Some(chunk)) => {
                                assistant.push_str(&chunk.text);
                                let _ = app.emit(
                                    "ai://chunk",
                                    AiChunkEvent {
                                        stream_id: stream_id.clone(),
                                        text: chunk.text,
                                    },
                                );
                            }
                            Ok(None) => break,
                            Err(e) => return emit_ai_error(&app, &stream_id, e),
                        }
                    }
                    usage = cont_stream.usage();

                    // Fall through to persist + done below.
                    break;
                }
            }
            Ok(None) => {
                usage = stream.usage();
                break;
            }
            Err(e) => return emit_ai_error(&app, &stream_id, e),
        }
    }

    // Persist the completed turn once (whether or not search continuation ran).
    let _ = gwenland_engine::ai::conversation::record_turn(
        &conversation_id,
        &expanded_user_message,
        &assistant,
        &provider_id,
        &model_id,
        usage,
    );
    let _ = app.emit(
        "ai://done",
        AiDoneEvent {
            stream_id: stream_id.clone(),
            usage,
        },
    );
    // M13: memory write-back — best-effort, silent on failure.
    run_memory_writeback(
        &expanded_user_message,
        &assistant,
        &provider_id,
        &model_id,
        &project_path,
        &conversation_name,
    )
    .await;
}

/// Detect the M13 search trigger phrase: `Let me search <topic> for more detail...`
/// Returns the extracted topic (trimmed) or `None`.
fn detect_search_trigger(text: &str) -> Option<String> {
    let marker = "Let me search ";
    let suffix = " for more detail...";
    // Find the first occurrence in the buffered text.
    let start = text.find(marker)?;
    let after_marker = &text[start + marker.len()..];
    let end = after_marker.find(suffix)?;
    let topic = after_marker[..end].trim();
    if topic.is_empty() || topic.len() > 200 {
        return None;
    }
    Some(topic.to_string())
}

/// After a successful response, call the write-back mini LLM to produce a short
/// memory note, then write it to `.gwenland/agent/memory/`. Any failure is
/// silent debug-only and never blocks the caller.
async fn run_memory_writeback(
    user_message: &str,
    assistant_response: &str,
    provider_id: &str,
    model_id: &str,
    project_path: &str,
    conversation_name: &str,
) {
    if assistant_response.trim().is_empty() {
        return;
    }

    const WRITEBACK_SYSTEM: &str = "You just completed a coding task. Write a short memory note \
        (max 10-15 lines). Return ONLY JSON: { \"filename\": \"<kebab-case>.md\", \"content\": \"...\" }";

    // Truncate inputs to approximately 500 tokens (2000 chars).
    let user_truncated = truncate_chars(user_message, 1000);
    let assistant_truncated = truncate_chars(assistant_response, 1500);
    let prompt = format!("User:\n{user_truncated}\n\nAssistant:\n{assistant_truncated}");

    let raw = match complete_once(
        Some(WRITEBACK_SYSTEM.to_string()),
        prompt,
        provider_id,
        model_id,
        Some(150),
    )
    .await
    {
        Ok(r) => r,
        Err(_) => return, // silent failure
    };

    let note = match gwenland_engine::agentic::parse_memory_note(&raw) {
        Ok(n) => n,
        Err(_) => return,
    };

    let workspace = std::path::Path::new(project_path);
    let project_name = gwenland_engine::agentic::project_name_from_root(workspace);
    let conv_name = gwenland_engine::agentic::sanitize_segment(conversation_name, "conversation");
    let target = gwenland_engine::agentic::MemoryWriteTarget {
        project_name,
        conversation_name: conv_name,
        filename: note.filename.clone(),
    };

    let _ = gwenland_engine::agentic::write_memory_note(workspace, &target, &note);
}

fn truncate_chars(s: &str, max_chars: usize) -> &str {
    if s.len() <= max_chars {
        return s;
    }
    let mut end = max_chars;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// GwenLand's AI system prompt (Requirement 10.1-10.3, plus always-on thinking).
/// Keeps the assistant local-first and review-first: it reasons in <think> tags,
/// then proposes changes as unified diffs the user explicitly accepts — it never
/// claims to apply edits itself.
const GWENLAND_SYSTEM_PROMPT: &str = "\
You are GwenLand's coding assistant, embedded in a local-first IDE.

Always think before you answer. Begin EVERY reply with your step-by-step reasoning
wrapped in <think> and </think> tags, then write the final answer AFTER the closing
</think> tag. Never place the final answer, code, or a diff inside the <think> block —
the reasoning is shown to the user separately from the answer.

When proposing code changes, put a unified diff in the final answer (after </think>):
--- a/path/to/file
+++ b/path/to/file
@@ -line,count +line,count @@
-removed line
+added line

Guidelines:
- Prefer unified diffs for edits and refactors so the user can review them hunk by hunk.
- You never apply changes yourself; the user decides which hunks to accept.
- Use real project-relative file paths in the diff headers.
- For questions that are not code edits, still reason in <think> first, then answer normally without a diff.
- If you need current or external information to answer well, write exactly this on its own line:
  Let me search <topic> for more detail...
  (keep <topic> short — 2-5 words, no secrets, no file contents). Then wait; search results will be injected before you continue.";

/// Compose the effective system prompt. A per-workspace persona/system prompt
/// (GWEN-334) is layered ON TOP of `GWENLAND_SYSTEM_PROMPT` — the persona sets
/// voice/instructions while the base prompt keeps the always-on <think> +
/// review-first-diff protocol the UI depends on for parsing. An empty/blank
/// prefix falls back to the base prompt unchanged.
fn compose_system_prompt(prefix: Option<&str>) -> String {
    match prefix.map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => format!("{p}\n\n---\n\n{GWENLAND_SYSTEM_PROMPT}"),
        None => GWENLAND_SYSTEM_PROMPT.to_string(),
    }
}

/// Bounded, non-streaming, non-persisted completion for internal side-prompts
/// (keyword extraction, memory write-back). Never records JSONL, never triggers
/// memory retrieval or write-back itself, and never surfaces a visible error to
/// the user. Returns the trimmed assistant text, or `Err` on provider failure.
async fn complete_once(
    system: Option<String>,
    prompt: String,
    provider_id: &str,
    model_id: &str,
    max_tokens: Option<u32>,
) -> Result<String, gwenland_engine::ai::AiError> {
    let settings = gwenland_engine::settings::load_settings()
        .map_err(|e| gwenland_engine::ai::AiError::ProviderError(e.to_string()))?;
    let adapter = gwenland_engine::ai::registry::resolve_provider(provider_id, &settings.ai)
        .map_err(|e| gwenland_engine::ai::AiError::ProviderError(e.to_string()))?;

    let request = MessageRequest {
        stream_id: format!("mini-{}", gwenland_engine::agentic::new_id()),
        messages: vec![ChatMessage::user(prompt)],
        system,
        attachments: Vec::new(),
        images: Vec::new(),
        model: model_id.to_string(),
        max_tokens,
    };

    let mut stream = adapter.send_message(request).await?;
    let mut text = String::new();
    while let Some(chunk) = stream.next_chunk().await? {
        text.push_str(&chunk.text);
    }
    Ok(text.trim().to_string())
}

/// Extract 3-7 search keywords from `user_prompt` using a bounded mini-call.
/// Returns `None` on any failure so the caller can skip memory gracefully.
async fn extract_keywords(
    user_prompt: &str,
    provider_id: &str,
    model_id: &str,
) -> Option<Vec<String>> {
    const SYSTEM: &str =
        "Extract 3-7 search keywords from this user message. Return ONLY a JSON array of strings.";

    let raw = complete_once(
        Some(SYSTEM.to_string()),
        user_prompt.to_string(),
        provider_id,
        model_id,
        Some(80),
    )
    .await
    .ok()?;

    let keywords = gwenland_engine::agentic::parse_keyword_array(&raw);
    if keywords.is_empty() {
        return None;
    }

    // Normalize: trim, deduplicate case-insensitively, cap at 7.
    let mut seen: Vec<String> = Vec::new();
    for kw in keywords {
        let kw = kw.trim().to_string();
        if kw.is_empty() {
            continue;
        }
        if !seen.iter().any(|s: &String| s.eq_ignore_ascii_case(&kw)) {
            seen.push(kw);
        }
        if seen.len() >= 7 {
            break;
        }
    }

    if seen.is_empty() { None } else { Some(seen) }
}

/// Run keyword extraction then memory grep and return the rendered `<memory>`
/// block, or `None` when memory is unavailable or extraction fails. Always
/// non-fatal: any error returns `None` so the caller skips injection silently.
async fn retrieve_memory_block(
    raw_user_prompt: &str,
    project_path: &str,
    provider_id: &str,
    model_id: &str,
) -> Option<String> {
    let workspace = std::path::Path::new(project_path);
    let project_name = gwenland_engine::agentic::project_name_from_root(workspace);

    // Extract keywords — failure silently skips memory injection.
    let keywords = extract_keywords(raw_user_prompt, provider_id, model_id).await?;

    let results = gwenland_engine::agentic::search_memory(
        workspace,
        &project_name,
        &keywords,
        gwenland_engine::agentic::MemoryBudget::default(),
    )
    .ok()?;

    gwenland_engine::agentic::render_memory_block(
        &results,
        gwenland_engine::agentic::MemoryBudget::default(),
    )
}

/// Start a streaming completion. The UI generates `stream_id` and registers its
/// listeners before calling this. Returns the same `stream_id` once accepted.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn ai_send(
    app: AppHandle,
    manager: State<'_, AiManager>,
    stream_id: String,
    conversation_id: String,
    message: String,
    attachments: Vec<ContextAttachment>,
    images: Vec<gwenland_engine::ai::ImageAttachment>,
    provider: Option<String>,
    model: Option<String>,
    system_prefix: Option<String>,
) -> Result<String, String> {
    // Reject a duplicate active stream id (Requirement 10.5 / 11.7).
    {
        let streams = manager
            .active_streams
            .lock()
            .map_err(|_| "ai manager lock poisoned".to_string())?;
        if streams.contains_key(&stream_id) {
            return Err(format!("stream {stream_id} is already active"));
        }
    }

    let settings = gwenland_engine::settings::load_settings().map_err(|e| e.to_string())?;
    let meta = gwenland_engine::ai::conversation::get_conversation(&conversation_id)
        .map_err(|e| e.to_string())?;

    // Resolve provider/model: explicit override > conversation metadata >
    // global active setting (Requirement 10.5 / 3.6 / 3.7).
    let provider_id = provider
        .filter(|s| !s.is_empty())
        .or_else(|| (!meta.provider.is_empty()).then(|| meta.provider.clone()))
        .unwrap_or_else(|| settings.ai.active_provider.clone());
    let model_id = model
        .filter(|s| !s.is_empty())
        .or_else(|| (!meta.model.is_empty()).then(|| meta.model.clone()))
        .unwrap_or_else(|| settings.ai.active_model.clone());

    // Build history from persisted turns, then append the new user message
    // expanded with any attachment context (bounded to the project root).
    let mut messages: Vec<ChatMessage> = Vec::new();
    let turns = gwenland_engine::ai::conversation::load_turns(&conversation_id)
        .map_err(|e| e.to_string())?;
    for turn in turns {
        for m in turn.messages {
            messages.push(ChatMessage {
                role: m.role,
                content: m.content,
            });
        }
    }
    let expanded = gwenland_engine::ai::context::expand_message(
        &message,
        &attachments,
        std::path::Path::new(&meta.project_path),
    )
    .map_err(|e| e.to_string())?;

    // M13: retrieve memory block for provider-only injection (non-fatal; skipped on failure).
    // `expanded` (the JSONL-persisted form) is kept unchanged.
    let memory_block =
        retrieve_memory_block(&message, &meta.project_path, &provider_id, &model_id).await;
    let provider_user = match &memory_block {
        Some(block) => format!("{block}\n\n{expanded}"),
        None => expanded.clone(),
    };

    messages.push(ChatMessage::user(provider_user));

    let request = MessageRequest {
        stream_id: stream_id.clone(),
        messages,
        system: Some(compose_system_prompt(system_prefix.as_deref())),
        attachments,
        images,
        model: model_id.clone(),
        max_tokens: None,
    };

    let adapter = gwenland_engine::ai::registry::resolve_provider(&provider_id, &settings.ai)
        .map_err(|e| e.to_string())?;

    // Resolve conversation name for memory write-back (title → fallback to id).
    let conv_name_for_writeback = if meta.title.trim().is_empty() {
        conversation_id.clone()
    } else {
        meta.title.clone()
    };

    // Spawn the stream task, gated until its handle is registered so the task's
    // self-removal can never race ahead of the insert below.
    let streams = manager.active_streams.clone();
    let search_resolvers = manager.search_resolvers.clone();
    let sid = stream_id.clone();
    let (gate_tx, gate_rx) = tokio::sync::oneshot::channel::<()>();
    let join = tauri::async_runtime::spawn(async move {
        if gate_rx.await.is_err() {
            return;
        }
        run_stream(
            app,
            search_resolvers,
            adapter,
            request,
            expanded,
            provider_id,
            model_id,
            conversation_id,
            meta.project_path,
            conv_name_for_writeback,
        )
        .await;
        streams.lock().unwrap().remove(&sid);
    });

    manager
        .active_streams
        .lock()
        .map_err(|_| "ai manager lock poisoned".to_string())?
        .insert(stream_id.clone(), join);
    let _ = gate_tx.send(());

    Ok(stream_id)
}

/// Cancel an active stream. Aborts the matching task and emits a terminal
/// `ai://error` carrying `Cancelled` (the UI keeps partial text and shows no red
/// banner). Cancelling a missing/finished stream is a no-op success.
#[tauri::command]
fn ai_cancel(
    app: AppHandle,
    manager: State<'_, AiManager>,
    stream_id: String,
) -> Result<(), String> {
    let handle = manager
        .active_streams
        .lock()
        .map_err(|_| "ai manager lock poisoned".to_string())?
        .remove(&stream_id);
    if let Some(handle) = handle {
        handle.abort();
        emit_ai_error(&app, &stream_id, AiError::Cancelled);
    }
    Ok(())
}

/// M13 — Deliver a web search result to a parked stream so generation continues.
/// The stream task is waiting on a oneshot channel; we look it up by stream_id
/// and send the result text. If the channel is missing (stream cancelled / already
/// continued), this is a safe no-op.
#[tauri::command]
fn ai_search_result(
    manager: State<'_, AiManager>,
    stream_id: String,
    result_text: String,
) -> Result<(), String> {
    let sender = manager
        .search_resolvers
        .lock()
        .map_err(|_| "ai manager lock poisoned".to_string())?
        .remove(&stream_id);
    if let Some(tx) = sender {
        // Ignore send failure — stream may have been cancelled concurrently.
        let _ = tx.send(result_text);
    }
    Ok(())
}

/// One-shot, non-streaming, non-persisted completion (GWEN-324). Used for short
/// side-prompts like auto-naming a conversation — does NOT touch conversation
/// history. Returns the trimmed assistant text, or a stringified `AiError`.
#[tauri::command]
async fn ai_complete(
    prompt: String,
    provider: Option<String>,
    model: Option<String>,
) -> Result<String, String> {
    let settings = gwenland_engine::settings::load_settings().map_err(|e| e.to_string())?;
    let provider_id = provider
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| settings.ai.active_provider.clone());
    let model_id = model
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| settings.ai.active_model.clone());
    // Titles are tiny; cap the response so a misbehaving model can't run on.
    complete_once(None, prompt, &provider_id, &model_id, Some(64))
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Agentic Coding Workflow (Milestone 10)
//
// The human-gated plan -> approve -> edit -> validate -> summarize loop. All
// workflow/policy/parsing logic is engine-side (`gwenland_engine::agentic`,
// zero Tauri). This is the Tauri half: a managed `AgentManager` holding sessions
// and active stream handles, plus the `agent_*` commands and `agent://*` events.
//
// Provider keys NEVER live here — adapters fetch them from the OS keychain at
// send time, exactly like `ai_send`. Sessions hold only provider/model ids.
// ---------------------------------------------------------------------------

/// One in-flight agent stream, tagged with its owning session so a session-level
/// cancel can abort it.
struct AgentStream {
    session_id: String,
    handle: tauri::async_runtime::JoinHandle<()>,
}

/// Managed state: agent sessions by id, and active streams by stream id. Cloned
/// `Arc`s let spawned stream tasks update session state / self-remove.
///
/// Wave 7 adds the ReAct tool loops (`loops`, keyed by session) and the gated
/// tool awaiting user resolution (`pending`, keyed by session). Both are
/// in-memory only — no keys, no provider headers.
///
/// `cmd_pids` maps session_id → child PID for the currently-running agent
/// terminal command so `agent_kill_terminal` can kill it mid-flight.
#[derive(Default, Clone)]
struct AgentManager {
    sessions: Arc<Mutex<HashMap<String, AgentSession>>>,
    streams: Arc<Mutex<HashMap<String, AgentStream>>>,
    loops: Arc<Mutex<HashMap<String, gwenland_engine::agentic::AgentLoop>>>,
    pending: Arc<Mutex<HashMap<String, gwenland_engine::agentic::ToolCall>>>,
    cmd_pids: Arc<Mutex<HashMap<String, u32>>>,
}

impl AgentManager {
    fn store_session(&self, session: AgentSession) -> Result<(), String> {
        self.sessions
            .lock()
            .map_err(|_| "agent manager lock poisoned".to_string())?
            .insert(session.id.clone(), session);
        Ok(())
    }

    /// Clone-snapshot of a session (the source of truth stays in the map).
    fn snapshot(&self, session_id: &str) -> Result<AgentSession, String> {
        self.sessions
            .lock()
            .map_err(|_| "agent manager lock poisoned".to_string())?
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("no agent session {session_id}"))
    }

    /// Register an active stream, rejecting a duplicate id (Req 10/2.7).
    fn register_stream(
        &self,
        stream_id: String,
        session_id: String,
        handle: tauri::async_runtime::JoinHandle<()>,
    ) -> Result<(), String> {
        let mut streams = self
            .streams
            .lock()
            .map_err(|_| "agent manager lock poisoned".to_string())?;
        if streams.contains_key(&stream_id) {
            return Err(format!("stream {stream_id} is already active"));
        }
        streams.insert(stream_id, AgentStream { session_id, handle });
        Ok(())
    }

    /// Remove a stream entry by id (no abort). Returns it if present.
    fn remove_stream(&self, stream_id: &str) -> Option<AgentStream> {
        self.streams.lock().ok()?.remove(stream_id)
    }

    /// Abort and drop every stream belonging to `session_id`. Returns how many
    /// were aborted (0 is fine — cancelling with no active stream is safe).
    fn abort_session_streams(&self, session_id: &str) -> usize {
        let mut streams = match self.streams.lock() {
            Ok(s) => s,
            Err(_) => return 0,
        };
        let ids: Vec<String> = streams
            .iter()
            .filter(|(_, s)| s.session_id == session_id)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &ids {
            if let Some(stream) = streams.remove(id) {
                stream.handle.abort();
            }
        }
        ids.len()
    }
}

fn persist_agent_session(session: &AgentSession) -> Result<(), String> {
    gwenland_engine::agentic::persist_session(session)
}

// --- Agent event payloads --------------------------------------------------

/// `agent://chunk` — one streamed token fragment (same shape as `ai://chunk`).
#[derive(Clone, Serialize)]
struct AgentChunkEvent {
    stream_id: String,
    text: String,
}

/// `agent://phase` — a phase transition the UI mirrors into its step indicator.
#[derive(Clone, Serialize)]
struct AgentPhaseEvent {
    session_id: String,
    phase: AgentPhase,
}

/// `agent://error` — a recoverable, key-free error (reuses the M4 `AiError`,
/// which by construction never carries keys or Authorization headers).
#[derive(Clone, Serialize)]
struct AgentErrorEvent {
    session_id: String,
    stream_id: String,
    error: AiError,
}

fn emit_agent_phase(app: &AppHandle, session_id: &str, phase: AgentPhase) {
    let _ = app.emit(
        "agent://phase",
        AgentPhaseEvent {
            session_id: session_id.to_string(),
            phase,
        },
    );
}

fn emit_agent_error(app: &AppHandle, session_id: &str, stream_id: &str, error: AiError) {
    let _ = app.emit(
        "agent://error",
        AgentErrorEvent {
            session_id: session_id.to_string(),
            stream_id: stream_id.to_string(),
            error,
        },
    );
}

/// Best-effort phase transition on the stored session (ignores guard failures so
/// event/stream bookkeeping never panics).
fn transition_session(
    sessions: &Arc<Mutex<HashMap<String, AgentSession>>>,
    session_id: &str,
    to: AgentPhase,
) {
    if let Ok(mut guard) = sessions.lock()
        && let Some(session) = guard.get_mut(session_id)
    {
        let _ = session.transition(to);
    }
}

// --- Context preview assembly ----------------------------------------------

/// One editor selection passed from the UI for context.
#[derive(Deserialize)]
struct AgentSelectionInput {
    path: String,
    content: String,
}

/// Workspace state the UI offers as candidate context. Paths are absolute.
#[derive(Default, Deserialize)]
struct AgentContextInput {
    active_file: Option<String>,
    selection: Option<AgentSelectionInput>,
    #[serde(default)]
    open_tabs: Vec<String>,
}

fn context_kind_str(kind: ContextItemKind) -> &'static str {
    match kind {
        ContextItemKind::ActiveFile => "active_file",
        ContextItemKind::Selection => "selection",
        ContextItemKind::OpenTab => "open_tab",
        ContextItemKind::Diagnostic => "diagnostic",
        ContextItemKind::TerminalError => "terminal_error",
        ContextItemKind::File => "file",
        ContextItemKind::WorkspaceTree => "workspace_tree",
    }
}

fn path_label(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

/// Build a policy-filtered context preview from the UI-provided workspace state.
/// Every file candidate is checked against the secret/excluded denylists, scoped
/// to the workspace root, read as UTF-8 (binary rejected), and bounded by the
/// engine context budgets. Secret/oversized/binary candidates become omissions
/// with reasons (Requirement 3).
fn build_context_preview(root: &std::path::Path, input: &AgentContextInput) -> ContextPreview {
    use gwenland_engine::agentic::{MAX_CONTEXT_ITEMS, MAX_ITEM_BYTES, MAX_TOTAL_CONTEXT_BYTES};

    let mut preview = ContextPreview::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut running = 0usize;
    let mut counter = 0usize;
    let mut next_id = || {
        counter += 1;
        format!("ctx-{counter}")
    };

    // The selection is the most relevant context; include it first (no file read
    // — the UI supplied the text).
    if let Some(sel) = &input.selection
        && !sel.content.trim().is_empty()
    {
        let item = ContextItem::included(
            next_id(),
            ContextItemKind::Selection,
            Some(sel.path.clone()),
            format!("selection in {}", path_label(&sel.path)),
            Some(sel.content.clone()),
            "editor selection",
        );
        running += item.byte_len;
        preview.items.push(item);
    }

    // Candidate files: active file first, then the rest of the open tabs.
    let mut candidates: Vec<(ContextItemKind, &String)> = Vec::new();
    if let Some(active) = &input.active_file {
        candidates.push((ContextItemKind::ActiveFile, active));
    }
    for tab in &input.open_tabs {
        candidates.push((ContextItemKind::OpenTab, tab));
    }

    for (kind, path) in candidates {
        if !seen.insert(path.clone()) {
            continue; // de-dup (active file is usually also an open tab)
        }
        if preview.items.len() >= MAX_CONTEXT_ITEMS {
            break;
        }
        let label = path_label(path);
        // 1. Path-based policy (secret / generated-build folder).
        if let Some(reason) = gwenland_engine::agentic::omission_for_path(path) {
            preview
                .omitted
                .push(ContextOmission::new(path.clone(), label, reason));
            continue;
        }
        // 2. Workspace scope.
        if !gwenland_engine::agentic::is_within_workspace(std::path::Path::new(path), root) {
            preview.omitted.push(ContextOmission::new(
                path.clone(),
                label,
                OmissionReason::OutsideWorkspace,
            ));
            continue;
        }
        // 3. Read + UTF-8/binary + budget.
        match gwenland_engine::fs::read_file(std::path::Path::new(path)) {
            Ok(content) => {
                if content.len() > MAX_ITEM_BYTES
                    || running.saturating_add(content.len()) > MAX_TOTAL_CONTEXT_BYTES
                {
                    preview.omitted.push(ContextOmission::new(
                        path.clone(),
                        label,
                        OmissionReason::Oversized,
                    ));
                    continue;
                }
                let reason = if kind == ContextItemKind::ActiveFile {
                    "active editor file"
                } else {
                    "open tab"
                };
                let item = ContextItem::included(
                    next_id(),
                    kind,
                    Some(path.clone()),
                    label,
                    Some(content),
                    reason,
                );
                running += item.byte_len;
                preview.items.push(item);
            }
            Err(gwenland_engine::fs::FsError::BinaryFile) => {
                preview.omitted.push(ContextOmission::new(
                    path.clone(),
                    label,
                    OmissionReason::Binary,
                ));
            }
            Err(_) => {
                preview.omitted.push(ContextOmission::new(
                    path.clone(),
                    label,
                    OmissionReason::ReadError,
                ));
            }
        }
    }

    preview.recompute_total();
    preview
}

/// Render the included (and UI-selected) context items into the provider prompt
/// as deterministic `<context …>` blocks.
fn render_context_for_prompt(preview: &ContextPreview, selected: &[String]) -> String {
    let mut blocks: Vec<String> = Vec::new();
    for item in &preview.items {
        if !item.included {
            continue;
        }
        if !selected.is_empty() && !selected.contains(&item.id) {
            continue;
        }
        let kind = context_kind_str(item.kind);
        let header = match &item.path {
            Some(p) => format!("<context kind=\"{kind}\" path=\"{p}\">"),
            None => format!("<context kind=\"{kind}\">"),
        };
        let body = item.content.clone().unwrap_or_default();
        blocks.push(format!("{header}\n{body}\n</context>"));
    }
    blocks.join("\n\n")
}

/// Normalize a model-proposed path to a project-relative path, rejecting paths
/// outside the workspace and secret/generated targets. This happens before the
/// UI can approve hunks, so blocked paths never become applyable ChangeSet rows.
fn normalize_agent_path(root: &std::path::Path, path: &str) -> Result<String, String> {
    if gwenland_engine::agentic::is_secret_path(path) {
        return Err("path matches a secret pattern".to_string());
    }
    if gwenland_engine::agentic::is_excluded_path(path) {
        return Err("path is in an excluded generated/dependency folder".to_string());
    }

    let raw = std::path::Path::new(path);
    let candidate = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        root.join(raw)
    };
    let resolved = gwenland_engine::agentic::canonical_within_workspace(&candidate, root)
        .map_err(|e| e.to_string())?;
    let root = root
        .canonicalize()
        .map_err(|_| "cannot resolve workspace root".to_string())?;
    let rel = resolved
        .strip_prefix(&root)
        .map_err(|_| "path is outside the workspace root".to_string())?;
    let normalized = rel.to_string_lossy().replace('\\', "/");
    if normalized.trim().is_empty() {
        Err("workspace root itself is not an edit target".to_string())
    } else {
        Ok(normalized)
    }
}

/// Validate and normalize every path in a ChangeSet. Invalid file entries are
/// dropped with parse warnings, preserving the raw streamed text for revision.
fn normalize_change_set_paths(root: &std::path::Path, mut change_set: ChangeSet) -> ChangeSet {
    let mut kept = Vec::new();
    for mut file in change_set.files.into_iter() {
        let label = file
            .new_path
            .as_deref()
            .or(file.old_path.as_deref())
            .unwrap_or("(unknown file)")
            .to_string();
        let mut rejected: Option<String> = None;

        if let Some(path) = file.old_path.clone() {
            match normalize_agent_path(root, &path) {
                Ok(path) => file.old_path = Some(path),
                Err(e) => rejected = Some(format!("Skipped `{label}`: {e}")),
            }
        }
        if rejected.is_none()
            && let Some(path) = file.new_path.clone()
        {
            match normalize_agent_path(root, &path) {
                Ok(path) => file.new_path = Some(path),
                Err(e) => rejected = Some(format!("Skipped `{label}`: {e}")),
            }
        }
        if rejected.is_none() && file.old_path.is_none() && file.new_path.is_none() {
            rejected = Some("Skipped a diff block with no file path.".to_string());
        }

        if let Some(warning) = rejected {
            change_set.parse_warnings.push(warning);
        } else {
            kept.push(file);
        }
    }

    change_set.files = kept;
    if change_set.files.is_empty() && change_set.parse_warnings.is_empty() {
        change_set.parse_warnings.push(
            "No applyable changes were found in the response. Ask for a revision with unified diffs."
                .to_string(),
        );
    }
    change_set
}

fn approved_hunk_ids(file: &ProposedFileChange) -> Vec<String> {
    file.hunks
        .iter()
        .filter(|h| h.approval == ApprovalState::Approved)
        .map(|h| h.id.clone())
        .collect()
}

fn rejected_hunk_ids(file: &ProposedFileChange) -> Vec<String> {
    file.hunks
        .iter()
        .filter(|h| h.approval == ApprovalState::Rejected)
        .map(|h| h.id.clone())
        .collect()
}

fn apply_outcome(
    file: &ProposedFileChange,
    path: String,
    hunk_ids: Vec<String>,
    message: String,
) -> ApplyOutcome {
    ApplyOutcome {
        file_id: file.id.clone(),
        path,
        hunk_ids,
        message,
    }
}

fn absolute_agent_path(
    root: &std::path::Path,
    relative: &str,
) -> Result<std::path::PathBuf, String> {
    if gwenland_engine::agentic::is_secret_path(relative)
        || gwenland_engine::agentic::is_excluded_path(relative)
    {
        return Err("path is blocked by agent safety policy".to_string());
    }
    let candidate = if std::path::Path::new(relative).is_absolute() {
        std::path::PathBuf::from(relative)
    } else {
        root.join(relative)
    };
    gwenland_engine::agentic::canonical_within_workspace(&candidate, root)
        .map_err(|e| e.to_string())
}

fn apply_file_change(
    root: &std::path::Path,
    file: &ProposedFileChange,
    destructive_confirmed: bool,
) -> Result<ApplyOutcome, String> {
    let hunk_ids = approved_hunk_ids(file);
    let target = file
        .target_path()
        .ok_or_else(|| "file change has no target path".to_string())?
        .to_string();

    match file.change_kind {
        FileChangeKind::Modify => {
            let path = absolute_agent_path(root, &target)?;
            let current = gwenland_engine::fs::read_file(&path).map_err(|e| e.to_string())?;
            let updated = gwenland_engine::agentic::apply_approved_hunks_to_text(&current, file)?;
            gwenland_engine::fs::write_file(&path, &updated).map_err(|e| e.to_string())?;
            Ok(apply_outcome(
                file,
                target,
                hunk_ids,
                "applied approved hunks".to_string(),
            ))
        }
        FileChangeKind::Create => {
            let path = absolute_agent_path(root, &target)?;
            if path.exists() {
                return Err("create target already exists".to_string());
            }
            let updated = gwenland_engine::agentic::apply_approved_hunks_to_text("", file)?;
            gwenland_engine::fs::write_file(&path, &updated).map_err(|e| e.to_string())?;
            Ok(apply_outcome(
                file,
                target,
                hunk_ids,
                "created file from approved hunks".to_string(),
            ))
        }
        FileChangeKind::Delete => {
            if !destructive_confirmed {
                return Err("delete requires explicit confirmation".to_string());
            }
            let path = absolute_agent_path(root, &target)?;
            let current = gwenland_engine::fs::read_file(&path).map_err(|e| e.to_string())?;
            let updated = gwenland_engine::agentic::apply_approved_hunks_to_text(&current, file)?;
            if !updated.trim().is_empty() {
                return Err("delete diff did not reduce the file to empty content".to_string());
            }
            gwenland_engine::recovery::move_to_trash(&path, root, "agent")
                .map_err(|e| e.to_string())?;
            Ok(apply_outcome(
                file,
                target,
                hunk_ids,
                "moved file to workspace trash after confirmed review".to_string(),
            ))
        }
        FileChangeKind::Rename => {
            Err("rename apply is deferred until a dedicated confirmation flow".to_string())
        }
    }
}

fn resolve_validation_cwd(root: &std::path::Path, cwd: &str) -> Result<std::path::PathBuf, String> {
    let root = root
        .canonicalize()
        .map_err(|_| "cannot resolve workspace root".to_string())?;
    let candidate = if cwd.trim().is_empty() || cwd == "." {
        root.clone()
    } else if std::path::Path::new(cwd).is_absolute() {
        std::path::PathBuf::from(cwd)
    } else {
        root.join(cwd)
    };
    let resolved = candidate
        .canonicalize()
        .map_err(|_| "cannot resolve validation working directory".to_string())?;
    if (resolved == root || resolved.starts_with(&root)) && resolved.is_dir() {
        Ok(resolved)
    } else {
        Err("validation working directory is outside the workspace".to_string())
    }
}

fn truncate_redacted_output(output: &[u8]) -> String {
    const MAX: usize = 8 * 1024;
    let mut text = String::from_utf8_lossy(output).to_string();
    if text.len() > MAX {
        let start = text.len().saturating_sub(MAX);
        text = format!("...[truncated]\n{}", &text[start..]);
    }
    gwenland_engine::agentic::redact_secrets(&text).0
}

fn run_validation_command_blocking(
    command: &str,
    cwd: &std::path::Path,
) -> Result<(i32, String), String> {
    #[cfg(target_os = "windows")]
    let output = {
        let mut cmd = std::process::Command::new("powershell");
        cmd.arg("-NoProfile")
            .arg("-Command")
            .arg(command)
            .current_dir(cwd);
        hide_child_window(&mut cmd);
        cmd.output().map_err(|e| e.to_string())?
    };

    #[cfg(not(target_os = "windows"))]
    let output = {
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg(command).current_dir(cwd);
        cmd.output().map_err(|e| e.to_string())?
    };

    let mut combined = output.stdout;
    if !output.stderr.is_empty() {
        combined.extend_from_slice(b"\n");
        combined.extend_from_slice(&output.stderr);
    }
    Ok((
        output.status.code().unwrap_or(-1),
        truncate_redacted_output(&combined),
    ))
}

// --- Agent commands --------------------------------------------------------

/// Create a session in the `Goal` phase. Resolves provider/model with M4 rules:
/// explicit override, else the global active provider/model. Keys are not read.
#[tauri::command]
fn agent_create_session(
    manager: State<'_, AgentManager>,
    project_root: String,
    goal: String,
    provider: Option<String>,
    model: Option<String>,
    tier: Option<gwenland_engine::agentic::AgentTier>,
) -> Result<AgentSession, String> {
    let settings = gwenland_engine::settings::load_settings().map_err(|e| e.to_string())?;
    let provider_id = provider
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| settings.ai.active_provider.clone());
    let model_id = model
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| settings.ai.active_model.clone());

    let mut session = AgentSession::new(
        gwenland_engine::agentic::new_id(),
        project_root,
        goal,
        provider_id,
        model_id,
        ContextPreview::new(),
    );
    if let Some(tier) = tier {
        session.tier = tier;
    }
    manager.store_session(session.clone())?;
    persist_agent_session(&session)?;
    Ok(session)
}

/// Change a session's autonomy tier (M10 Wave 8). Allowed only between
/// iterations (not mid-stream/apply/validation); returns the updated session.
#[tauri::command]
fn agent_set_tier(
    manager: State<'_, AgentManager>,
    session_id: String,
    tier: gwenland_engine::agentic::AgentTier,
) -> Result<AgentSession, String> {
    let mut sessions = manager
        .sessions
        .lock()
        .map_err(|_| "agent manager lock poisoned".to_string())?;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| format!("no agent session {session_id}"))?;
    if !session.set_tier(tier) {
        return Err("tier can only change between iterations".to_string());
    }
    let snapshot = session.clone();
    drop(sessions);
    persist_agent_session(&snapshot)?;
    Ok(snapshot)
}

/// Build a policy-filtered context preview for a session and store it. The UI
/// passes current workspace state (active file, selection, open tabs).
#[tauri::command]
fn agent_context_preview(
    manager: State<'_, AgentManager>,
    session_id: String,
    input: AgentContextInput,
) -> Result<ContextPreview, String> {
    let root = {
        let session = manager.snapshot(&session_id)?;
        session.project_root
    };
    let preview = build_context_preview(std::path::Path::new(&root), &input);

    // Store the preview back on the session.
    let snapshot = {
        let mut guard = manager
            .sessions
            .lock()
            .map_err(|_| "agent manager lock poisoned".to_string())?;
        if let Some(session) = guard.get_mut(&session_id) {
            session.context = preview.clone();
            Some(session.clone())
        } else {
            None
        }
    };
    if let Some(session) = snapshot {
        persist_agent_session(&session)?;
    }
    Ok(preview)
}

/// Request a plan. Requires a planning-eligible phase, builds a provider-neutral
/// request from the goal + selected context, streams `agent://chunk`, and stores
/// the normalized plan on completion (moving to `AwaitingPlanApproval`).
#[tauri::command]
fn agent_request_plan(
    app: AppHandle,
    manager: State<'_, AgentManager>,
    session_id: String,
    stream_id: String,
    context_item_ids: Vec<String>,
) -> Result<String, String> {
    // Reject a duplicate active stream id up front.
    if manager
        .streams
        .lock()
        .map_err(|_| "agent manager lock poisoned".to_string())?
        .contains_key(&stream_id)
    {
        return Err(format!("stream {stream_id} is already active"));
    }

    let session = manager.snapshot(&session_id)?;
    if !matches!(
        session.phase,
        AgentPhase::Goal | AgentPhase::AwaitingPlanApproval
    ) {
        return Err(format!(
            "cannot request a plan from phase {:?}",
            session.phase
        ));
    }

    let settings = gwenland_engine::settings::load_settings().map_err(|e| e.to_string())?;
    let adapter = gwenland_engine::ai::registry::resolve_provider(&session.provider, &settings.ai)
        .map_err(|e| e.to_string())?;

    let context_summary = render_context_for_prompt(&session.context, &context_item_ids);
    let user_prompt =
        gwenland_engine::agentic::build_plan_user_prompt(&session.goal, &context_summary);
    let plan_id = gwenland_engine::agentic::new_id();

    let request = MessageRequest {
        stream_id: stream_id.clone(),
        messages: vec![ChatMessage::user(user_prompt)],
        system: Some(gwenland_engine::agentic::PLAN_SYSTEM.to_string()),
        attachments: Vec::new(),
        images: Vec::new(),
        model: session.model.clone(),
        max_tokens: None,
    };

    // Move to DraftingPlan and announce it before the stream starts.
    transition_session(&manager.sessions, &session_id, AgentPhase::DraftingPlan);
    emit_agent_phase(&app, &session_id, AgentPhase::DraftingPlan);

    let agent = (*manager).clone();
    let sessions = agent.sessions.clone();
    let sid = session_id.clone();
    let stream_key = stream_id.clone();
    let (gate_tx, gate_rx) = tokio::sync::oneshot::channel::<()>();
    let join = tauri::async_runtime::spawn(async move {
        if gate_rx.await.is_err() {
            return;
        }
        run_plan_stream(
            app,
            sessions,
            adapter,
            request,
            sid,
            stream_key.clone(),
            plan_id,
        )
        .await;
        // Self-remove the finished stream (idempotent with a concurrent cancel).
        agent.remove_stream(&stream_key);
    });

    manager.register_stream(stream_id.clone(), session_id, join)?;
    let _ = gate_tx.send(());
    Ok(stream_id)
}

/// Drive one plan stream: emit each token, then parse + store the plan and move
/// to `AwaitingPlanApproval`. On failure, revert to `Goal` so the user can retry
/// (the partial text stays visible in the UI). Never persists keys.
async fn run_plan_stream(
    app: AppHandle,
    sessions: Arc<Mutex<HashMap<String, AgentSession>>>,
    adapter: Box<dyn AiProvider>,
    request: MessageRequest,
    session_id: String,
    stream_id: String,
    plan_id: String,
) {
    let mut stream = match adapter.send_message(request).await {
        Ok(s) => s,
        Err(e) => {
            transition_session(&sessions, &session_id, AgentPhase::Goal);
            emit_agent_phase(&app, &session_id, AgentPhase::Goal);
            emit_agent_error(&app, &session_id, &stream_id, e);
            return;
        }
    };

    let mut text = String::new();
    loop {
        match stream.next_chunk().await {
            Ok(Some(chunk)) => {
                text.push_str(&chunk.text);
                let _ = app.emit(
                    "agent://chunk",
                    AgentChunkEvent {
                        stream_id: stream_id.clone(),
                        text: chunk.text,
                    },
                );
            }
            Ok(None) => {
                let plan = gwenland_engine::agentic::parse_plan(&plan_id, &text);
                let mut snapshot = None;
                if let Ok(mut guard) = sessions.lock()
                    && let Some(session) = guard.get_mut(&session_id)
                {
                    session.plan = Some(plan);
                    let _ = session.transition(AgentPhase::AwaitingPlanApproval);
                    snapshot = Some(session.clone());
                }
                if let Some(session) = snapshot {
                    let _ = persist_agent_session(&session);
                }
                emit_agent_phase(&app, &session_id, AgentPhase::AwaitingPlanApproval);
                return;
            }
            Err(e) => {
                transition_session(&sessions, &session_id, AgentPhase::Goal);
                emit_agent_phase(&app, &session_id, AgentPhase::Goal);
                emit_agent_error(&app, &session_id, &stream_id, e);
                return;
            }
        }
    }
}

/// Request proposed edits for the approved plan. This streams assistant text,
/// parses the final response into a ChangeSet, and stops at review. No file
/// writes happen in Wave 4.
#[tauri::command]
fn agent_request_edits(
    app: AppHandle,
    manager: State<'_, AgentManager>,
    session_id: String,
    stream_id: String,
    context_item_ids: Vec<String>,
) -> Result<String, String> {
    if manager
        .streams
        .lock()
        .map_err(|_| "agent manager lock poisoned".to_string())?
        .contains_key(&stream_id)
    {
        return Err(format!("stream {stream_id} is already active"));
    }

    let session = manager.snapshot(&session_id)?;
    let plan = session
        .plan
        .clone()
        .ok_or_else(|| "session has no approved plan to edit from".to_string())?;
    session
        .can_transition(AgentPhase::DraftingEdits)
        .map_err(|e| e.to_string())?;

    let settings = gwenland_engine::settings::load_settings().map_err(|e| e.to_string())?;
    let adapter = gwenland_engine::ai::registry::resolve_provider(&session.provider, &settings.ai)
        .map_err(|e| e.to_string())?;

    let context_summary = render_context_for_prompt(&session.context, &context_item_ids);
    let user_prompt = gwenland_engine::agentic::build_edit_user_prompt(&plan, &context_summary);
    let request = MessageRequest {
        stream_id: stream_id.clone(),
        messages: vec![ChatMessage::user(user_prompt)],
        system: Some(gwenland_engine::agentic::EDIT_SYSTEM.to_string()),
        attachments: Vec::new(),
        images: Vec::new(),
        model: session.model.clone(),
        max_tokens: None,
    };

    {
        let mut guard = manager
            .sessions
            .lock()
            .map_err(|_| "agent manager lock poisoned".to_string())?;
        let stored = guard
            .get_mut(&session_id)
            .ok_or_else(|| format!("no agent session {session_id}"))?;
        stored
            .transition(AgentPhase::DraftingEdits)
            .map_err(|e| e.to_string())?;
    }
    emit_agent_phase(&app, &session_id, AgentPhase::DraftingEdits);

    let agent = (*manager).clone();
    let sessions = agent.sessions.clone();
    let sid = session_id.clone();
    let stream_key = stream_id.clone();
    let root = session.project_root.clone();
    let plan_id = plan.id.clone();
    let (gate_tx, gate_rx) = tokio::sync::oneshot::channel::<()>();
    let join = tauri::async_runtime::spawn(async move {
        if gate_rx.await.is_err() {
            return;
        }
        run_edit_stream(
            app,
            sessions,
            adapter,
            request,
            sid,
            stream_key.clone(),
            plan_id,
            root,
        )
        .await;
        agent.remove_stream(&stream_key);
    });

    manager.register_stream(stream_id.clone(), session_id, join)?;
    let _ = gate_tx.send(());
    Ok(stream_id)
}

/// Drive one edit stream: emit each token, parse the complete response into a
/// workspace-safe ChangeSet, then move to review. The streamed raw text remains
/// visible in the frontend buffer; only parsed ChangeSets become review state.
#[allow(clippy::too_many_arguments)]
async fn run_edit_stream(
    app: AppHandle,
    sessions: Arc<Mutex<HashMap<String, AgentSession>>>,
    adapter: Box<dyn AiProvider>,
    request: MessageRequest,
    session_id: String,
    stream_id: String,
    plan_id: String,
    project_root: String,
) {
    let mut stream = match adapter.send_message(request).await {
        Ok(s) => s,
        Err(e) => {
            transition_session(&sessions, &session_id, AgentPhase::AwaitingPlanApproval);
            emit_agent_phase(&app, &session_id, AgentPhase::AwaitingPlanApproval);
            emit_agent_error(&app, &session_id, &stream_id, e);
            return;
        }
    };

    let mut text = String::new();
    loop {
        match stream.next_chunk().await {
            Ok(Some(chunk)) => {
                text.push_str(&chunk.text);
                let _ = app.emit(
                    "agent://chunk",
                    AgentChunkEvent {
                        stream_id: stream_id.clone(),
                        text: chunk.text,
                    },
                );
            }
            Ok(None) => {
                let change_set = gwenland_engine::agentic::change_set_from_text(&plan_id, &text);
                let change_set =
                    normalize_change_set_paths(std::path::Path::new(&project_root), change_set);
                let mut snapshot = None;
                if let Ok(mut guard) = sessions.lock()
                    && let Some(session) = guard.get_mut(&session_id)
                {
                    session.change_sets.push(change_set);
                    let _ = session.transition(AgentPhase::AwaitingEditApproval);
                    snapshot = Some(session.clone());
                }
                if let Some(session) = snapshot {
                    let _ = persist_agent_session(&session);
                }
                emit_agent_phase(&app, &session_id, AgentPhase::AwaitingEditApproval);
                return;
            }
            Err(e) => {
                transition_session(&sessions, &session_id, AgentPhase::AwaitingPlanApproval);
                emit_agent_phase(&app, &session_id, AgentPhase::AwaitingPlanApproval);
                emit_agent_error(&app, &session_id, &stream_id, e);
                return;
            }
        }
    }
}

/// Approve the current plan, recording a one-use, session-scoped approval that
/// unlocks edit generation (Wave 4). Requires `AwaitingPlanApproval` and a
/// matching plan id.
#[tauri::command]
fn agent_approve_plan(
    manager: State<'_, AgentManager>,
    session_id: String,
    plan_id: String,
) -> Result<ApprovalRecord, String> {
    let (record, snapshot) = {
        let mut guard = manager
            .sessions
            .lock()
            .map_err(|_| "agent manager lock poisoned".to_string())?;
        let session = guard
            .get_mut(&session_id)
            .ok_or_else(|| format!("no agent session {session_id}"))?;

        if session.phase != AgentPhase::AwaitingPlanApproval {
            return Err(format!(
                "cannot approve a plan from phase {:?}",
                session.phase
            ));
        }
        match &session.plan {
            Some(plan) if plan.id == plan_id => {}
            Some(_) => return Err("plan id does not match the current plan".to_string()),
            None => return Err("session has no plan to approve".to_string()),
        }
        let record = session.record_approval(ApprovalKind::Plan, plan_id);
        (record, session.clone())
    };
    persist_agent_session(&snapshot)?;
    Ok(record)
}

/// Read-only snapshot of a session (its phase, plan, context, approvals, …). The
/// UI calls this after a stream completes to pick up the normalized plan, and on
/// reload to restore resumable sessions. Never returns keys.
/// Review-only hunk approval. This mutates ChangeSet state but does not write
/// files; Wave 5 consumes approved hunks during apply.
#[tauri::command]
fn agent_set_hunk_approval(
    manager: State<'_, AgentManager>,
    session_id: String,
    change_set_id: String,
    hunk_id: String,
    approval: ApprovalState,
) -> Result<AgentSession, String> {
    let mut guard = manager
        .sessions
        .lock()
        .map_err(|_| "agent manager lock poisoned".to_string())?;
    let session = guard
        .get_mut(&session_id)
        .ok_or_else(|| format!("no agent session {session_id}"))?;
    if session.phase != AgentPhase::AwaitingEditApproval {
        return Err(format!(
            "cannot approve edit hunks from phase {:?}",
            session.phase
        ));
    }
    let change_set = session
        .change_sets
        .iter_mut()
        .find(|cs| cs.id == change_set_id)
        .ok_or_else(|| "change set not found".to_string())?;
    if !change_set.set_hunk_approval(&hunk_id, approval) {
        return Err("hunk not found".to_string());
    }
    let snapshot = session.clone();
    drop(guard);
    persist_agent_session(&snapshot)?;
    Ok(snapshot)
}

/// Review-only file approval. Sets every hunk in one proposed file to the same
/// state. Does not write files.
#[tauri::command]
fn agent_set_file_approval(
    manager: State<'_, AgentManager>,
    session_id: String,
    change_set_id: String,
    file_id: String,
    approval: ApprovalState,
) -> Result<AgentSession, String> {
    let mut guard = manager
        .sessions
        .lock()
        .map_err(|_| "agent manager lock poisoned".to_string())?;
    let session = guard
        .get_mut(&session_id)
        .ok_or_else(|| format!("no agent session {session_id}"))?;
    if session.phase != AgentPhase::AwaitingEditApproval {
        return Err(format!(
            "cannot approve edit files from phase {:?}",
            session.phase
        ));
    }
    let change_set = session
        .change_sets
        .iter_mut()
        .find(|cs| cs.id == change_set_id)
        .ok_or_else(|| "change set not found".to_string())?;
    if !change_set.set_file_approval(&file_id, approval) {
        return Err("file change not found".to_string());
    }
    let snapshot = session.clone();
    drop(guard);
    persist_agent_session(&snapshot)?;
    Ok(snapshot)
}

/// Apply only approved hunks/files. Re-checks workspace boundaries and hunk
/// context before writing. Destructive changes require an explicit UI
/// confirmation flag.
#[tauri::command]
fn agent_apply_changes(
    app: AppHandle,
    manager: State<'_, AgentManager>,
    session_id: String,
    destructive_confirmed: bool,
) -> Result<AgentSession, String> {
    let session = manager.snapshot(&session_id)?;
    if session.phase != AgentPhase::AwaitingEditApproval {
        return Err(format!(
            "cannot apply changes from phase {:?}",
            session.phase
        ));
    }
    let change_set = session
        .latest_change_set()
        .cloned()
        .ok_or_else(|| "session has no change set to apply".to_string())?;
    if !change_set.has_approved_change() {
        return Err("at least one hunk or file must be approved before apply".to_string());
    }

    {
        let mut guard = manager
            .sessions
            .lock()
            .map_err(|_| "agent manager lock poisoned".to_string())?;
        let stored = guard
            .get_mut(&session_id)
            .ok_or_else(|| format!("no agent session {session_id}"))?;
        stored
            .transition(AgentPhase::ApplyingApprovedEdits)
            .map_err(|e| e.to_string())?;
    }
    emit_agent_phase(&app, &session_id, AgentPhase::ApplyingApprovedEdits);

    let root = std::path::Path::new(&session.project_root);
    let mut report = ApplyReport::default();
    for file in &change_set.files {
        let path = file.target_path().unwrap_or("(unknown file)").to_string();
        let approved_ids = approved_hunk_ids(file);
        let rejected_ids = rejected_hunk_ids(file);
        if approved_ids.is_empty() {
            if !rejected_ids.is_empty() {
                report.rejected.push(apply_outcome(
                    file,
                    path,
                    rejected_ids,
                    "all reviewed hunks were rejected".to_string(),
                ));
            } else {
                report.skipped.push(apply_outcome(
                    file,
                    path,
                    Vec::new(),
                    "no approved hunks".to_string(),
                ));
            }
            continue;
        }

        match apply_file_change(root, file, destructive_confirmed) {
            Ok(outcome) => report.applied.push(outcome),
            Err(message) => report
                .failed
                .push(apply_outcome(file, path, approved_ids, message)),
        }
    }

    let next_phase = if report.applied.is_empty() {
        AgentPhase::AwaitingEditApproval
    } else {
        AgentPhase::AwaitingValidationApproval
    };
    let snapshot = {
        let mut guard = manager
            .sessions
            .lock()
            .map_err(|_| "agent manager lock poisoned".to_string())?;
        let stored = guard
            .get_mut(&session_id)
            .ok_or_else(|| format!("no agent session {session_id}"))?;
        stored.apply_report = Some(report);
        stored.transition(next_phase).map_err(|e| e.to_string())?;
        stored.clone()
    };
    persist_agent_session(&snapshot)?;
    emit_agent_phase(&app, &session_id, next_phase);
    Ok(snapshot)
}

/// Record a one-use approval for a validation command after risk/cwd checks.
/// Dependency-changing commands require a size-impact note; destructive
/// commands require explicit danger confirmation; blocked commands never pass.
#[tauri::command]
fn agent_approve_validation_command(
    manager: State<'_, AgentManager>,
    session_id: String,
    command_id: String,
    size_impact_note: Option<String>,
    danger_confirmed: bool,
) -> Result<ApprovalRecord, String> {
    let (record, snapshot) = {
        let mut guard = manager
            .sessions
            .lock()
            .map_err(|_| "agent manager lock poisoned".to_string())?;
        let session = guard
            .get_mut(&session_id)
            .ok_or_else(|| format!("no agent session {session_id}"))?;
        if session.phase != AgentPhase::AwaitingValidationApproval {
            return Err(format!(
                "cannot approve validation from phase {:?}",
                session.phase
            ));
        }
        let root = std::path::PathBuf::from(&session.project_root);
        let plan = session
            .plan
            .as_mut()
            .ok_or_else(|| "session has no plan validation commands".to_string())?;
        let command = plan
            .suggested_validation
            .iter_mut()
            .find(|c| c.id == command_id)
            .ok_or_else(|| "validation command not found".to_string())?;

        command.risk = gwenland_engine::agentic::classify_command(&command.command);
        if let Some(note) = size_impact_note
            && !note.trim().is_empty()
        {
            command.size_impact_note = Some(note);
        }
        resolve_validation_cwd(&root, &command.cwd)?;
        if command.risk.is_blocked() {
            return Err("blocked validation commands cannot be approved".to_string());
        }
        if command.risk.requires_size_impact_note()
            && command
                .size_impact_note
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
        {
            return Err("dependency-changing commands require a size-impact note".to_string());
        }
        if command.risk.requires_danger_confirmation() && !danger_confirmed {
            return Err("destructive commands require explicit confirmation".to_string());
        }
        let record = session.record_approval(ApprovalKind::ValidationCommand, command_id);
        (record, session.clone())
    };
    persist_agent_session(&snapshot)?;
    Ok(record)
}

/// Run one approved validation command and store a bounded, redacted output
/// excerpt. The approval is consumed before execution so it cannot be replayed.
#[tauri::command]
async fn agent_run_validation(
    app: AppHandle,
    manager: State<'_, AgentManager>,
    session_id: String,
    command_id: String,
    approval_id: String,
) -> Result<AgentSession, String> {
    let (command, cwd, run_id, started_snapshot) = {
        let mut guard = manager
            .sessions
            .lock()
            .map_err(|_| "agent manager lock poisoned".to_string())?;
        let session = guard
            .get_mut(&session_id)
            .ok_or_else(|| format!("no agent session {session_id}"))?;
        if session.phase != AgentPhase::AwaitingValidationApproval {
            return Err(format!(
                "cannot run validation from phase {:?}",
                session.phase
            ));
        }
        let approval_index = session
            .approvals
            .iter()
            .position(|a| {
                a.id == approval_id
                    && a.kind == ApprovalKind::ValidationCommand
                    && a.target_id == command_id
                    && !a.consumed
            })
            .ok_or_else(|| "validation approval is missing or already consumed".to_string())?;

        let command = session
            .plan
            .as_ref()
            .and_then(|p| p.suggested_validation.iter().find(|c| c.id == command_id))
            .cloned()
            .ok_or_else(|| "validation command not found".to_string())?;
        if gwenland_engine::agentic::classify_command(&command.command) != command.risk {
            return Err("validation command risk changed before run".to_string());
        }
        let cwd =
            resolve_validation_cwd(std::path::Path::new(&session.project_root), &command.cwd)?;
        session
            .transition(AgentPhase::Validating)
            .map_err(|e| e.to_string())?;
        session.approvals[approval_index].consumed = true;
        let run_id = gwenland_engine::agentic::new_id();
        let now = unix_timestamp_string();
        session.validation_runs.push(ValidationRun::started(
            run_id.clone(),
            command_id.clone(),
            now,
        ));
        (command, cwd, run_id, session.clone())
    };
    persist_agent_session(&started_snapshot)?;
    emit_agent_phase(&app, &session_id, AgentPhase::Validating);

    let command_line = command.command.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        run_validation_command_blocking(&command_line, &cwd)
    })
    .await
    .map_err(|e| e.to_string())?;

    let snapshot = {
        let mut guard = manager
            .sessions
            .lock()
            .map_err(|_| "agent manager lock poisoned".to_string())?;
        let session = guard
            .get_mut(&session_id)
            .ok_or_else(|| format!("no agent session {session_id}"))?;
        let run = session
            .validation_runs
            .iter_mut()
            .find(|r| r.id == run_id)
            .ok_or_else(|| "validation run not found".to_string())?;
        match result {
            Ok((code, output)) => {
                run.exit_code = Some(code);
                run.output_excerpt = output;
                run.status = if code == 0 {
                    ValidationStatus::Passed
                } else {
                    ValidationStatus::Failed
                };
            }
            Err(message) => {
                run.exit_code = None;
                run.output_excerpt = gwenland_engine::agentic::redact_secrets(&message).0;
                run.status = ValidationStatus::Blocked;
            }
        }
        run.finished_at = Some(unix_timestamp_string());
        session
            .transition(AgentPhase::AwaitingValidationApproval)
            .map_err(|e| e.to_string())?;
        session.clone()
    };
    persist_agent_session(&snapshot)?;
    emit_agent_phase(&app, &session_id, AgentPhase::AwaitingValidationApproval);
    Ok(snapshot)
}

fn unix_timestamp_string() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}

#[tauri::command]
fn agent_summarize(
    app: AppHandle,
    manager: State<'_, AgentManager>,
    session_id: String,
) -> Result<AgentSession, String> {
    let (summary, summarizing_snapshot) = {
        let mut guard = manager
            .sessions
            .lock()
            .map_err(|_| "agent manager lock poisoned".to_string())?;
        let session = guard
            .get_mut(&session_id)
            .ok_or_else(|| format!("no agent session {session_id}"))?;
        if session.phase == AgentPhase::Complete {
            return Ok(session.clone());
        }

        let plan = session
            .plan
            .clone()
            .ok_or_else(|| "session has no approved plan to summarize".to_string())?;
        let report = session.apply_report.clone().unwrap_or_default();
        let mut unresolved_risks = plan.risks.clone();
        unresolved_risks.extend(
            report
                .failed
                .iter()
                .map(|o| format!("Apply failed for {}: {}", o.path, o.message)),
        );
        unresolved_risks.extend(session.validation_runs.iter().filter_map(
            |run| match run.status {
                ValidationStatus::Failed => Some(format!("Validation failed: {}", run.command_id)),
                ValidationStatus::Blocked => {
                    Some(format!("Validation blocked: {}", run.command_id))
                }
                _ => None,
            },
        ));

        session
            .transition(AgentPhase::Summarizing)
            .map_err(|e| e.to_string())?;
        let summary = gwenland_engine::agentic::build_local_summary(
            gwenland_engine::agentic::new_id(),
            &session.goal,
            &plan.title,
            &report,
            &session.validation_runs,
            unresolved_risks,
        );
        (summary, session.clone())
    };
    persist_agent_session(&summarizing_snapshot)?;
    emit_agent_phase(&app, &session_id, AgentPhase::Summarizing);

    let snapshot = {
        let mut guard = manager
            .sessions
            .lock()
            .map_err(|_| "agent manager lock poisoned".to_string())?;
        let session = guard
            .get_mut(&session_id)
            .ok_or_else(|| format!("no agent session {session_id}"))?;
        session.summary = Some(summary);
        session
            .transition(AgentPhase::Complete)
            .map_err(|e| e.to_string())?;
        session.clone()
    };
    persist_agent_session(&snapshot)?;
    emit_agent_phase(&app, &session_id, AgentPhase::Complete);
    Ok(snapshot)
}

#[tauri::command]
fn agent_restore_sessions(
    manager: State<'_, AgentManager>,
    project_root: Option<String>,
) -> Result<Vec<AgentSession>, String> {
    let sessions =
        gwenland_engine::agentic::load_sessions(project_root.as_deref().map(std::path::Path::new))?;
    for session in &sessions {
        manager.store_session(session.clone())?;
        persist_agent_session(session)?;
    }
    Ok(sessions)
}

#[tauri::command]
fn agent_get_session(
    manager: State<'_, AgentManager>,
    session_id: String,
) -> Result<AgentSession, String> {
    manager.snapshot(&session_id)
}

/// Cancel a session from any non-terminal phase: abort its active stream(s),
/// move it to `Cancelled`, and announce the phase. Cancelling a session with no
/// active stream is safe. Already-applied edits are untouched (Req 1.6).
#[tauri::command]
fn agent_cancel(
    app: AppHandle,
    manager: State<'_, AgentManager>,
    session_id: String,
) -> Result<(), String> {
    manager.abort_session_streams(&session_id);
    let snapshot = {
        let mut guard = manager
            .sessions
            .lock()
            .map_err(|_| "agent manager lock poisoned".to_string())?;
        guard.get_mut(&session_id).map(|session| {
            let _ = session.transition(AgentPhase::Cancelled);
            session.clone()
        })
    };
    if let Some(session) = snapshot {
        persist_agent_session(&session)?;
    }
    emit_agent_phase(&app, &session_id, AgentPhase::Cancelled);
    Ok(())
}

// --- Agent tool-calling ReAct loop (M10 Wave 7) ----------------------------
// The engine owns the loop transcript, tool model, tool-call parsing, and pure
// read tools (`gwenland_engine::agentic::{AgentLoop, tools}`). This is the Tauri
// half: it streams one provider turn per `agent_tool_step`, auto-runs the
// non-gated tools (read/git/diagnostics/open_browser), and parks mutating /
// terminal / ask tools as "awaiting" so `agent_tool_resolve` can apply them
// behind the Apply / Validation gates. Keys never appear in tool args/results.

/// `agent://tool_call` — the model requested a tool (args serialized as JSON text
/// so this crate need not depend on serde_json).
#[derive(Clone, Serialize)]
struct AgentToolCallEvent {
    session_id: String,
    id: String,
    tool: String,
    args: String,
}

/// `agent://tool_result` — the observation produced by running a tool.
#[derive(Clone, Serialize)]
struct AgentToolResultEvent {
    session_id: String,
    id: String,
    ok: bool,
    content: String,
    error: Option<String>,
}

/// `agent://ask` — the agent is asking the user to choose option(s).
#[derive(Clone, Serialize)]
struct AgentAskEvent {
    session_id: String,
    id: String,
    prompt: String,
    options: Vec<String>,
    multi: bool,
}

/// What one `agent_tool_step` resolved to, for the UI loop to react on.
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum AgentStepResult {
    /// The model produced a final answer; the loop is done.
    Final { text: String },
    /// The iteration cap was hit; stop and summarize.
    Exhausted,
    /// A non-gated tool ran; the UI should call `agent_tool_step` again.
    Ran { tool: String, ok: bool },
    /// A gated tool needs user resolution via `agent_tool_resolve`.
    Awaiting {
        id: String,
        tool: String,
        side: String,
        risk: Option<String>,
    },
}

fn agent_tool_result_event(
    session_id: &str,
    result: &gwenland_engine::agentic::ToolResult,
) -> AgentToolResultEvent {
    AgentToolResultEvent {
        session_id: session_id.to_string(),
        id: result.id.clone(),
        ok: result.ok,
        content: result.content.clone(),
        error: result.error.clone(),
    }
}

/// Redact secret-looking values and bound long output before it is fed back to
/// the model or shown in the UI.
fn redact_and_bound(text: String) -> String {
    let (redacted, _) = gwenland_engine::agentic::redact_secrets(&text);
    const MAX: usize = 16 * 1024;
    if redacted.len() <= MAX {
        return redacted;
    }
    let mut end = MAX;
    while end > 0 && !redacted.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n…(truncated)", &redacted[..end])
}

fn is_browsable_url(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    u.starts_with("http://") || u.starts_with("https://")
}

/// Open a URL in the OS default browser (zero deps, OS auto-detect).
fn open_url(url: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let mut cmd = std::process::Command::new("cmd");
        cmd.arg("/c").arg("start").arg("").arg(url);
        hide_child_window(&mut cmd);
        cmd.spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("open");
        cmd.arg(url);
        cmd.spawn()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(url);
        cmd.spawn()?;
    }
    Ok(())
}

/// Standalone "open in browser" command (also used by the agent's open_browser
/// tool). Only http/https is allowed.
#[tauri::command]
async fn open_browser(url: String) -> Result<(), String> {
    run_blocking("open_browser", move || {
        if !is_browsable_url(&url) {
            return Err("only http/https URLs can be opened".to_string());
        }
        open_url(&url).map_err(|e| e.to_string())
    })
    .await
}

/// Run a shell command in `root`, capturing exit code + combined output.
fn run_shell(root: &std::path::Path, command: &str) -> std::io::Result<(i32, String)> {
    #[cfg(windows)]
    let output = {
        let mut cmd = std::process::Command::new("cmd");
        cmd.arg("/c").arg(command).current_dir(root);
        hide_child_window(&mut cmd);
        cmd.output()?
    };
    #[cfg(not(windows))]
    let output = {
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg(command).current_dir(root);
        cmd.output()?
    };
    let code = output.status.code().unwrap_or(-1);
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        text.push('\n');
        text.push_str(&stderr);
    }
    Ok((code, text))
}

/// Execute a non-gated tool (read tools via the engine, plus git diff,
/// diagnostics stub, and open_browser). Read tools never touch secret/outside
/// paths (engine-enforced).
fn execute_auto_tool(
    root: &std::path::Path,
    call: &gwenland_engine::agentic::ToolCall,
) -> gwenland_engine::agentic::ToolResult {
    use gwenland_engine::agentic::{ToolKind, ToolResult};
    if let Some(result) = gwenland_engine::agentic::execute_local_tool(root, call) {
        return result;
    }
    match call.tool {
        ToolKind::GetGitDiff => match run_shell(root, "git diff") {
            Ok((_code, out)) => {
                let body = if out.trim().is_empty() {
                    "(no changes)".to_string()
                } else {
                    redact_and_bound(out)
                };
                ToolResult::ok(&call.id, body)
            }
            Err(e) => ToolResult::err(&call.id, format!("git diff failed: {e}")),
        },
        ToolKind::GetDiagnostics => ToolResult::ok(
            &call.id,
            "No diagnostics snapshot is available to the agent in this build.",
        ),
        ToolKind::OpenBrowser => {
            let url = call.args.get("url").and_then(|v| v.as_str()).unwrap_or("");
            if !is_browsable_url(url) {
                ToolResult::err(&call.id, "refused: only http/https URLs are allowed")
            } else {
                match open_url(url) {
                    Ok(()) => ToolResult::ok(&call.id, format!("Opened {url}")),
                    Err(e) => ToolResult::err(&call.id, format!("could not open browser: {e}")),
                }
            }
        }
        other => ToolResult::err(
            &call.id,
            format!("tool {} is not auto-executable", other.name()),
        ),
    }
}

/// Rough size of a proposed mutation in lines, feeding the Accept-for-Me
/// confidence heuristic (M10 Wave 8). Uses the `diff` (edit) or `content`
/// (write) arg; `delete_file` is irrelevant since it is always destructive.
fn estimate_mutation_lines(call: &gwenland_engine::agentic::ToolCall) -> usize {
    use gwenland_engine::agentic::ToolKind;
    let key = if matches!(call.tool, ToolKind::EditFile) {
        "diff"
    } else {
        "content"
    };
    call.args
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.lines().count())
        .unwrap_or(0)
}

/// Apply an approved mutating tool (Apply Gate). All paths are workspace-scoped
/// and secret-checked; agent deletes move into `.gwenland/trash/` for recovery.
fn apply_mutation_tool(
    root: &std::path::Path,
    call: &gwenland_engine::agentic::ToolCall,
) -> gwenland_engine::agentic::ToolResult {
    use gwenland_engine::agentic::{ToolKind, ToolResult};
    let path = call.args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.trim().is_empty() {
        return ToolResult::err(&call.id, "missing required arg 'path'");
    }
    if gwenland_engine::agentic::is_secret_path(path) {
        return ToolResult::err(&call.id, "refused: path matches a secret pattern");
    }
    let abs = root.join(path);
    if gwenland_engine::agentic::canonical_within_workspace(&abs, root).is_err() {
        return ToolResult::err(&call.id, "refused: path is outside the workspace");
    }
    match call.tool {
        ToolKind::WriteFile => {
            let content = call
                .args
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if let Some(parent) = abs.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match gwenland_engine::fs::write_file(&abs, content) {
                Ok(()) => ToolResult::ok(&call.id, format!("Wrote {path}")),
                Err(e) => ToolResult::err(&call.id, format!("write failed: {e}")),
            }
        }
        ToolKind::DeleteFile => match gwenland_engine::recovery::move_to_trash(&abs, root, "agent")
        {
            Ok(_) => ToolResult::ok(&call.id, format!("Moved {path} to workspace trash")),
            Err(e) => ToolResult::err(&call.id, format!("delete failed: {e}")),
        },
        ToolKind::EditFile => {
            let diff = call.args.get("diff").and_then(|v| v.as_str()).unwrap_or("");
            let original = match gwenland_engine::fs::read_file(&abs) {
                Ok(text) => text,
                Err(_) => {
                    return ToolResult::err(
                        &call.id,
                        format!(
                            "cannot edit '{path}': file not found. Use file_search to locate it, or write_file to create it."
                        ),
                    );
                }
            };
            let mut change_set = gwenland_engine::agentic::change_set_from_text("agent", diff);
            if change_set.files.is_empty() {
                return ToolResult::err(&call.id, "no applyable diff hunks were parsed");
            }
            // Approve every parsed hunk so the engine applier writes them all.
            let hunk_ids: Vec<String> = change_set
                .files
                .iter()
                .flat_map(|f| f.hunks.iter().map(|h| h.id.clone()))
                .collect();
            for hid in &hunk_ids {
                change_set
                    .set_hunk_approval(hid, gwenland_engine::agentic::ApprovalState::Approved);
            }
            let file = &change_set.files[0];
            match gwenland_engine::agentic::apply_approved_hunks_to_text(&original, file) {
                Ok(updated) => match gwenland_engine::fs::write_file(&abs, &updated) {
                    Ok(()) => ToolResult::ok(&call.id, format!("Edited {path}")),
                    Err(e) => ToolResult::err(&call.id, format!("write failed: {e}")),
                },
                Err(e) => ToolResult::err(&call.id, format!("could not apply diff: {e}")),
            }
        }
        other => ToolResult::err(&call.id, format!("{} is not a mutation", other.name())),
    }
}

/// `agent://cmd_output` — one line of stdout/stderr from a running agent terminal
/// command. Lets the UI render a live output stream without blocking.
#[derive(Clone, Serialize)]
struct AgentCmdOutputEvent {
    session_id: String,
    line: String,
}

/// `agent://cmd_done` — process exited; carries success flag and final output.
#[derive(Clone, Serialize)]
struct AgentCmdDoneEvent {
    session_id: String,
    success: bool,
    output: String,
}

/// Run an approved terminal command (Validation Gate). Blocked commands are
/// refused even after approval. Output is streamed line-by-line as
/// `agent://cmd_output` events so the UI can display live progress, then
/// `agent://cmd_done` is emitted on exit. The full output is also returned in
/// the `ToolResult` for the model's next turn (bounded + redacted as before).
///
/// The child PID is registered in `cmd_pids` while running so
/// `agent_kill_terminal` can kill it mid-flight.
async fn run_terminal_tool(
    app: &AppHandle,
    cmd_pids: Arc<Mutex<HashMap<String, u32>>>,
    session_id: &str,
    root: &std::path::Path,
    call: &gwenland_engine::agentic::ToolCall,
) -> gwenland_engine::agentic::ToolResult {
    use gwenland_engine::agentic::{CommandRisk, ToolResult};
    let command = call
        .args
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if command.trim().is_empty() {
        return ToolResult::err(&call.id, "missing required arg 'command'");
    }
    if matches!(
        gwenland_engine::agentic::classify_command(command),
        CommandRisk::Blocked
    ) {
        return ToolResult::err(&call.id, "refused: command could not be classified as safe");
    }

    // Spawn the child with piped stdout+stderr so we can stream line-by-line.
    #[cfg(windows)]
    let child_result = {
        let mut cmd = std::process::Command::new("cmd");
        cmd.arg("/c")
            .arg(command)
            .current_dir(root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        hide_child_window(&mut cmd);
        cmd.spawn()
    };
    #[cfg(not(windows))]
    let child_result = {
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c")
            .arg(command)
            .current_dir(root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        cmd.spawn()
    };

    let mut child = match child_result {
        Ok(c) => c,
        Err(e) => return ToolResult::err(&call.id, format!("command failed to start: {e}")),
    };

    // Register PID so `agent_kill_terminal` can kill it.
    let pid = child.id();
    if let Ok(mut pids) = cmd_pids.lock() {
        pids.insert(session_id.to_string(), pid);
    }

    // Read stdout and stderr concurrently via spawn_blocking (no tokio line readers
    // without extra deps). We collect both into a single interleaved output string
    // while emitting lines as events. `spawn_blocking` runs in the blocking thread
    // pool so it never stalls the async runtime.
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let app_out = app.clone();
    let app_err = app.clone();
    let sid_out = session_id.to_string();
    let sid_err = session_id.to_string();

    use std::io::BufRead;
    let stdout_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let stderr_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let stdout_capture = stdout_lines.clone();
    let stderr_capture = stderr_lines.clone();

    let stdout_task = tauri::async_runtime::spawn_blocking(move || {
        if let Some(out) = stdout {
            for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                let _ = app_out.emit(
                    "agent://cmd_output",
                    AgentCmdOutputEvent {
                        session_id: sid_out.clone(),
                        line: line.clone(),
                    },
                );
                if let Ok(mut v) = stdout_capture.lock() {
                    v.push(line);
                }
            }
        }
    });
    let stderr_task = tauri::async_runtime::spawn_blocking(move || {
        if let Some(err) = stderr {
            for line in std::io::BufReader::new(err).lines().map_while(Result::ok) {
                let _ = app_err.emit(
                    "agent://cmd_output",
                    AgentCmdOutputEvent {
                        session_id: sid_err.clone(),
                        line: line.clone(),
                    },
                );
                if let Ok(mut v) = stderr_capture.lock() {
                    v.push(line);
                }
            }
        }
    });

    // Wait for both readers then reap the child.
    let _ = stdout_task.await;
    let _ = stderr_task.await;
    let exit_status = tauri::async_runtime::spawn_blocking(move || child.wait())
        .await
        .ok()
        .and_then(|r| r.ok());
    let code = exit_status.as_ref().and_then(|s| s.code()).unwrap_or(-1);
    let success = exit_status.map(|s| s.success()).unwrap_or(false);

    // Deregister PID.
    if let Ok(mut pids) = cmd_pids.lock() {
        pids.remove(session_id);
    }

    // Assemble combined output (stdout then stderr, bounded + redacted).
    let mut combined = String::new();
    if let Ok(out) = stdout_lines.lock() {
        for l in out.iter() {
            combined.push_str(l);
            combined.push('\n');
        }
    }
    if let Ok(err) = stderr_lines.lock() {
        for l in err.iter() {
            combined.push_str(l);
            combined.push('\n');
        }
    }
    let output = redact_and_bound(combined.clone());

    let _ = app.emit(
        "agent://cmd_done",
        AgentCmdDoneEvent {
            session_id: session_id.to_string(),
            success,
            output: output.clone(),
        },
    );

    ToolResult::ok(&call.id, format!("exit {code}\n{output}"))
}

/// Kill the child process currently running an agent terminal command for
/// `session_id` (best-effort; tolerates an already-exited child). Uses only
/// OS shell tools — no new Rust crates required.
#[tauri::command]
async fn agent_kill_terminal(
    manager: State<'_, AgentManager>,
    session_id: String,
) -> Result<(), String> {
    let cmd_pids = manager.cmd_pids.clone();
    run_blocking("agent_kill_terminal", move || {
        let pid = cmd_pids
            .lock()
            .map_err(|_| "agent manager lock poisoned".to_string())?
            .get(&session_id)
            .copied();
        if let Some(pid) = pid {
            #[cfg(windows)]
            {
                // Force-terminate the process tree so sub-shells (npm, npx) also die.
                let args = vec![
                    "/F".to_string(),
                    "/T".to_string(),
                    "/PID".to_string(),
                    pid.to_string(),
                ];
                let mut cmd = std::process::Command::new("taskkill");
                cmd.args(&args);
                hide_child_window(&mut cmd);
                let _ = cmd.output();
            }
            #[cfg(not(windows))]
            {
                // SIGTERM the process group. `kill` is available on every Unix target
                // in the Rust standard library via std::process — we call it through
                // the shell so we don't need libc.
                let args = vec!["-TERM".to_string(), pid.to_string()];
                let mut cmd = std::process::Command::new("kill");
                cmd.args(&args);
                let _ = cmd.output();
            }
        }
        Ok(())
    })
    .await
}

/// Stream one provider turn for the tool loop: emit each token as `agent://chunk`
/// and return the full assistant text, or emit `agent://error` and return None.
async fn stream_agent_text(
    app: &AppHandle,
    adapter: Box<dyn AiProvider>,
    request: MessageRequest,
    session_id: &str,
    stream_id: &str,
) -> Option<String> {
    let mut stream = match adapter.send_message(request).await {
        Ok(s) => s,
        Err(e) => {
            emit_agent_error(app, session_id, stream_id, e);
            return None;
        }
    };
    let mut text = String::new();
    loop {
        match stream.next_chunk().await {
            Ok(Some(chunk)) => {
                text.push_str(&chunk.text);
                let _ = app.emit(
                    "agent://chunk",
                    AgentChunkEvent {
                        stream_id: stream_id.to_string(),
                        text: chunk.text,
                    },
                );
            }
            Ok(None) => return Some(text),
            Err(e) => {
                emit_agent_error(app, session_id, stream_id, e);
                return None;
            }
        }
    }
}

/// Run ONE step of the tool loop: stream a provider turn, parse the tool call,
/// and either finish, auto-run a non-gated tool, or park a gated one. The UI
/// drives iteration by calling this until it returns Final/Exhausted/Awaiting.
#[tauri::command]
async fn agent_tool_step(
    app: AppHandle,
    manager: State<'_, AgentManager>,
    session_id: String,
    stream_id: String,
    context_item_ids: Vec<String>,
) -> Result<AgentStepResult, String> {
    let session = manager.snapshot(&session_id)?;
    let root = std::path::PathBuf::from(&session.project_root);
    let base_context_summary = render_context_for_prompt(&session.context, &context_item_ids);

    // M13: on the first step only, inject a memory block into the context summary
    // using the session goal as the keyword source. Non-fatal if retrieval fails.
    let is_first_step = manager
        .loops
        .lock()
        .map_err(|_| "agent manager lock poisoned".to_string())?
        .get(&session_id)
        .map(|lp| lp.iteration == 0)
        .unwrap_or(true);

    let context_summary = if is_first_step {
        if let Some(mem_block) = retrieve_memory_block(
            &session.goal,
            &session.project_root,
            &session.provider,
            &session.model,
        )
        .await
        {
            if base_context_summary.trim().is_empty() {
                mem_block
            } else {
                format!("{mem_block}\n\n{base_context_summary}")
            }
        } else {
            base_context_summary
        }
    } else {
        base_context_summary
    };

    // Build the next request from the loop transcript (lazily create the loop).
    // Drop the guard before any await — std Mutex guards are not held across .await.
    let messages = {
        let mut loops = manager
            .loops
            .lock()
            .map_err(|_| "agent manager lock poisoned".to_string())?;
        let lp = loops
            .entry(session_id.clone())
            .or_insert_with(|| gwenland_engine::agentic::AgentLoop::new(session.goal.clone()));
        if lp.is_exhausted() {
            return Ok(AgentStepResult::Exhausted);
        }
        lp.build_messages(&context_summary)
    };

    let settings = gwenland_engine::settings::load_settings().map_err(|e| e.to_string())?;
    let adapter = gwenland_engine::ai::registry::resolve_provider(&session.provider, &settings.ai)
        .map_err(|e| e.to_string())?;
    let request = MessageRequest {
        stream_id: stream_id.clone(),
        messages,
        system: Some(gwenland_engine::agentic::AGENT_TOOL_SYSTEM.to_string()),
        attachments: Vec::new(),
        images: Vec::new(),
        model: session.model.clone(),
        max_tokens: None,
    };

    let text = match stream_agent_text(&app, adapter, request, &session_id, &stream_id).await {
        Some(t) => t,
        None => return Err("provider stream failed".to_string()),
    };

    // Record the assistant turn and parse out the tool call (None = final).
    let call = {
        let mut loops = manager
            .loops
            .lock()
            .map_err(|_| "agent manager lock poisoned".to_string())?;
        let lp = loops
            .get_mut(&session_id)
            .ok_or_else(|| "agent loop missing".to_string())?;
        lp.record_assistant(&text)
    };
    let mut call = match call {
        None => return Ok(AgentStepResult::Final { text }),
        Some(c) => c,
    };

    // Preflight a mutating tool's target path BEFORE any gate: auto-correct a
    // unique same-named file (so the Apply Gate shows the real path) or bounce a
    // missing/ambiguous path straight back to the model as a failed observation so
    // it calls `file_search` instead of attempting a doomed write (OS error 3).
    {
        use gwenland_engine::agentic::{MutationPreflight, ToolResult, preflight_mutation_path};
        if let MutationPreflight::Reject(msg) = preflight_mutation_path(&root, &mut call) {
            let result = ToolResult::err(&call.id, msg);
            let ok = result.ok;
            let tool = call.tool.name().to_string();
            let _ = app.emit(
                "agent://tool_call",
                AgentToolCallEvent {
                    session_id: session_id.clone(),
                    id: call.id.clone(),
                    tool: tool.clone(),
                    args: call.args.to_string(),
                },
            );
            let _ = app.emit(
                "agent://tool_result",
                agent_tool_result_event(&session_id, &result),
            );
            manager
                .loops
                .lock()
                .map_err(|_| "agent manager lock poisoned".to_string())?
                .get_mut(&session_id)
                .ok_or_else(|| "agent loop missing".to_string())?
                .record_tool_result(result);
            return Ok(AgentStepResult::Ran { tool, ok });
        }
    }

    let _ = app.emit(
        "agent://tool_call",
        AgentToolCallEvent {
            session_id: session_id.clone(),
            id: call.id.clone(),
            tool: call.tool.name().to_string(),
            args: call.args.to_string(),
        },
    );

    use gwenland_engine::agentic::ToolKind;
    match call.tool {
        // Mutating / terminal tools: classify, then either auto-resolve (when the
        // session's tier permits) or park behind the Apply / Validation gate.
        ToolKind::EditFile
        | ToolKind::WriteFile
        | ToolKind::DeleteFile
        | ToolKind::RunTerminalCmd => {
            use gwenland_engine::agentic::{
                ActionConfidence, CommandRisk, ToolSide, command_confidence, mutation_confidence,
                requires_user_approval,
            };
            // Classify the action for both the gate label and the tier policy.
            let (gate_side, gate_risk, confidence, risk_label) = match call.tool {
                ToolKind::RunTerminalCmd => {
                    let cmd = call
                        .args
                        .get("command")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let risk = gwenland_engine::agentic::classify_command(cmd);
                    (
                        ToolSide::Terminal,
                        Some(risk),
                        command_confidence(risk),
                        Some(format!("{risk:?}")),
                    )
                }
                // `delete_file` is a destructive mutation → hits the hard floor.
                ToolKind::DeleteFile => (
                    ToolSide::Mutating,
                    Some(CommandRisk::Destructive),
                    ActionConfidence::Low,
                    None,
                ),
                _ => {
                    let changed = estimate_mutation_lines(&call);
                    let within = call
                        .args
                        .get("path")
                        .and_then(|v| v.as_str())
                        .map(|p| {
                            gwenland_engine::agentic::is_within_workspace(&root.join(p), &root)
                        })
                        .unwrap_or(false);
                    (
                        ToolSide::Mutating,
                        None,
                        mutation_confidence(changed, within),
                        None,
                    )
                }
            };
            let side = if matches!(gate_side, ToolSide::Terminal) {
                "terminal"
            } else {
                "mutating"
            };

            // Tier policy: auto-mint + auto-consume the gate for permitted actions
            // (Full Control / high-confidence Accept-for-Me), running them inline.
            if !requires_user_approval(gate_side, gate_risk, confidence, session.tier) {
                let result = if matches!(call.tool, ToolKind::RunTerminalCmd) {
                    run_terminal_tool(&app, manager.cmd_pids.clone(), &session_id, &root, &call)
                        .await
                } else {
                    apply_mutation_tool(&root, &call)
                };
                let ok = result.ok;
                let tool = call.tool.name().to_string();
                let _ = app.emit(
                    "agent://tool_result",
                    agent_tool_result_event(&session_id, &result),
                );
                manager
                    .loops
                    .lock()
                    .map_err(|_| "agent manager lock poisoned".to_string())?
                    .get_mut(&session_id)
                    .ok_or_else(|| "agent loop missing".to_string())?
                    .record_tool_result(result);
                return Ok(AgentStepResult::Ran { tool, ok });
            }

            let id = call.id.clone();
            let tool = call.tool.name().to_string();
            manager
                .pending
                .lock()
                .map_err(|_| "agent manager lock poisoned".to_string())?
                .insert(session_id.clone(), call);
            Ok(AgentStepResult::Awaiting {
                id,
                tool,
                side: side.to_string(),
                risk: risk_label,
            })
        }
        ToolKind::AskUser => {
            let prompt = call
                .args
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let options = call
                .args
                .get("options")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let multi = call
                .args
                .get("multi")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let id = call.id.clone();
            let _ = app.emit(
                "agent://ask",
                AgentAskEvent {
                    session_id: session_id.clone(),
                    id: id.clone(),
                    prompt,
                    options,
                    multi,
                },
            );
            manager
                .pending
                .lock()
                .map_err(|_| "agent manager lock poisoned".to_string())?
                .insert(session_id.clone(), call);
            Ok(AgentStepResult::Awaiting {
                id,
                tool: "ask_user".to_string(),
                side: "ask".to_string(),
                risk: None,
            })
        }
        // Non-gated: run now, feed the observation back, ask the UI to continue.
        _ => {
            let result = execute_auto_tool(&root, &call);
            let ok = result.ok;
            let tool = call.tool.name().to_string();
            let _ = app.emit(
                "agent://tool_result",
                agent_tool_result_event(&session_id, &result),
            );
            manager
                .loops
                .lock()
                .map_err(|_| "agent manager lock poisoned".to_string())?
                .get_mut(&session_id)
                .ok_or_else(|| "agent loop missing".to_string())?
                .record_tool_result(result);
            Ok(AgentStepResult::Ran { tool, ok })
        }
    }
}

/// Resolve a gated tool (Apply / Validation gate, or an ask_user choice). The UI
/// sends `decision` ("approve" | "confirm" for destructive | "reject") and, for
/// ask_user, the chosen `selection`. The observation is recorded so the next
/// `agent_tool_step` continues the loop.
#[tauri::command]
async fn agent_tool_resolve(
    app: AppHandle,
    manager: State<'_, AgentManager>,
    session_id: String,
    decision: String,
    selection: Vec<String>,
) -> Result<(), String> {
    let session = manager.snapshot(&session_id)?;
    let root = std::path::PathBuf::from(&session.project_root);
    let call = manager
        .pending
        .lock()
        .map_err(|_| "agent manager lock poisoned".to_string())?
        .remove(&session_id)
        .ok_or_else(|| "no pending tool to resolve".to_string())?;

    let go = decision == "approve" || decision == "confirm";
    use gwenland_engine::agentic::{ToolKind, ToolResult};
    let result = match call.tool {
        ToolKind::AskUser => {
            if selection.is_empty() {
                ToolResult::ok(&call.id, "User dismissed the prompt without choosing.")
            } else {
                ToolResult::ok(&call.id, format!("User selected: {}", selection.join(", ")))
            }
        }
        ToolKind::RunTerminalCmd => {
            if go {
                run_terminal_tool(&app, manager.cmd_pids.clone(), &session_id, &root, &call).await
            } else {
                ToolResult::ok(&call.id, "User rejected the command; it was not run.")
            }
        }
        ToolKind::EditFile | ToolKind::WriteFile | ToolKind::DeleteFile => {
            if go {
                apply_mutation_tool(&root, &call)
            } else {
                ToolResult::ok(&call.id, "User rejected the change; nothing was written.")
            }
        }
        other => ToolResult::err(&call.id, format!("{} is not a gated tool", other.name())),
    };

    let _ = app.emit(
        "agent://tool_result",
        agent_tool_result_event(&session_id, &result),
    );
    manager
        .loops
        .lock()
        .map_err(|_| "agent manager lock poisoned".to_string())?
        .get_mut(&session_id)
        .ok_or_else(|| "agent loop missing".to_string())?
        .record_tool_result(result);
    Ok(())
}

// --- LSP Bridge (Milestone 6) ----------------------------------------------
// Thin wrappers over `gwenland_engine::lsp::LspManager`. Document open/change/
// close and completion commands are added in Waves 3 and 5; Wave 2 registers
// status + restart. Each loads current LSP settings and delegates to the engine.

/// Report the current LSP status for `path` (no server is spawned for this).
#[tauri::command]
async fn lsp_status(
    manager: State<'_, Arc<LspManager>>,
    path: String,
) -> Result<LspStatus, String> {
    let manager = Arc::clone(manager.inner());
    run_blocking("lsp_status", move || {
        let settings = gwenland_engine::settings::load_settings().map_err(|e| e.to_string())?;
        Ok(manager.status_for_path(Path::new(&path), &settings.lsp))
    })
    .await
}

/// Open an eligible document: ensure the server and send `didOpen` with the full
/// text. `workspace_root` (the open project folder) refines root detection.
/// Returns the resulting status (Connected / MissingServer / Disabled / …).
#[tauri::command]
async fn lsp_open_document(
    manager: State<'_, Arc<LspManager>>,
    path: String,
    text: String,
    version: i32,
    workspace_root: Option<String>,
) -> Result<LspStatus, String> {
    let manager = Arc::clone(manager.inner());
    run_blocking("lsp_open_document", move || {
        let settings = gwenland_engine::settings::load_settings().map_err(|e| e.to_string())?;
        let ws = workspace_root.as_deref().map(Path::new);
        Ok(manager.open_document(Path::new(&path), &text, version, ws, &settings.lsp))
    })
    .await
}

/// Push a full-text change to the document's server. No-op (Ok) when the
/// document is not LSP-backed; typing must never fail.
#[tauri::command]
async fn lsp_change_document(
    manager: State<'_, Arc<LspManager>>,
    path: String,
    text: String,
    version: i32,
) -> Result<(), String> {
    let manager = Arc::clone(manager.inner());
    run_blocking("lsp_change_document", move || {
        manager
            .change_document(Path::new(&path), &text, version)
            .map_err(|e| e.to_string())
    })
    .await
}

/// Close an LSP-backed document (sends `didClose`, clears its diagnostics).
#[tauri::command]
async fn lsp_close_document(
    manager: State<'_, Arc<LspManager>>,
    path: String,
) -> Result<(), String> {
    let manager = Arc::clone(manager.inner());
    run_blocking("lsp_close_document", move || {
        manager
            .close_document(Path::new(&path))
            .map_err(|e| e.to_string())
    })
    .await
}

/// Request completions at a position. Fail-soft: returns an empty list (never an
/// error) for missing servers, unsupported languages, or timeouts so the
/// autocomplete source never disrupts typing. `version` is accepted for API
/// completeness; the UI flushes a `didChange` before requesting so the server
/// already has the current text.
#[tauri::command]
async fn lsp_completion(
    manager: State<'_, Arc<LspManager>>,
    path: String,
    line: u32,
    character: u32,
    version: i32,
) -> Result<Vec<gwenland_engine::lsp::LspCompletionOption>, String> {
    let _ = version;
    let manager = Arc::clone(manager.inner());
    run_blocking("lsp_completion", move || {
        Ok(manager.completion(Path::new(&path), line, character))
    })
    .await
}

/// Request the first definition location at a position. Fail-soft: returns
/// `None` for missing servers, unsupported languages, or timeouts.
#[tauri::command]
async fn lsp_definition(
    manager: State<'_, Arc<LspManager>>,
    path: String,
    line: u32,
    character: u32,
    version: i32,
) -> Result<Option<gwenland_engine::lsp::LspDefinitionLocation>, String> {
    let _ = version;
    let manager = Arc::clone(manager.inner());
    run_blocking("lsp_definition", move || {
        Ok(manager.definition(Path::new(&path), line, character))
    })
    .await
}

/// Request hover contents at a position. Fail-soft: returns `None` for missing
/// servers, unsupported languages, or timeouts.
#[tauri::command]
async fn lsp_hover(
    manager: State<'_, Arc<LspManager>>,
    path: String,
    line: u32,
    character: u32,
    version: i32,
) -> Result<Option<gwenland_engine::lsp::LspHover>, String> {
    let _ = version;
    let manager = Arc::clone(manager.inner());
    run_blocking("lsp_hover", move || {
        Ok(manager.hover(Path::new(&path), line, character))
    })
    .await
}

/// Manually restart the server bucket for `language` (`"rust"`, `"typescript"`,
/// or `"python"`). Tears down the old client(s); the UI re-opens documents to
/// reconnect. Returns the fresh status.
#[tauri::command]
async fn lsp_restart(
    manager: State<'_, Arc<LspManager>>,
    language: String,
) -> Result<LspStatus, String> {
    let manager = Arc::clone(manager.inner());
    run_blocking("lsp_restart", move || {
        let settings = gwenland_engine::settings::load_settings().map_err(|e| e.to_string())?;
        Ok(manager.restart(&language, &settings.lsp))
    })
    .await
}

// ---------------------------------------------------------------------------
// Agent runtime tests (Milestone 10, Wave 2 task 2.7)
//
// These cover the manager's stream bookkeeping and the key-safety guarantee of
// agent session/error state — the parts that don't need a live Tauri app. Pure
// workflow logic is tested engine-side under `gwenland_engine::agentic`.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod agent_tests {
    use super::*;

    fn dummy_handle() -> tauri::async_runtime::JoinHandle<()> {
        // tauri::async_runtime lazily spins up a global runtime, so this works in
        // a plain unit test without a full Tauri app.
        tauri::async_runtime::spawn(async {})
    }

    #[test]
    fn duplicate_stream_ids_are_rejected() {
        let manager = AgentManager::default();
        assert!(
            manager
                .register_stream("stream-1".into(), "sess-1".into(), dummy_handle())
                .is_ok()
        );
        // A second registration of the same id is refused.
        assert!(
            manager
                .register_stream("stream-1".into(), "sess-1".into(), dummy_handle())
                .is_err()
        );
    }

    #[test]
    fn removing_or_aborting_a_missing_stream_is_safe() {
        let manager = AgentManager::default();
        assert!(manager.remove_stream("nope").is_none());
        // Aborting streams for a session with none active reports zero, no panic.
        assert_eq!(manager.abort_session_streams("ghost-session"), 0);
    }

    #[test]
    fn abort_session_streams_only_targets_that_session() {
        let manager = AgentManager::default();
        manager
            .register_stream("a".into(), "sess-A".into(), dummy_handle())
            .unwrap();
        manager
            .register_stream("b".into(), "sess-B".into(), dummy_handle())
            .unwrap();
        assert_eq!(manager.abort_session_streams("sess-A"), 1);
        // Session B's stream is untouched.
        assert!(manager.remove_stream("b").is_some());
        assert!(manager.remove_stream("a").is_none());
    }

    #[test]
    fn session_state_never_serializes_provider_keys_or_headers() {
        // Requirement 8.3 / 3.9 / 2.7: agent session state holds no keys/headers.
        let mut session = AgentSession::new(
            gwenland_engine::agentic::new_id(),
            "/project",
            "do the thing",
            "anthropic",
            "claude-x",
            ContextPreview::new(),
        );
        session.record_approval(ApprovalKind::Plan, "plan-1");
        let json = serde_json::to_string(&session).expect("session serializes");
        let lower = json.to_lowercase();
        for needle in [
            "api_key",
            "apikey",
            "authorization",
            "bearer",
            "secret",
            "x-api-key",
        ] {
            assert!(
                !lower.contains(needle),
                "session JSON leaked `{needle}`: {json}"
            );
        }
    }

    #[test]
    fn compose_system_prompt_layers_persona_over_base() {
        // No prefix → base prompt unchanged.
        assert_eq!(compose_system_prompt(None), GWENLAND_SYSTEM_PROMPT);
        // Blank/whitespace prefix is ignored (falls back to base).
        assert_eq!(
            compose_system_prompt(Some("   \n ")),
            GWENLAND_SYSTEM_PROMPT
        );
        // A real persona is layered ON TOP, base protocol still present.
        let composed = compose_system_prompt(Some("You are Gwen."));
        assert!(composed.starts_with("You are Gwen."));
        assert!(composed.contains(GWENLAND_SYSTEM_PROMPT));
        // The <think> protocol from the base prompt survives.
        assert!(composed.contains("<think>"));
    }

    #[test]
    fn agent_error_event_carries_only_sanitized_aierror() {
        // The agent error event reuses `AiError`, which by construction holds no
        // key material. Verify a representative error stays key-free when emitted.
        let event = AgentErrorEvent {
            session_id: "s".into(),
            stream_id: "st".into(),
            error: AiError::InvalidKey,
        };
        let json = serde_json::to_string(&event).expect("event serializes");
        let lower = json.to_lowercase();
        assert!(!lower.contains("authorization"));
        assert!(!lower.contains("bearer"));
    }
}

// ---------------------------------------------------------------------------
// Git integration (Wave 2 — GWEN-327..331)
//
// Thin wrappers over the tauri-free `gwenland_engine::git` module, which shells
// out to the system `git` binary. Every command takes the workspace `root`.
// ---------------------------------------------------------------------------

#[tauri::command]
async fn git_is_repo(root: String) -> bool {
    tauri::async_runtime::spawn_blocking(move || {
        gwenland_engine::git::is_git_repo(Path::new(&root))
    })
    .await
    .unwrap_or(false)
}

#[tauri::command]
async fn git_status(root: String) -> Result<gwenland_engine::git::GitStatus, String> {
    run_blocking("git_status", move || {
        gwenland_engine::git::status(Path::new(&root)).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn get_git_graph(
    workspace_path: String,
    max_commits: Option<u32>,
) -> Result<gwenland_engine::git::CommitGraphPayload, String> {
    run_blocking("get_git_graph", move || {
        gwenland_engine::git::graph(Path::new(&workspace_path), max_commits)
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn get_commit_details(
    workspace_path: String,
    hash: String,
) -> Result<gwenland_engine::git::CommitDetails, String> {
    run_blocking("get_commit_details", move || {
        gwenland_engine::git::commit_details(Path::new(&workspace_path), &hash)
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn get_commit_diff(workspace_path: String, hash: String) -> Result<String, String> {
    run_blocking("get_commit_diff", move || {
        gwenland_engine::git::commit_diff(Path::new(&workspace_path), &hash)
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn git_stage(root: String, path: String, all: bool) -> Result<(), String> {
    run_blocking("git_stage", move || {
        let r = Path::new(&root);
        if all {
            gwenland_engine::git::stage_all(r)
        } else {
            gwenland_engine::git::stage(r, &path)
        }
        .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn git_unstage(root: String, path: String, all: bool) -> Result<(), String> {
    run_blocking("git_unstage", move || {
        let r = Path::new(&root);
        if all {
            gwenland_engine::git::unstage_all(r)
        } else {
            gwenland_engine::git::unstage(r, &path)
        }
        .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn git_discard(root: String, path: String, untracked: bool) -> Result<(), String> {
    run_blocking("git_discard", move || {
        gwenland_engine::git::discard(Path::new(&root), &path, untracked).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn git_commit(root: String, message: String) -> Result<(), String> {
    run_blocking("git_commit", move || {
        gwenland_engine::git::commit(Path::new(&root), &message).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn git_push(root: String) -> Result<String, String> {
    run_blocking("git_push", move || {
        gwenland_engine::git::push(Path::new(&root)).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn git_pull(root: String) -> Result<String, String> {
    run_blocking("git_pull", move || {
        gwenland_engine::git::pull(Path::new(&root)).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn git_diff_file(root: String, path: String, untracked: bool) -> Result<String, String> {
    run_blocking("git_diff_file", move || {
        gwenland_engine::git::diff_file(Path::new(&root), &path, untracked)
            .map_err(|e| e.to_string())
    })
    .await
}

const AI_DIFF_REVIEW_SYSTEM: &str = "\
You are GwenLand IDE's inline code reviewer. Review only the provided git diff.
Return concise Markdown with high-signal findings first. Prioritize correctness,
bugs, regressions, security, data loss, performance cliffs, and missing tests.
Call out exact changed areas when possible. If you find no high-confidence issue,
say that clearly. Do not propose a full rewrite and do not claim to apply code.";

#[tauri::command]
async fn ai_review_diff(root: String, path: String, untracked: bool) -> Result<String, String> {
    let diff_path = path.clone();
    let diff = run_blocking("ai_review_diff.git_diff", move || {
        gwenland_engine::git::diff_file(Path::new(&root), &diff_path, untracked)
            .map_err(|e| e.to_string())
    })
    .await?;

    if diff.trim().is_empty() {
        return Ok("No changes to review.".to_string());
    }

    let settings = gwenland_engine::settings::load_settings().map_err(|e| e.to_string())?;
    let provider_id = settings.ai.active_provider.trim().to_string();
    let model_id = settings.ai.active_model.trim().to_string();
    if provider_id.is_empty() {
        return Err("No active AI provider configured.".to_string());
    }
    if model_id.is_empty() {
        return Err("No active AI model selected.".to_string());
    }

    let prompt = format!(
        "Review this git diff for `{path}`. untracked={untracked}.\n\n```diff\n{diff}\n```"
    );

    complete_once(
        Some(AI_DIFF_REVIEW_SYSTEM.to_string()),
        prompt,
        &provider_id,
        &model_id,
        Some(1200),
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn git_list_branches(root: String) -> Result<Vec<String>, String> {
    run_blocking("git_list_branches", move || {
        gwenland_engine::git::list_branches(Path::new(&root)).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn git_checkout(root: String, branch: String) -> Result<(), String> {
    run_blocking("git_checkout", move || {
        gwenland_engine::git::checkout(Path::new(&root), &branch).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn git_create_branch(root: String, name: String) -> Result<String, String> {
    run_blocking("git_create_branch", move || {
        gwenland_engine::git::create_branch(Path::new(&root), &name).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn git_delete_branch(root: String, branch: String) -> Result<(), String> {
    run_blocking("git_delete_branch", move || {
        gwenland_engine::git::delete_branch(Path::new(&root), &branch).map_err(|e| e.to_string())
    })
    .await
}

// ---------------------------------------------------------------------------
// Workspace settings (M14 Wave 1)
//
// Thin wrappers over `gwenland_engine::workspace`. The engine owns load/save
// logic; these commands only convert the engine result to a Tauri-friendly
// `Result<_, String>`. No policy logic lives here.
// ---------------------------------------------------------------------------

/// Load the per-workspace settings overlay from `.gwenland/settings.json`.
/// Returns the default struct (all None fields) when the file is absent or
/// malformed — never errors on missing config.
#[tauri::command]
async fn workspace_load_settings(
    workspace_root: String,
) -> Result<gwenland_engine::workspace::WorkspaceSettings, String> {
    run_blocking("workspace_load_settings", move || {
        Ok(gwenland_engine::workspace::load_workspace_settings(
            Path::new(&workspace_root),
        ))
    })
    .await
}

/// Save the per-workspace settings overlay to `.gwenland/settings.json`.
/// Creates the `.gwenland/` directory if needed; write is atomic.
#[tauri::command]
async fn workspace_save_settings(
    workspace_root: String,
    settings: gwenland_engine::workspace::WorkspaceSettings,
) -> Result<(), String> {
    run_blocking("workspace_save_settings", move || {
        gwenland_engine::workspace::save_workspace_settings(Path::new(&workspace_root), &settings)
            .map_err(|e| e.to_string())
    })
    .await
}

/// Load UI-owned workspace restore state from `.gwenland/workspace.json`.
/// Missing, empty, or malformed files return `None`.
#[tauri::command]
async fn load_workspace_state(workspace_root: String) -> Option<serde_json::Value> {
    tauri::async_runtime::spawn_blocking(move || {
        gwenland_engine::workspace::load_workspace_state(Path::new(&workspace_root))
    })
    .await
    .unwrap_or(None)
}

/// Save UI-owned workspace restore state to `.gwenland/workspace.json`.
#[tauri::command]
async fn save_workspace_state(
    workspace_root: String,
    state: serde_json::Value,
) -> Result<(), String> {
    run_blocking("save_workspace_state", move || {
        gwenland_engine::workspace::save_workspace_state(Path::new(&workspace_root), &state)
            .map_err(|e| e.to_string())
    })
    .await
}

/// Load UI-owned layout restore state from `.gwenland/layout.json`.
/// Missing, empty, or malformed files return `None`.
#[tauri::command]
async fn load_layout_state(workspace_root: String) -> Option<serde_json::Value> {
    tauri::async_runtime::spawn_blocking(move || {
        gwenland_engine::workspace::load_layout_state(Path::new(&workspace_root))
    })
    .await
    .unwrap_or(None)
}

/// Save UI-owned layout restore state to `.gwenland/layout.json`.
#[tauri::command]
async fn save_layout_state(workspace_root: String, state: serde_json::Value) -> Result<(), String> {
    run_blocking("save_layout_state", move || {
        gwenland_engine::workspace::save_layout_state(Path::new(&workspace_root), &state)
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn history_save_entry(
    workspace_root: String,
    file_path: String,
    content: String,
    source: String,
) -> Result<Option<gwenland_engine::history::HistoryEntry>, String> {
    run_blocking("history_save_entry", move || {
        gwenland_engine::history::save_history_entry(
            Path::new(&workspace_root),
            Path::new(&file_path),
            &content,
            &source,
        )
        .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn history_list(
    workspace_root: String,
    file_path: String,
) -> Result<Vec<gwenland_engine::history::HistoryEntry>, String> {
    run_blocking("history_list", move || {
        gwenland_engine::history::list_history(Path::new(&workspace_root), Path::new(&file_path))
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn history_read_entry(
    workspace_root: String,
    file_path: String,
    timestamp: String,
) -> Result<String, String> {
    run_blocking("history_read_entry", move || {
        gwenland_engine::history::read_history_entry(
            Path::new(&workspace_root),
            Path::new(&file_path),
            &timestamp,
        )
        .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn history_clear(workspace_root: String, file_path: String) -> Result<(), String> {
    run_blocking("history_clear", move || {
        gwenland_engine::history::clear_history(Path::new(&workspace_root), Path::new(&file_path))
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn move_to_trash(
    path: String,
    workspace_root: String,
) -> Result<gwenland_engine::recovery::TrashRecord, String> {
    run_blocking("move_to_trash", move || {
        gwenland_engine::recovery::move_to_trash(
            Path::new(&path),
            Path::new(&workspace_root),
            "user",
        )
        .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
fn search_workspace(
    app: AppHandle,
    manager: State<SearchManager>,
    root: String,
    query: String,
    search_id: String,
) -> Result<String, String> {
    let cancel = Arc::new(AtomicBool::new(false));

    {
        let mut active = manager
            .active
            .lock()
            .map_err(|_| "search manager lock poisoned".to_string())?;
        if let Some(previous) = active.as_ref() {
            previous.cancel.store(true, Ordering::Relaxed);
        }
        *active = Some(ActiveSearch {
            id: search_id.clone(),
            cancel: cancel.clone(),
        });
    }

    let active = manager.active.clone();
    let app_handle = app.clone();
    let id_for_task = search_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = gwenland_engine::search::search_workspace(
            std::path::Path::new(&root),
            &query,
            cancel.clone(),
            |result| {
                let _ = app_handle.emit(
                    "search:result",
                    SearchResultEvent {
                        search_id: id_for_task.clone(),
                        result,
                    },
                );
            },
        );

        let payload = match result {
            Ok(summary) => SearchDoneEvent {
                search_id: id_for_task.clone(),
                summary: Some(summary),
                error: None,
            },
            Err(e) => SearchDoneEvent {
                search_id: id_for_task.clone(),
                summary: None,
                error: Some(e.to_string()),
            },
        };
        let _ = app_handle.emit("search:done", payload);

        if let Ok(mut slot) = active.lock()
            && slot
                .as_ref()
                .is_some_and(|current| current.id == id_for_task)
        {
            *slot = None;
        }
    });

    Ok(search_id)
}

#[tauri::command]
fn search_cancel(manager: State<SearchManager>, search_id: Option<String>) -> Result<(), String> {
    let active = manager
        .active
        .lock()
        .map_err(|_| "search manager lock poisoned".to_string())?;
    if let Some(current) = active.as_ref()
        && search_id.as_ref().is_none_or(|id| id == &current.id)
    {
        current.cancel.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
async fn mark_protected_path(path: String, workspace_root: String) -> Result<(), String> {
    run_blocking("mark_protected_path", move || {
        use gwenland_engine::safety::decision::RiskLevel;
        use gwenland_engine::safety::{ProtectedPathEntry, ProtectedPathRegistry, ProtectionLevel};

        let root = Path::new(&workspace_root);
        let target = Path::new(&path);
        if !gwenland_engine::agentic::policy::is_within_workspace(target, root) {
            return Err("path would escape workspace root".to_string());
        }
        let rel = target
            .strip_prefix(root)
            .map_err(|_| "path would escape workspace root".to_string())?
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let registry_path =
            gwenland_engine::workspace::safety_dir(root).join("protected-paths.json");
        let mut registry = std::fs::read_to_string(&registry_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<ProtectedPathRegistry>(&raw).ok())
            .unwrap_or_default();
        if !registry.entries.iter().any(|entry| entry.pattern == rel) {
            registry.entries.push(ProtectedPathEntry {
                pattern: rel,
                protection: ProtectionLevel::Ask,
                risk: RiskLevel::High,
                reason: "marked protected by user".to_string(),
            });
        }
        if let Some(parent) = registry_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let raw = serde_json::to_string_pretty(&registry).map_err(|e| e.to_string())?;
        std::fs::write(registry_path, raw).map_err(|e| e.to_string())
    })
    .await
}

// ---------------------------------------------------------------------------
// Safety Engine (M14 Wave 5)
//
// Thin command wrapper so the UI can call the Safety Engine for explicit
// confirmation prompts. Policy logic lives entirely in the engine.
// ---------------------------------------------------------------------------

/// Evaluate a safety action and return the decision. The UI uses this to
/// show confirmation dialogs or block actions before calling the underlying
/// engine operation.
///
/// `action_kind_json` is the JSON-serialized `SafetyActionKind` value.
/// `strictness` is one of "standard" | "strict" | "paranoid".
#[tauri::command]
async fn safety_evaluate(
    action_kind_json: String,
    workspace_root: String,
    actor: String,
    strictness: String,
) -> Result<gwenland_engine::safety::SafetyDecision, String> {
    run_blocking("safety_evaluate", move || {
        use gwenland_engine::safety::protected_paths::ProtectedPathRegistry;
        use gwenland_engine::safety::{Actor, SafetyAction, SafetyActionKind};
        use gwenland_engine::workspace::SafetyStrictness;

        let kind: SafetyActionKind = serde_json::from_str(&action_kind_json)
            .map_err(|e| format!("invalid action_kind_json: {e}"))?;

        let actor_val = match actor.as_str() {
            "user" => Actor::User,
            "agent" => Actor::Agent,
            "system" => Actor::System,
            ext if ext.starts_with("extension:") => Actor::Extension {
                id: ext["extension:".len()..].to_string(),
            },
            _ => Actor::User,
        };

        let strictness_val = match strictness.as_str() {
            "strict" => SafetyStrictness::Strict,
            "paranoid" => SafetyStrictness::Paranoid,
            _ => SafetyStrictness::Standard,
        };

        let ws = Path::new(&workspace_root);
        let registry = ProtectedPathRegistry::load(ws);
        let action = SafetyAction::new(actor_val, kind, &workspace_root);
        Ok(gwenland_engine::safety::evaluate(
            &action,
            &registry,
            strictness_val,
        ))
    })
    .await
}

/// Check whether a path should be excluded from local search results.
#[tauri::command]
async fn search_should_exclude(path: String, workspace_root: String) -> bool {
    tauri::async_runtime::spawn_blocking(move || {
        gwenland_engine::search_policy::should_exclude_from_search(
            &path,
            Path::new(&workspace_root),
        )
    })
    .await
    .unwrap_or(true)
}

// ---------------------------------------------------------------------------
// Extension Permission Foundation (M14 Wave 6)
//
// Thin wrappers over `gwenland_engine::permissions`. Registry and approval-
// history logic is entirely engine-side; these commands only convert errors
// to strings. No extension runtime is implemented in M14.
// ---------------------------------------------------------------------------

/// Load the effective permission state for an extension. Returns the resolved
/// verdict (from the per-workspace registry + default matrix) for every known
/// permission kind. Never errors — falls back to default matrix when the
/// registry file is absent or malformed.
#[tauri::command]
async fn permissions_load_state(
    workspace_root: String,
    extension_id: String,
) -> Vec<gwenland_engine::permissions::PermissionDecision> {
    tauri::async_runtime::spawn_blocking(move || {
        use gwenland_engine::permissions::{Permission, PermissionRegistry};
        let registry = PermissionRegistry::load(Path::new(&workspace_root));
        let known: &[Permission] = &[
            Permission::ReadWorkspace,
            Permission::WriteFile,
            Permission::DeleteFile,
            Permission::RunTerminal,
            Permission::AccessGit,
            Permission::AccessEnv,
            Permission::AccessDatabase,
        ];
        known
            .iter()
            .map(|perm| registry.resolve(&extension_id, perm))
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// Record an extension permission approval or denial in the workspace approval
/// history JSONL. The `target_summary` is bounded + redacted before writing.
#[tauri::command]
async fn permissions_record_approval(
    workspace_root: String,
    extension_id: String,
    permission: String,
    approved: bool,
    target_summary: String,
) -> Result<(), String> {
    run_blocking("permissions_record_approval", move || {
        let record = gwenland_engine::permissions::ApprovalRecord::new(
            extension_id,
            permission,
            approved,
            target_summary,
        );
        gwenland_engine::permissions::record_approval(Path::new(&workspace_root), &record)
            .map_err(|e| e.to_string())
    })
    .await
}

// ---------------------------------------------------------------------------
// Batched file watcher (M19 Wave 1, GWEN-376)
//
// The engine's `FsWatcher` is a tauri-free polling watcher: it diffs registered
// directories on an interval and hands coalesced `Vec<FsPatch>` to a callback.
// This is the Tauri side — the callback emits one `fs:patch` event per cycle, so
// a burst (e.g. `npm install`) becomes a handful of batched events, never one
// event per file. The frontend registers each expanded folder via `fs_watch_dir`
// and drops it on collapse via `fs_unwatch_dir`.
// ---------------------------------------------------------------------------

/// Managed state holding the single live watcher. Built in `setup()` because the
/// emit callback needs the `AppHandle` (same pattern as `LspManager`).
struct FsWatcherState {
    watcher: Arc<Mutex<gwenland_engine::fs_watch::FsWatcher>>,
}

/// Start watching a directory for changes. Re-registering an already-watched dir
/// is a cheap idempotent re-seed. Paths inside `.git` are silently refused by the
/// engine.
#[tauri::command]
async fn fs_watch_dir(state: State<'_, FsWatcherState>, path: String) -> Result<(), String> {
    let watcher = state.watcher.clone();
    run_blocking("fs_watch_dir", move || {
        watcher
            .lock()
            .map_err(|_| "fs watcher lock poisoned".to_string())?
            .watch(&path);
        Ok(())
    })
    .await
}

/// Stop watching a directory (e.g. the user collapsed the folder). Idempotent.
#[tauri::command]
async fn fs_unwatch_dir(state: State<'_, FsWatcherState>, path: String) -> Result<(), String> {
    let watcher = state.watcher.clone();
    run_blocking("fs_unwatch_dir", move || {
        watcher
            .lock()
            .map_err(|_| "fs watcher lock poisoned".to_string())?
            .unwatch(&path);
        Ok(())
    })
    .await
}

/// Stop watching every directory (e.g. workspace closed/switched).
#[tauri::command]
async fn fs_watch_clear(state: State<'_, FsWatcherState>) -> Result<(), String> {
    let watcher = state.watcher.clone();
    run_blocking("fs_watch_clear", move || {
        watcher
            .lock()
            .map_err(|_| "fs watcher lock poisoned".to_string())?
            .clear();
        Ok(())
    })
    .await
}

// ---------------------------------------------------------------------------
// Rust-owned flat file tree (M19 Wave 2, GWEN-374 + GWEN-375)
//
// The engine's `WorkspaceTree` owns the tree shape as an ordered flat row array
// and returns `TreePatch` deltas on every mutation. This is the Tauri side: a
// single mutex-guarded tree in managed state plus the commands that drive it.
// `tree_set_root` returns the initial rows; expand/collapse/refresh return
// patches the frontend splices into its virtual-scroll mirror. The frontend
// calls `tree_refresh_dir` from its `fs:patch` handler (Wave 1) so disk changes
// reconcile through the same patch path.
// ---------------------------------------------------------------------------

/// Managed state holding the one window's tree. A `Mutex` is plenty: tree
/// mutations are user-paced (expand/collapse) or coarse (poll-driven refresh).
#[derive(Default)]
struct WorkspaceTreeState {
    tree: Arc<Mutex<gwenland_engine::tree::WorkspaceTree>>,
}

/// Open a workspace root and return its immediate child rows (the initial,
/// non-diff render). Replaces any prior tree.
#[tauri::command]
async fn tree_set_root(
    state: State<'_, WorkspaceTreeState>,
    path: String,
) -> Result<Vec<gwenland_engine::tree::FlatRow>, String> {
    let tree = state.tree.clone();
    run_blocking("tree_set_root", move || {
        let mut tree = tree
            .lock()
            .map_err(|_| "tree state lock poisoned".to_string())?;
        Ok(tree.set_root(&path))
    })
    .await
}

/// Expand a folder; returns the patches that splice its children in.
#[tauri::command]
async fn tree_expand(
    state: State<'_, WorkspaceTreeState>,
    path: String,
) -> Result<Vec<gwenland_engine::tree::TreePatch>, String> {
    let tree = state.tree.clone();
    run_blocking("tree_expand", move || {
        let mut tree = tree
            .lock()
            .map_err(|_| "tree state lock poisoned".to_string())?;
        tree.expand(&path).map_err(|e| e.to_string())
    })
    .await
}

/// Collapse a folder; returns the patches that remove its subtree.
#[tauri::command]
async fn tree_collapse(
    state: State<'_, WorkspaceTreeState>,
    path: String,
) -> Result<Vec<gwenland_engine::tree::TreePatch>, String> {
    let tree = state.tree.clone();
    run_blocking("tree_collapse", move || {
        let mut tree = tree
            .lock()
            .map_err(|_| "tree state lock poisoned".to_string())?;
        Ok(tree.collapse(&path))
    })
    .await
}

/// Reconcile a single directory against disk (driven by `fs:patch`). Returns the
/// minimal add/remove patches; empty when nothing changed or the dir is not a
/// visible/expanded part of the tree.
#[tauri::command]
async fn tree_refresh_dir(
    state: State<'_, WorkspaceTreeState>,
    path: String,
) -> Result<Vec<gwenland_engine::tree::TreePatch>, String> {
    let tree = state.tree.clone();
    run_blocking("tree_refresh_dir", move || {
        let mut tree = tree
            .lock()
            .map_err(|_| "tree state lock poisoned".to_string())?;
        Ok(tree.refresh_dir(&path))
    })
    .await
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(TerminalManager::default())
        .manage(AiManager::default())
        .manage(AgentManager::default())
        .manage(WorkspaceTreeState::default())
        // The LspManager needs the AppHandle to emit lsp:// events, so it is
        // built in setup() (not on the builder). Its two callbacks are the only
        // bridge between the tauri-free engine and the event bus.
        .setup(|app| {
            let diag_handle = app.handle().clone();
            let status_handle = app.handle().clone();
            let missing_handle = app.handle().clone();
            let message_handle = app.handle().clone();
            let manager = Arc::new(LspManager::new(
                Arc::new(move |upd: DiagnosticsUpdate| {
                    let _ = diag_handle.emit("lsp://diagnostics", upd);
                }),
                Arc::new(move |upd: StatusUpdate| {
                    if matches!(&upd.status, LspStatus::MissingServer { .. }) {
                        let _ = missing_handle.emit("lsp://server-not-found", upd.clone());
                    }
                    let _ = status_handle.emit("lsp://status", upd);
                }),
                Arc::new(move |upd: MessageUpdate| {
                    let _ = message_handle.emit("lsp://message", upd);
                }),
            ));
            app.manage(manager);

            // M19 Wave 1: start the polling file watcher. Each coalesced cycle
            // becomes one `fs:patch` event carrying `Vec<FsPatch>`.
            let watch_handle = app.handle().clone();
            let watcher = gwenland_engine::fs_watch::FsWatcher::start(
                move |patches: Vec<gwenland_engine::fs_watch::FsPatch>| {
                    let _ = watch_handle.emit("fs:patch", patches);
                },
            );
            app.manage(FsWatcherState {
                watcher: Arc::new(Mutex::new(watcher)),
            });
            app.manage(SearchManager::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_data_path,
            load_settings,
            save_settings,
            get_recent_projects,
            add_recent_project,
            open_folder_dialog,
            list_directory,
            read_file,
            get_file_meta,
            write_file,
            path_exists,
            fs_watch_dir,
            fs_unwatch_dir,
            fs_watch_clear,
            tree_set_root,
            tree_expand,
            tree_collapse,
            tree_refresh_dir,
            create_file,
            create_dir,
            rename_path,
            delete_path,
            duplicate_path,
            reveal_in_explorer,
            move_path_to_os_trash,
            move_to_trash,
            search_workspace,
            search_cancel,
            mark_protected_path,
            terminal_create,
            terminal_detect_shells,
            terminal_write,
            terminal_resize,
            terminal_kill,
            open_new_window,
            ai_set_key,
            ai_delete_key,
            ai_check_key,
            ai_list_models,
            ai_model_catalog,
            ai_send,
            ai_cancel,
            ai_search_result,
            ai_complete,
            agent_create_session,
            agent_set_tier,
            agent_context_preview,
            agent_request_plan,
            agent_request_edits,
            agent_approve_plan,
            agent_set_hunk_approval,
            agent_set_file_approval,
            agent_apply_changes,
            agent_approve_validation_command,
            agent_run_validation,
            agent_summarize,
            agent_restore_sessions,
            agent_get_session,
            agent_cancel,
            agent_tool_step,
            agent_tool_resolve,
            agent_kill_terminal,
            open_browser,
            parse_diff,
            conversation_new,
            conversation_list,
            conversation_load,
            conversation_truncate,
            conversation_rename,
            conversation_delete,
            conversation_set_training_opt_in,
            lsp_status,
            lsp_restart,
            lsp_open_document,
            lsp_change_document,
            lsp_close_document,
            lsp_completion,
            lsp_definition,
            lsp_hover,
            git_is_repo,
            git_status,
            get_git_graph,
            get_commit_details,
            get_commit_diff,
            git_stage,
            git_unstage,
            git_discard,
            git_commit,
            git_push,
            git_pull,
            git_diff_file,
            ai_review_diff,
            git_list_branches,
            git_checkout,
            git_create_branch,
            git_delete_branch,
            workspace_load_settings,
            workspace_save_settings,
            load_workspace_state,
            save_workspace_state,
            load_layout_state,
            save_layout_state,
            history_save_entry,
            history_list,
            history_read_entry,
            history_clear,
            safety_evaluate,
            search_should_exclude,
            permissions_load_state,
            permissions_record_approval
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application");

    // On exit, gracefully shut down language servers (Requirement 6.8/6.10).
    // ServerProcess::Drop also force-kills, so children are never orphaned.
    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event
            && let Some(manager) = app_handle.try_state::<LspManager>()
        {
            manager.shutdown_all();
        }
    });
}
