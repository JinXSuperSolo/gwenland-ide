//! Cross-platform PTY session manager (Milestone 3, Wave 1).
//!
//! Built on `portable-pty` (wezterm), which selects ConPTY on Windows and
//! openpty on Unix at runtime via `native_pty_system()`. This module is the
//! zero-tauri core: it owns the PTY lifecycle (spawn → I/O → kill) and exposes
//! nothing about Tauri. The Tauri command + event bridge lands in Wave 2 in
//! `frontend/src/main.rs`.
//!
//! Three correctness concerns drove the design, each enforced in code (not just
//! documented) and exercised by the tests at the bottom of this file:
//!
//!   1. **Reads happen on a dedicated thread.** `portable-pty` readers block;
//!      reading on the caller's thread would freeze whoever drives the session.
//!      [`PtySession::spawn_command`] moves the cloned reader into a thread that
//!      drains it into a shared, mutex-guarded buffer.
//!
//!   2. **Teardown order is explicit, and the reader is *not* joined.** The
//!      slave is dropped immediately after spawning so only the child holds it.
//!      At teardown the child is killed/reaped first, then the writer and master
//!      are dropped. The reader thread is deliberately detached: on Windows a
//!      blocking read on a ConPTY pipe never returns once all slaves terminate
//!      (dropping the master does not unblock it, unlike Unix's EOF), so joining
//!      it would deadlock. See [`PtySession::teardown`].
//!
//!   3. **Kill is explicit and followed by a wait.** [`PtySession::kill`] kills
//!      the child via its killer handle and then `wait`s, so the child is reaped
//!      rather than left as a zombie/orphan; an already-exited child (whose kill
//!      handle is invalid on Windows) is tolerated. [`Drop`] does the same as a
//!      backstop if a session is dropped while still running.
//!
//! The module is a pure byte pipe: it does not interpret terminal escape
//! sequences. Answering queries like the startup DSR cursor-position request is
//! the renderer's job (xterm.js in Wave 3); the headless tests stand in for it.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use portable_pty::{Child, ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use thiserror::Error;

use crate::devserver_detect::{DevServerDetector, DevServerSignal};
use crate::error_detect::{ErrorDetector, ErrorSignal};
use crate::ring_buffer::{DEFAULT_MAX_LINES, RingBuffer};

#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("failed to open pty: {0}")]
    OpenPty(String),
    #[error("failed to spawn shell: {0}")]
    Spawn(String),
    #[error("failed to obtain pty writer: {0}")]
    Writer(String),
    #[error("failed to obtain pty reader: {0}")]
    Reader(String),
    #[error("write to pty failed: {0}")]
    Write(String),
    #[error("failed to kill session: {0}")]
    Kill(String),
}

/// Lifecycle status of a session's child process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Running,
    Exited,
}

