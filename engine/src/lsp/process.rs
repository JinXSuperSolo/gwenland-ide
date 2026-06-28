//! Language-server stdio process lifecycle (Milestone 6, Wave 2).
//!
//! [`resolve_command`] does BYO server detection (absolute path / `PATH` search
//! with Windows `PATHEXT`). [`ServerProcess`] spawns a server with piped stdio,
//! runs a dedicated reader thread that frames + routes JSON-RPC traffic
//! (mirroring the PTY reader-thread pattern in [`crate::terminal`]), and offers
//! request/notify plus graceful `shutdown` and force `kill`.
//!
//! No async runtime: request/response correlation uses `std::sync::mpsc` and
//! completion's fail-soft deadline uses `recv_timeout`.

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::error::{LspError, sanitize};
use super::json_rpc::{
    FrameDecoder, Incoming, PendingRequests, RequestIdGen, ResponseError, encode_notification,
    encode_request, encode_response, parse_message,
};

/// Responder channel stored per in-flight request: the reader thread sends the
/// response (or a crash error) back to the waiting caller.
type Responder = SyncSender<Result<Value, LspError>>;
type ExitCallback = Box<dyn Fn(String) + Send>;

const MAX_STDERR_TAIL_CHARS: usize = 2_000;

/// Check whether `base` (or, on Windows, `base` + a `PATHEXT` extension) is an
/// executable file, returning the resolved path.
fn find_executable(base: &Path) -> Option<PathBuf> {
    if base.is_file() {
        return Some(base.to_path_buf());
    }
    #[cfg(windows)]
    {
        // Only append extensions when the base has none (so an explicit
        // ".cmd"/".exe" is honored as-is above).
        if base.extension().is_none() {
            let pathext =
                std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
            for ext in pathext.split(';') {
                let ext = ext.trim().trim_start_matches('.');
                if ext.is_empty() {
                    continue;
                }
                let mut s = base.as_os_str().to_os_string();
                s.push(".");
                s.push(ext);
                let cand = PathBuf::from(s);
                if cand.is_file() {
                    return Some(cand);
                }
            }
        }
    }
    None
}

/// Resolve a configured server command to a concrete executable path.
/// Absolute paths (and paths containing a separator) are checked directly;
/// bare command names are searched on `PATH`. Returns `None` when nothing is
/// found — the caller maps that to [`LspError::MissingServer`] (Requirement
/// 3.8/3.9).
pub fn resolve_command(command: &str) -> Option<PathBuf> {
    resolve_command_with_path(command, std::env::var_os("PATH").as_deref())
}

/// Testable core of [`resolve_command`] with an explicit `PATH` value.
pub fn resolve_command_with_path(command: &str, path: Option<&OsStr>) -> Option<PathBuf> {
    if command.trim().is_empty() {
        return None;
    }
    let p = Path::new(command);
    if p.is_absolute() || command.contains('/') || command.contains('\\') {
        return find_executable(p);
    }
    let path = path?;
    for dir in std::env::split_paths(path) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        if let Some(found) = find_executable(&dir.join(command)) {
            return Some(found);
        }
    }
    None
}

/// A live language-server child process plus its JSON-RPC transport.
pub struct ServerProcess {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    id_gen: Arc<RequestIdGen>,
    pending: Arc<Mutex<PendingRequests<Responder>>>,
    /// Set true before an intentional teardown so the reader thread does not
    /// report a normal shutdown as a crash.
    shutting_down: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
    stderr_drain: Option<JoinHandle<()>>,
}