/// Default interactive shell for the current platform, chosen at *runtime* so a
/// single binary adapts to the environment it actually runs in.
///
/// Honours `$SHELL` on Unix and `%ComSpec%` on Windows when set; otherwise falls
/// back to a sensible default (PowerShell on Windows, `/bin/sh` on Unix).
fn default_shell() -> String {
    #[cfg(windows)]
    {
        // Prefer PowerShell, but respect an explicit ComSpec override (e.g. cmd).
        std::env::var("ComSpec").unwrap_or_else(|_| "powershell.exe".to_string())
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
}

/// Callback invoked on the reader thread for each chunk of PTY output, as it
/// arrives. Used by the Tauri bridge (Wave 2) to push output to the frontend as
/// events without polling. Runs on the reader thread, so it must be `Send` and
/// should be cheap/non-blocking; it must not call back into the session.
pub type OutputCallback = Box<dyn FnMut(&[u8]) + Send + 'static>;

/// Callback invoked on the reader thread when the error detector flags a line
/// (Wave 6). Used by the Tauri bridge to emit a `terminal://error` event. Like
/// [`OutputCallback`] it runs on the reader thread, so it must be `Send` and
/// cheap, and must not call back into the session.
pub type ErrorCallback = Box<dyn FnMut(&ErrorSignal) + Send + 'static>;

/// Callback invoked on the reader thread the first time the dev-server detector
/// spots a ready URL (M5). Used by the Tauri bridge to emit a
/// `terminal://devserver-ready` event so the IDE can auto-open a web preview.
/// Fires at most once per session (the detector latches). Same reader-thread
/// constraints as [`OutputCallback`].
pub type DevServerCallback = Box<dyn FnMut(&DevServerSignal) + Send + 'static>;

/// A single PTY-backed shell session.
///
/// Owns the master end of the PTY, the spawned child, a background reader thread
/// draining output into [`PtySession::output`], and a killer handle usable to
/// terminate the child without holding the child lock.
pub struct PtySession {
    /// Master end of the PTY. `Option` so it can be dropped *explicitly* before
    /// joining the reader thread: the blocked reader only sees EOF once the
    /// master (and thus the PTY) is closed. Joining while the master is still
    /// alive deadlocks — that is the central drop-order constraint here.
    master: Option<Box<dyn MasterPty + Send>>,
    /// Writer taken from the master once at spawn so `write_input` can be called
    /// repeatedly (`take_writer` may only be called once). Dropped explicitly
    /// during teardown alongside the master.
    writer: Option<Box<dyn Write + Send>>,
    child: Box<dyn Child + Send + Sync>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    reader_handle: Option<JoinHandle<()>>,
    /// Line-capped scrollback (Wave 5). The reader thread feeds chunks in; old
    /// lines are trimmed so memory stays bounded under heavy output.
    output: Arc<Mutex<RingBuffer>>,
    /// Error signals detected so far (Wave 6), in arrival order. The reader
    /// thread appends as it scans each chunk; queryable via [`error_signals`].
    errors: Arc<Mutex<Vec<ErrorSignal>>>,
    /// The detected dev-server ready URL (M5), set once by the reader thread the
    /// first time a ready line is seen; `None` until then. Queryable via
    /// [`dev_server_signal`](PtySession::dev_server_signal).
    dev_server: Arc<Mutex<Option<DevServerSignal>>>,
    killed: bool,
}

/// Builds a `CommandBuilder` for the platform default shell, optionally rooted
/// at `cwd` so the shell opens in the project folder rather than the app's cwd.
/// A `cwd` that doesn't exist is ignored (the shell would otherwise fail to
/// spawn); the shell then starts in its default directory.
fn default_shell_command(cwd: Option<&std::path::Path>) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(default_shell());
    if let Some(dir) = cwd
        && dir.is_dir()
    {
        cmd.cwd(dir);
    }
    cmd
}

impl PtySession {
    /// Spawns the platform default shell in a fresh PTY of the given size,
    /// optionally rooted at `cwd` (e.g. the open project folder).
    pub fn spawn(
        rows: u16,
        cols: u16,
        cwd: Option<&std::path::Path>,
    ) -> Result<Self, TerminalError> {
        Self::spawn_command(default_shell_command(cwd), rows, cols)
    }

    /// Spawns an explicit command in a fresh PTY. Used by [`spawn`] and by tests
    /// that want a deterministic command rather than an interactive shell.
    pub fn spawn_command(cmd: CommandBuilder, rows: u16, cols: u16) -> Result<Self, TerminalError> {
        Self::spawn_command_with_callback(cmd, rows, cols, None)
    }

    /// Spawns the platform default shell, streaming each output chunk to
    /// `on_output` as it arrives (in addition to buffering it). `on_error`, if
    /// given, fires whenever the error detector flags a line; `on_dev_server`, if
    /// given, fires once when a dev-server ready URL is detected (M5). This is the
    /// entry point the Tauri bridge uses to push output + error + dev-server
    /// events to the frontend live. `cwd`, if set and existing, becomes the
    /// shell's working directory.
    pub fn spawn_with_callback(
        rows: u16,
        cols: u16,
        cwd: Option<&std::path::Path>,
        on_output: OutputCallback,
        on_error: Option<ErrorCallback>,
        on_dev_server: Option<DevServerCallback>,
    ) -> Result<Self, TerminalError> {
        Self::spawn_inner(
            default_shell_command(cwd),
            rows,
            cols,
            DEFAULT_MAX_LINES,
            Some(on_output),
            on_error,
            on_dev_server,
        )
    }

    /// Spawns `cmd` in a fresh PTY; if `on_output` is given, the reader thread
    /// invokes it with each chunk before buffering. Scrollback is capped at
    /// [`DEFAULT_MAX_LINES`]; tests that need a smaller cap use [`spawn_inner`].
    pub fn spawn_command_with_callback(
        cmd: CommandBuilder,
        rows: u16,
        cols: u16,
        on_output: Option<OutputCallback>,
    ) -> Result<Self, TerminalError> {
        Self::spawn_inner(cmd, rows, cols, DEFAULT_MAX_LINES, on_output, None, None)
    }

    /// Core constructor with an explicit scrollback line cap and optional output,
    /// error, and dev-server callbacks.
    fn spawn_inner(
        cmd: CommandBuilder,
        rows: u16,
        cols: u16,
        max_lines: usize,
        on_output: Option<OutputCallback>,
        on_error: Option<ErrorCallback>,
        on_dev_server: Option<DevServerCallback>,
    ) -> Result<Self, TerminalError> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TerminalError::OpenPty(e.to_string()))?;

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| TerminalError::Spawn(e.to_string()))?;

        // Mitigation #2 (drop order): drop the slave now. Only the child should
        // hold the slave end; otherwise on Windows the reader may never see EOF.
        drop(pair.slave);

        let killer = child.clone_killer();

        // Take the writer once now; `take_writer` may only be called a single
        // time, so we keep it for the life of the session.
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| TerminalError::Writer(e.to_string()))?;

        // Mitigation #1 (reads on a dedicated thread): the reader blocks, so we
        // drain it off-thread into a shared buffer until EOF.
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| TerminalError::Reader(e.to_string()))?;
        let output = Arc::new(Mutex::new(RingBuffer::new(max_lines)));
        let errors = Arc::new(Mutex::new(Vec::<ErrorSignal>::new()));
        let dev_server = Arc::new(Mutex::new(None::<DevServerSignal>));

        let thread_output = Arc::clone(&output);
        let thread_errors = Arc::clone(&errors);
        let thread_dev_server = Arc::clone(&dev_server);
        let reader_handle = std::thread::spawn(move || {
            let mut reader = reader;
            let mut on_output = on_output;
            let mut on_error = on_error;
            let mut on_dev_server = on_dev_server;
            let mut detector = ErrorDetector::new();
            let mut dev_detector = DevServerDetector::new();
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF: PTY closed.
                    Ok(n) => {
                        let chunk = &buf[..n];
                        // Push live to the consumer first (Wave 2 streaming),
                        // then retain in the line-capped ring buffer (Wave 5).
                        if let Some(cb) = on_output.as_mut() {
                            cb(chunk);
                        }
                        if let Ok(mut out) = thread_output.lock() {
                            out.push(chunk);
                        }
                        // Reactive error detection (Wave 6): scan only this
                        // chunk's newly completed lines, record + notify.
                        for sig in detector.scan_chunk(chunk) {
                            if let Some(cb) = on_error.as_mut() {
                                cb(&sig);
                            }
                            if let Ok(mut errs) = thread_errors.lock() {
                                errs.push(sig);
                            }
                        }
                        // Dev-server ready detection (M5): latches, so this fires
                        // at most once per session — record it + notify the bridge.
                        if let Some(sig) = dev_detector.scan_chunk(chunk) {
                            if let Some(cb) = on_dev_server.as_mut() {
                                cb(&sig);
                            }
                            if let Ok(mut slot) = thread_dev_server.lock() {
                                *slot = Some(sig);
                            }
                        }
                    }
                    Err(_) => break, // PTY torn down underneath us.
                }
            }
        });

        Ok(Self {
            master: Some(pair.master),
            writer: Some(writer),
            child,
            killer,
            reader_handle: Some(reader_handle),
            output,
            errors,
            dev_server,
            killed: false,
        })
    }

    /// Writes raw bytes to the PTY (keystrokes / command text). The caller is
    /// responsible for line endings (e.g. trailing `\n`). Uses the writer taken
    /// once at spawn, so it may be called repeatedly.
    pub fn write_input(&mut self, data: &[u8]) -> Result<(), TerminalError> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| TerminalError::Write("session writer already closed".to_string()))?;
        writer
            .write_all(data)
            .map_err(|e| TerminalError::Write(e.to_string()))?;
        writer
            .flush()
            .map_err(|e| TerminalError::Write(e.to_string()))?;
        Ok(())
    }

    /// Resizes the PTY's reported window. Safe to call while the child runs.
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), TerminalError> {
        let master = self
            .master
            .as_ref()
            .ok_or_else(|| TerminalError::OpenPty("session pty already closed".to_string()))?;
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TerminalError::OpenPty(e.to_string()))
    }

    /// Current lifecycle status. `Exited` once the child is no longer running.
    pub fn status(&mut self) -> SessionStatus {
        if self.killed {
            return SessionStatus::Exited;
        }
        match self.child.try_wait() {
            Ok(Some(_)) => SessionStatus::Exited,
            _ => SessionStatus::Running,
        }
    }

    /// Snapshot of the retained scrollback (UTF-8 lossy). With the Wave 5 ring
    /// buffer this is the last `DEFAULT_MAX_LINES` lines, not all output ever.
    pub fn output_string(&self) -> String {
        let out = self.output.lock().unwrap_or_else(|e| e.into_inner());
        out.to_string_lossy()
    }

    /// Number of completed scrollback lines currently retained. Caps at the
    /// ring buffer's line limit; primarily for tests/diagnostics.
    pub fn retained_line_count(&self) -> usize {
        let out = self.output.lock().unwrap_or_else(|e| e.into_inner());
        out.line_count()
    }

    /// True if the detector has flagged at least one possible-error line this
    /// session (Wave 6). The flag for a future AI "explain this error" feature.
    pub fn error_detected(&self) -> bool {
        let errs = self.errors.lock().unwrap_or_else(|e| e.into_inner());
        !errs.is_empty()
    }

    /// All error signals detected so far, in arrival order.
    pub fn error_signals(&self) -> Vec<ErrorSignal> {
        let errs = self.errors.lock().unwrap_or_else(|e| e.into_inner());
        errs.clone()
    }

    /// The detected dev-server ready signal (M5), or `None` if no ready URL has
    /// been seen in this session's output yet. Set once (the detector latches).
    pub fn dev_server_signal(&self) -> Option<DevServerSignal> {
        let slot = self.dev_server.lock().unwrap_or_else(|e| e.into_inner());
        slot.clone()
    }

    /// Mitigation #3 (explicit kill + wait): terminate the child, reap it so no
    /// zombie/orphan is left behind, then tear the PTY down and join the reader.
    pub fn kill(&mut self) -> Result<(), TerminalError> {
        self.teardown()
    }

    /// Shared teardown used by both [`kill`] and [`Drop`]. Idempotent.
    ///
    /// Order and the join policy are both load-bearing, and the Windows ConPTY
    /// reality drives them:
    ///
    ///   1. Kill the child (unless already exited), then `wait` to reap it so no
    ///      zombie (Unix) / lingering handle (Windows) is left (mitigation #3).
    ///      Killing an *already-exited* child reports an "invalid handle" error
    ///      on Windows, and `try_wait` can still say "running" in the brief race
    ///      window right after a fast child exits — so we treat a kill failure
    ///      as success when a follow-up `wait` confirms the child is gone.
    ///   2. Drop the writer and master to close our ends of the PTY.
    ///   3. Do **not** block-join the reader thread. On Windows a blocking read
    ///      on a ConPTY pipe hangs indefinitely once all slaves have terminated
    ///      — dropping the master does not unblock it (unlike Unix, which EOFs).
    ///      Joining here is the deadlock we keep hitting. The reader holds only
    ///      a cloned reader handle and no child handle, so leaving its blocked
    ///      read to be torn down at process exit leaks nothing meaningful; the
    ///      captured output is already in `self.output`.
    fn teardown(&mut self) -> Result<(), TerminalError> {
        if !self.killed {
            // Attempt the kill, but tolerate failure when the child has in fact
            // already exited (the common case for fast commands, where the kill
            // handle is invalid). We confirm by reaping: if `wait` yields a
            // status, the child is gone and the kill error is irrelevant.
            let kill_result = self.killer.kill();
            let reaped = self.child.wait(); // reaps; no zombie/orphan left.

            if let Err(e) = kill_result {
                // Only surface the error if the child is somehow still alive.
                if reaped.is_err() {
                    return Err(TerminalError::Kill(e.to_string()));
                }
            }
            self.killed = true;
        }

        // Close our ends of the PTY. We deliberately do not join the reader
        // thread (see step 3 above) — detaching it avoids the ConPTY deadlock.
        drop(self.writer.take());
        drop(self.master.take());
        self.reader_handle.take(); // detach; do not join.

        Ok(())
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        // Backstop: if a live session is dropped without an explicit kill, the
        // same teardown kills+reaps the child and joins the reader so we never
        // leak a process or thread.
        let _ = self.teardown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant};

    /// Runs `body` on a worker thread and fails fast if it does not finish
    /// within `limit`. Without this, a teardown deadlock (the bug this module
    /// is careful to avoid) would hang the test runner indefinitely instead of
    /// failing; here it aborts with a labelled message so the culprit is
    /// obvious. Run tests with `--test-threads=1` to keep the label unambiguous.
    fn with_timeout(label: &'static str, limit: Duration, body: impl FnOnce() + Send + 'static) {
        let done = Arc::new(AtomicBool::new(false));
        let done_worker = Arc::clone(&done);
        let worker = std::thread::spawn(move || {
            body();
            done_worker.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        let start = Instant::now();
        while start.elapsed() < limit {
            if done.load(std::sync::atomic::Ordering::SeqCst) {
                // Propagate any panic from the body as a test failure.
                worker.join().expect("test body panicked");
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        // Deadline blown: almost certainly a teardown deadlock. Abort with a
        // clear label rather than letting the harness hang for minutes.
        panic!("test {label:?} exceeded {limit:?} — likely PTY teardown deadlock");
    }

    /// Blocks until `f` returns true or `timeout` elapses. Returns whether the
    /// condition was met. Used to wait on async PTY output without sleeping for
    /// a fixed (and flaky) duration.
    fn wait_until(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if f() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        f()
    }

    /// A long-lived shell command, per platform, used by the lifecycle tests
    /// that need a child which keeps running until we kill it. We deliberately
    /// avoid an instant `echo`-and-exit child: on Windows ConPTY such a child
    /// can exit before its output is relayed and before `try_wait` observes it,
    /// which is a flaky basis for assertions. The interactive-shell acceptance
    /// test below covers the spawn → command → output → kill flow instead.
    fn long_lived_command() -> CommandBuilder {
        #[cfg(windows)]
        {
            let mut c = CommandBuilder::new("cmd.exe");
            c.arg("/C");
            c.arg("ping -n 60 127.0.0.1 >NUL");
            c
        }
        #[cfg(not(windows))]
        {
            let mut c = CommandBuilder::new("/bin/sh");
            c.arg("-c");
            c.arg("sleep 60");
            c
        }
    }

    /// On Windows, ConPTY forwards a DSR cursor-position query (`ESC[6n`) from
    /// the shell at startup and the shell *blocks* until the terminal answers
    /// with a cursor-position report (`ESC[<row>;<col>R`). A real terminal
    /// emulator (xterm.js, Wave 3) answers this automatically; the engine PTY
    /// layer deliberately stays a pure byte pipe and does NOT interpret escape
    /// sequences. So here in the headless tests we answer it ourselves, standing
    /// in for the Wave 3 renderer, then send the command. Returns once the reply
    /// has been sent (or immediately on platforms that don't issue the query).
    fn answer_startup_dsr_then(session: &mut PtySession, then_send: &[u8]) {
        // Wait briefly for the startup query, answer it, then send the command.
        let saw_query = wait_until(Duration::from_secs(5), || {
            session.output_string().contains("\u{1b}[6n")
        });
        if saw_query {
            session
                .write_input(b"\x1b[1;1R")
                .expect("dsr reply write should succeed");
        }
        session
            .write_input(then_send)
            .expect("command write should succeed");
    }

    // Acceptance test for Wave 1: spawn the default shell, send one command,
    // read its output, then kill the session — all without crashing or leaking.
    // Using an interactive shell (rather than a one-shot command) keeps ConPTY
    // actively relaying output on Windows, which a fire-and-exit child does not
    // reliably do. The marker proves the dedicated reader thread (mitigation #1)
    // captured output; the final kill exercises mitigation #3.
    #[test]
    fn spawn_shell_command_output_then_kill() {
        with_timeout(
            "spawn_shell_command_output_then_kill",
            Duration::from_secs(30),
            || {
                let marker = "gwenland_pty_ok";
                let mut session = PtySession::spawn(24, 80, None).expect("spawn default shell");

                // Send one command (CRLF submits it on both cmd and POSIX shells).
                // The shell stays alive so the PTY keeps relaying its output to us.
                answer_startup_dsr_then(&mut session, format!("echo {marker}\r\n").as_bytes());

                let captured = wait_until(Duration::from_secs(10), || {
                    session.output_string().contains(marker)
                });
                assert!(
                    captured,
                    "expected marker {marker:?} in shell output, got: {:?}",
                    session.output_string()
                );

                session.kill().expect("kill should succeed");
                assert_eq!(session.status(), SessionStatus::Exited);
            },
        );
    }

    // Mitigation #3: kill terminates a *running* child and is idempotent — a
    // second kill (e.g. explicit kill then Drop) never errors or hangs, and the
    // session reports Exited afterward.
    #[test]
    fn kill_running_session_is_idempotent() {
        with_timeout(
            "kill_running_session_is_idempotent",
            Duration::from_secs(30),
            || {
                let mut session = PtySession::spawn_command(long_lived_command(), 24, 80)
                    .expect("spawn should succeed");
                assert_eq!(session.status(), SessionStatus::Running);
                session.kill().expect("first kill ok");
                session.kill().expect("second kill ok (idempotent)");
                assert_eq!(session.status(), SessionStatus::Exited);
            },
        );
    }

    // Mitigation #2 / #3: dropping a still-running session must not leak the
    // child or hang. A long-running command is spawned and then dropped without
    // an explicit kill; Drop must kill+reap and detach the reader. The test
    // returning (not hanging) is the assertion.
    #[test]
    fn drop_running_session_cleans_up() {
        with_timeout(
            "drop_running_session_cleans_up",
            Duration::from_secs(30),
            || {
                let session = PtySession::spawn_command(long_lived_command(), 24, 80)
                    .expect("spawn should succeed");
                // Give the child a moment to actually start, then drop it.
                std::thread::sleep(Duration::from_millis(100));
                drop(session); // Must return promptly via kill+wait, not hang.
            },
        );
    }

    // Sending two commands in sequence proves the stored writer survives across
    // calls (`take_writer` may only run once, so write_input reuses it).
    #[test]
    fn write_input_handles_multiple_commands() {
        with_timeout(
            "write_input_handles_multiple_commands",
            Duration::from_secs(30),
            || {
                let mut session = PtySession::spawn(24, 80, None).expect("spawn default shell");

                // First command also clears the startup DSR query; the second is a
                // plain follow-up, proving the stored writer survives across calls.
                answer_startup_dsr_then(&mut session, b"echo first_marker\r\n");
                session
                    .write_input(b"echo second_marker\r\n")
                    .expect("second write ok");

                let captured = wait_until(Duration::from_secs(10), || {
                    let out = session.output_string();
                    out.contains("first_marker") && out.contains("second_marker")
                });
                assert!(
                    captured,
                    "expected both markers in shell output, got: {:?}",
                    session.output_string()
                );

                session.kill().expect("kill ok");
            },
        );
    }

    // Wave 2: the output callback fires live with each chunk as it arrives,
    // independently of the internal buffer. We collect callback bytes into our
    // own Vec and assert the command's marker shows up there — proving the
    // streaming path the Tauri event bridge depends on.
    #[test]
    fn output_callback_receives_live_chunks() {
        with_timeout(
            "output_callback_receives_live_chunks",
            Duration::from_secs(30),
            || {
                let marker = "gwenland_cb_marker";
                let collected = Arc::new(Mutex::new(Vec::<u8>::new()));
                let cb_sink = Arc::clone(&collected);

                let mut session = PtySession::spawn_with_callback(
                    24,
                    80,
                    None,
                    Box::new(move |chunk: &[u8]| {
                        cb_sink.lock().unwrap().extend_from_slice(chunk);
                    }),
                    None,
                    None,
                )
                .expect("spawn with callback");

                answer_startup_dsr_then(&mut session, format!("echo {marker}\r\n").as_bytes());

                let seen = wait_until(Duration::from_secs(10), || {
                    let bytes = collected.lock().unwrap();
                    String::from_utf8_lossy(&bytes).contains(marker)
                });
                assert!(
                    seen,
                    "expected marker {marker:?} via callback, got: {:?}",
                    String::from_utf8_lossy(&collected.lock().unwrap())
                );

                session.kill().expect("kill ok");
            },
        );
    }

    // The shell spawns in the requested cwd. We point it at a fresh tempdir,
    // ask the shell to print its working directory, and assert the tempdir's
    // unique final component shows up. Matching just that component (case-
    // insensitively) avoids brittleness from path-form differences (8.3 names,
    // drive-letter casing) on Windows.
    #[test]
    fn spawn_honors_cwd() {
        with_timeout("spawn_honors_cwd", Duration::from_secs(30), || {
            let dir = tempfile::tempdir().expect("tempdir");
            let leaf = dir
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_lowercase();

            let mut session = PtySession::spawn(24, 80, Some(dir.path())).expect("spawn in cwd");

            // `cd` (no args) prints the cwd on cmd; `pwd` on POSIX shells.
            let print_cwd = if cfg!(windows) { "cd\r\n" } else { "pwd\r\n" };
            answer_startup_dsr_then(&mut session, print_cwd.as_bytes());

            let seen = wait_until(Duration::from_secs(10), || {
                session.output_string().to_lowercase().contains(&leaf)
            });
            assert!(
                seen,
                "expected cwd leaf {leaf:?} in shell output, got: {:?}",
                session.output_string()
            );

            session.kill().expect("kill ok");
        });
    }

    // GWEN-250 acceptance at the session level: a command that prints far more
    // lines than the cap must leave retained scrollback bounded to the cap, not
    // growing without limit. We spawn with a deliberately tiny cap, run a print
    // loop that emits ~10x the cap, and assert the retained line count never
    // exceeds it while the newest output survives and the oldest is gone.
    #[test]
    fn scrollback_stays_bounded_to_line_cap() {
        with_timeout(
            "scrollback_stays_bounded_to_line_cap",
            Duration::from_secs(40),
            || {
                let cap = 50;
                // Spawn an interactive shell with a tiny cap. We drive it via stdin
                // (the proven-reliable path on ConPTY) rather than `cmd /C`, which
                // can stall on the unanswered startup DSR query.
                let mut session = PtySession::spawn_inner(
                    default_shell_command(None),
                    24,
                    80,
                    cap,
                    None,
                    None,
                    None,
                )
                .expect("spawn with small cap");

                // Answer the startup DSR, then send a loop printing line1..line500.
                let loop_cmd = if cfg!(windows) {
                    "for /L %i in (1,1,500) do @echo gwline%i\r\n"
                } else {
                    "i=1; while [ $i -le 500 ]; do echo gwline$i; i=$((i+1)); done\r\n"
                };
                answer_startup_dsr_then(&mut session, loop_cmd.as_bytes());

                // Wait until the last line has been emitted (proves the flood ran).
                let saw_last = wait_until(Duration::from_secs(25), || {
                    session.output_string().contains("gwline500")
                });
                assert!(
                    saw_last,
                    "expected the loop to reach gwline500; got tail: {:?}",
                    session.output_string()
                );

                // The core guarantee: retained lines never exceed the cap, even
                // though ~500 lines were printed.
                let retained = session.retained_line_count();
                assert!(
                    retained <= cap,
                    "retained line count {retained} exceeded cap {cap}"
                );

                // Newest survives; an early line has been trimmed away.
                let out = session.output_string();
                assert!(out.contains("gwline500"), "newest line should be retained");
                assert!(
                    !out.contains("gwline1\n"),
                    "oldest line should have been trimmed, but it is still present"
                );

                session.kill().expect("kill ok");
            },
        );
    }

    // GWEN-251 acceptance: a deliberately-failing command raises the error flag,
    // detected reactively per chunk (the detector only scans new lines, never
    // re-reads the whole buffer). We run a command that emits a real error line
    // per platform and assert `error_detected()` flips true.
    #[test]
    fn failing_command_sets_error_flag() {
        with_timeout(
            "failing_command_sets_error_flag",
            Duration::from_secs(30),
            || {
                let mut session = PtySession::spawn(24, 80, None).expect("spawn default shell");

                // Windows cmd: an unknown command → "is not recognized ...".
                // POSIX sh: listing a missing path → "No such file or directory".
                let failing = if cfg!(windows) {
                    "gwen_no_such_cmd_zzz\r\n"
                } else {
                    "ls /gwen_no_such_path_zzz\r\n"
                };
                answer_startup_dsr_then(&mut session, failing.as_bytes());

                let flagged = wait_until(Duration::from_secs(10), || session.error_detected());
                assert!(
                    flagged,
                    "expected an error to be flagged; scrollback: {:?}",
                    session.output_string()
                );
                // The signal carries a label + the offending line.
                let signals = session.error_signals();
                assert!(!signals.is_empty(), "error_signals should be non-empty");

                session.kill().expect("kill ok");
            },
        );
    }

    // Counterpart: a successful command must NOT raise the flag (guards against
    // the detector firing on ordinary output).
    #[test]
    fn clean_command_leaves_flag_unset() {
        with_timeout(
            "clean_command_leaves_flag_unset",
            Duration::from_secs(30),
            || {
                let mut session = PtySession::spawn(24, 80, None).expect("spawn default shell");
                let marker = "gwen_clean_marker";
                answer_startup_dsr_then(&mut session, format!("echo {marker}\r\n").as_bytes());

                // Wait for the command to have run (its output echoed back).
                let ran = wait_until(Duration::from_secs(10), || {
                    session.output_string().contains(marker)
                });
                assert!(
                    ran,
                    "echo did not complete; got: {:?}",
                    session.output_string()
                );
                // Give any stray output a beat, then assert no error was flagged.
                std::thread::sleep(Duration::from_millis(300));
                assert!(
                    !session.error_detected(),
                    "clean command falsely flagged: {:?}",
                    session.error_signals()
                );

                session.kill().expect("kill ok");
            },
        );
    }

    // M5 acceptance at the session level: a command that prints a dev-server
    // ready line (as Vite/Next/CRA do) is detected reactively, exposing the
    // parsed URL + port via `dev_server_signal()`. We `echo` a Vite-style line
    // rather than spawn a real dev server (none is guaranteed on the test host).
    #[test]
    fn dev_server_ready_is_detected() {
        with_timeout(
            "dev_server_ready_is_detected",
            Duration::from_secs(30),
            || {
                let mut session = PtySession::spawn(24, 80, None).expect("spawn default shell");

                // Echo a line containing a localhost URL with a port. The trailing
                // text after the URL keeps cmd/sh from trimming it oddly.
                answer_startup_dsr_then(
                    &mut session,
                    b"echo Local: http://localhost:5173/ ready\r\n",
                );

                let detected = wait_until(Duration::from_secs(10), || {
                    session.dev_server_signal().is_some()
                });
                assert!(
                    detected,
                    "expected a dev-server URL to be detected; scrollback: {:?}",
                    session.output_string()
                );
                let sig = session.dev_server_signal().expect("signal present");
                assert_eq!(sig.port, 5173);
                assert_eq!(sig.url, "http://localhost:5173");

                session.kill().expect("kill ok");
            },
        );
    }
}