impl ServerProcess {
    /// Spawn `program args...` in `root` with piped stdio and start the reader
    /// thread. `on_notification` receives server notifications
    /// (`publishDiagnostics`, `logMessage`, …); `on_exit` fires once if the
    /// process exits *unexpectedly* (not on graceful shutdown/kill).
    pub fn spawn(
        program: &Path,
        args: &[String],
        root: &Path,
        on_notification: Box<dyn Fn(String, Value) + Send>,
        on_exit: ExitCallback,
    ) -> Result<ServerProcess, LspError> {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Only set the working directory when it exists; a stale root must not
        // turn into a spawn failure.
        if root.is_dir() {
            cmd.current_dir(root);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: don't flash a console for the child server.
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| LspError::Spawn(sanitize(&e.to_string())))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| LspError::Spawn("server stdin unavailable".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| LspError::Spawn("server stdout unavailable".into()))?;
        let stderr = child.stderr.take();

        let stdin = Arc::new(Mutex::new(stdin));
        let id_gen = Arc::new(RequestIdGen::new());
        let pending: Arc<Mutex<PendingRequests<Responder>>> =
            Arc::new(Mutex::new(PendingRequests::new()));
        let shutting_down = Arc::new(AtomicBool::new(false));
        let stderr_tail = Arc::new(Mutex::new(String::new()));

        // Reader thread: decode frames and route them.
        let reader = {
            let pending = pending.clone();
            let stdin_for_replies = stdin.clone();
            let shutting_down = shutting_down.clone();
            let stderr_tail = stderr_tail.clone();
            thread::spawn(move || {
                reader_loop(
                    stdout,
                    pending,
                    stdin_for_replies,
                    on_notification,
                    on_exit,
                    shutting_down,
                    stderr_tail,
                );
            })
        };

        // Drain stderr so a chatty server can't fill the pipe and stall. In
        // debug builds the (sanitized) lines are echoed for troubleshooting.
        let stderr_drain = stderr.map(|mut err| {
            let stderr_tail = stderr_tail.clone();
            thread::spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match err.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            append_stderr_tail(&stderr_tail, &buf[..n]);
                            #[cfg(debug_assertions)]
                            {
                                let text = String::from_utf8_lossy(&buf[..n]);
                                eprint!("[lsp stderr] {}", sanitize(&text));
                            }
                            #[cfg(not(debug_assertions))]
                            {
                                let _ = n; // discard
                            }
                        }
                    }
                }
            })
        });

        Ok(ServerProcess {
            child,
            stdin,
            id_gen,
            pending,
            shutting_down,
            reader: Some(reader),
            stderr_drain,
        })
    }

    /// Send a request and block for its response up to `timeout`. On timeout the
    /// pending entry is cleaned up (Requirement 7.4); on transport loss the
    /// error is `Crashed`.
    pub fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, LspError> {
        let id = self.id_gen.next_id();
        let (tx, rx) = mpsc::sync_channel::<Result<Value, LspError>>(1);

        // Insert before writing so a fast response can never miss the entry.
        self.pending
            .lock()
            .map_err(|_| LspError::transport("pending map poisoned"))?
            .insert(id, tx);

        let frame = encode_request(id, method, params);
        if let Err(e) = self.write_frame(&frame) {
            self.remove_pending(id);
            return Err(e);
        }

        match rx.recv_timeout(timeout) {
            Ok(res) => res,
            Err(RecvTimeoutError::Timeout) => {
                self.remove_pending(id);
                Err(LspError::Timeout)
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.remove_pending(id);
                Err(LspError::Crashed("language server exited".into()))
            }
        }
    }

    /// Send a fire-and-forget notification.
    pub fn notify(&self, method: &str, params: Value) -> Result<(), LspError> {
        let frame = encode_notification(method, params);
        self.write_frame(&frame)
    }

    fn write_frame(&self, frame: &[u8]) -> Result<(), LspError> {
        let mut stdin = self
            .stdin
            .lock()
            .map_err(|_| LspError::transport("stdin poisoned"))?;
        stdin
            .write_all(frame)
            .map_err(|e| LspError::transport(e.to_string()))?;
        stdin
            .flush()
            .map_err(|e| LspError::transport(e.to_string()))
    }

    fn remove_pending(&self, id: u64) {
        if let Ok(mut p) = self.pending.lock() {
            p.take(id);
        }
    }

    /// Whether the child is still running.
    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Graceful shutdown: `shutdown` request, `exit` notification, then wait for
    /// the process to exit, force-killing after `timeout` (Requirement 6.4/6.9).
    pub fn shutdown(&mut self, timeout: Duration) {
        self.shutting_down.store(true, Ordering::SeqCst);
        let _ = self.request("shutdown", Value::Null, timeout);
        let _ = self.notify("exit", Value::Null);

        let deadline = Instant::now() + timeout;
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = self.child.kill();
                        let _ = self.child.wait();
                        return;
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                Err(_) => {
                    let _ = self.child.kill();
                    return;
                }
            }
        }
    }

    /// Force-kill immediately (no graceful handshake).
    pub fn kill(&mut self) {
        self.shutting_down.store(true, Ordering::SeqCst);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        // Guarantee no orphaned child (Requirement 6.10). The reader thread sees
        // stdout close and exits on its own; we detach rather than join so drop
        // never blocks.
        self.shutting_down.store(true, Ordering::SeqCst);
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.reader.take();
        self.stderr_drain.take();
    }
}

/// The reader thread body: pump `stdout` through a [`FrameDecoder`], routing
/// each message until EOF, then clean up pending requests and (for an
/// unexpected exit) fire `on_exit`.
fn reader_loop(
    mut stdout: std::process::ChildStdout,
    pending: Arc<Mutex<PendingRequests<Responder>>>,
    stdin_for_replies: Arc<Mutex<ChildStdin>>,
    on_notification: Box<dyn Fn(String, Value) + Send>,
    on_exit: ExitCallback,
    shutting_down: Arc<AtomicBool>,
    stderr_tail: Arc<Mutex<String>>,
) {
    let mut decoder = FrameDecoder::new();
    let mut buf = [0u8; 8192];

    'read: loop {
        let n = match stdout.read(&mut buf) {
            Ok(0) => break 'read, // EOF — process exited
            Ok(n) => n,
            Err(_) => break 'read, // read error — treat as exit
        };
        decoder.push(&buf[..n]);

        loop {
            match decoder.next_frame() {
                Ok(Some(body)) => match parse_message(&body) {
                    Ok(Incoming::Response { id, result, error }) => {
                        if let Some(tx) = pending.lock().ok().and_then(|mut p| p.take(id)) {
                            let res = match error {
                                Some(ResponseError { code, message, .. }) => Err(
                                    LspError::protocol(format!("server error {code}: {message}")),
                                ),
                                None => Ok(result.unwrap_or(Value::Null)),
                            };
                            let _ = tx.send(res);
                        }
                    }
                    Ok(Incoming::Notification { method, params }) => {
                        on_notification(method, params);
                    }
                    Ok(Incoming::ServerRequest { id, .. }) => {
                        // M6 implements no server-request handlers; reply with a
                        // null result so the server never blocks waiting on us.
                        let frame = encode_response(id, Value::Null);
                        if let Ok(mut w) = stdin_for_replies.lock() {
                            let _ = w.write_all(&frame);
                            let _ = w.flush();
                        }
                    }
                    Ok(Incoming::UnknownResponse) => {}
                    // Malformed payload: stay alive and keep reading (Req 7.7).
                    Err(_) => {}
                },
                Ok(None) => break, // need more bytes
                Err(_) => break,   // malformed header; resync on next read
            }
        }
    }

    let crash_message = stderr_crash_message(&stderr_tail);

    // Transport closed: fail every waiter so no request hangs (Requirement 7.4).
    if let Ok(mut p) = pending.lock() {
        for tx in p.drain() {
            let _ = tx.send(Err(LspError::Crashed(crash_message.clone())));
        }
    }
    // Only surface a crash if this wasn't an intentional teardown.
    if !shutting_down.load(Ordering::SeqCst) {
        on_exit(crash_message);
    }
}

fn append_stderr_tail(tail: &Arc<Mutex<String>>, bytes: &[u8]) {
    let text = sanitize(&String::from_utf8_lossy(bytes));
    if text.trim().is_empty() {
        return;
    }
    let Ok(mut tail) = tail.lock() else {
        return;
    };
    if !tail.is_empty() && !tail.ends_with(' ') {
        tail.push(' ');
    }
    tail.push_str(text.trim());
    let len = tail.chars().count();
    if len > MAX_STDERR_TAIL_CHARS {
        *tail = tail
            .chars()
            .skip(len - MAX_STDERR_TAIL_CHARS)
            .collect::<String>();
    }
}

fn stderr_crash_message(tail: &Arc<Mutex<String>>) -> String {
    // The stdout reader often observes EOF just before the stderr drain thread
    // stores the final line. Give very short-lived failing servers a tiny window
    // so the user sees the real startup error rather than a generic exit.
    for _ in 0..5 {
        if tail.lock().map(|t| !t.trim().is_empty()).unwrap_or(false) {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let detail = tail
        .lock()
        .ok()
        .map(|t| t.trim().to_string())
        .unwrap_or_default();
    if detail.is_empty() {
        "language server exited".into()
    } else {
        format!("language server exited: {detail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn empty_command_resolves_to_none() {
        assert_eq!(resolve_command_with_path("", None), None);
        assert_eq!(resolve_command_with_path("   ", None), None);
    }

    #[test]
    fn missing_command_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        let path = std::env::join_paths([tmp.path()]).unwrap();
        assert_eq!(
            resolve_command_with_path("definitely-not-a-real-server", Some(&path)),
            None
        );
    }

    #[test]
    fn resolves_command_found_on_path() {
        let tmp = tempfile::tempdir().unwrap();
        // A bare filename (no extension) is matched by the exact-path check on
        // every platform.
        let exe = tmp.path().join("myserver");
        fs::write(&exe, b"#!/bin/sh\n").unwrap();
        let path = std::env::join_paths([tmp.path()]).unwrap();

        let found = resolve_command_with_path("myserver", Some(&path));
        assert_eq!(found, Some(exe));
    }

    #[test]
    fn resolves_absolute_path_directly() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("tool.bin");
        fs::write(&exe, b"x").unwrap();
        let found = resolve_command_with_path(exe.to_str().unwrap(), None);
        assert_eq!(found, Some(exe));
    }

    #[test]
    fn absolute_path_that_does_not_exist_is_none() {
        let missing = if cfg!(windows) {
            "C:\\nope\\not-here.exe"
        } else {
            "/nope/not-here"
        };
        assert_eq!(resolve_command_with_path(missing, None), None);
    }

    /// A child that exits immediately must cause an in-flight request to fail
    /// (not hang) and fire the unexpected-exit callback, with pending requests
    /// drained (Requirement 6.4/7.4; Wave 2 checkpoint).
    #[test]
    fn quick_exit_child_triggers_crash_and_clears_pending() {
        let (program, args): (PathBuf, Vec<String>) = if cfg!(windows) {
            let cmd = resolve_command("cmd").expect("cmd.exe present on Windows");
            (cmd, vec!["/c".into(), "exit".into(), "0".into()])
        } else {
            (PathBuf::from("/bin/sh"), vec!["-c".into(), "exit 0".into()])
        };

        let exited = Arc::new(AtomicBool::new(false));
        let exited_cb = exited.clone();
        let proc = ServerProcess::spawn(
            &program,
            &args,
            Path::new("."),
            Box::new(|_, _| {}),
            Box::new(move |_| exited_cb.store(true, Ordering::SeqCst)),
        )
        .expect("spawn quick-exit child");

        // Either the write fails (child already gone) or the reader drains the
        // pending request on EOF — both are errors, never a hang.
        let res = proc.request("initialize", Value::Null, Duration::from_secs(5));
        assert!(res.is_err(), "expected an error, got {res:?}");

        // The unexpected-exit callback should fire shortly.
        let mut fired = false;
        for _ in 0..100 {
            if exited.load(Ordering::SeqCst) {
                fired = true;
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert!(fired, "on_exit was not called for an unexpected exit");
        assert!(
            proc.pending.lock().unwrap().is_empty(),
            "pending requests must be cleared on crash"
        );
    }

    /// Graceful kill must NOT report a crash via `on_exit`.
    #[test]
    fn intentional_kill_does_not_report_crash() {
        // A long-lived child (sleep) we then kill.
        let (program, args): (PathBuf, Vec<String>) = if cfg!(windows) {
            // `cmd /c pause` waits for input forever on its stdin (piped here).
            let cmd = resolve_command("cmd").expect("cmd.exe present");
            (cmd, vec!["/c".into(), "pause".into()])
        } else {
            (
                PathBuf::from("/bin/sh"),
                vec!["-c".into(), "sleep 30".into()],
            )
        };

        let crashed = Arc::new(AtomicBool::new(false));
        let crashed_cb = crashed.clone();
        let mut proc = ServerProcess::spawn(
            &program,
            &args,
            Path::new("."),
            Box::new(|_, _| {}),
            Box::new(move |_| crashed_cb.store(true, Ordering::SeqCst)),
        )
        .expect("spawn long-lived child");

        assert!(proc.is_alive());
        proc.kill();

        // Give the reader thread time to observe the EOF.
        thread::sleep(Duration::from_millis(200));
        assert!(
            !crashed.load(Ordering::SeqCst),
            "intentional kill must not be reported as a crash"
        );
    }

    #[test]
    fn quick_exit_child_surfaces_stderr() {
        let (program, args): (PathBuf, Vec<String>) = if cfg!(windows) {
            let cmd = resolve_command("cmd").expect("cmd.exe present on Windows");
            (
                cmd,
                vec![
                    "/c".into(),
                    "echo lsp startup failed 1>&2 & exit /b 1".into(),
                ],
            )
        } else {
            (
                PathBuf::from("/bin/sh"),
                vec!["-c".into(), "echo lsp startup failed >&2; exit 1".into()],
            )
        };

        let proc = ServerProcess::spawn(
            &program,
            &args,
            Path::new("."),
            Box::new(|_, _| {}),
            Box::new(move |_| {}),
        )
        .expect("spawn stderr child");

        let err = proc
            .request("initialize", Value::Null, Duration::from_secs(5))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("lsp startup failed"),
            "stderr detail should be included, got {err:?}"
        );
    }
}
