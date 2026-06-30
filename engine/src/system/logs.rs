//! Local log and crash report foundation (M14 Wave 7).
//!
//! ## Log storage
//!
//! Workspace-specific logs live under `.gwenland/logs/` when a workspace is
//! open. App-wide logs (no workspace) go to the OS app-data directory. Logs
//! are never sent anywhere by default.
//!
//! ## Crash reports
//!
//! Crash reports are stored as redacted JSONL under `.gwenland/logs/crash/`
//! (workspace) or `<app-data>/crash/` (no workspace). They include:
//!
//! - timestamp, app version, platform
//! - error summary (bounded, redacted)
//! - stack/output excerpt (bounded, redacted)
//!
//! They explicitly do NOT include:
//! - API keys, tokens, bearer headers
//! - Full terminal history
//! - Full file contents
//! - Workspace data beyond a short redacted excerpt
//!
//! ## Upload/export policy (Requirement 14 / 7.3)
//!
//! Crash report upload is **manual and opt-in only**. No upload happens
//! automatically. Any future export path must:
//! 1. Pass through the Safety Engine as a `SafetyActionKind::RemoteExport`
//! 2. Show the user exactly what will be sent before it is sent.
//!
//! Nothing in this module initiates any network connection.

use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Log path helpers (7.1)
// ---------------------------------------------------------------------------

/// Path to workspace-specific log directory: `.gwenland/logs/`.
pub fn workspace_logs_dir(workspace_root: &Path) -> PathBuf {
    crate::workspace::logs_dir(workspace_root)
}

/// Path to workspace crash log directory: `.gwenland/logs/crash/`.
pub fn workspace_crash_dir(workspace_root: &Path) -> PathBuf {
    workspace_logs_dir(workspace_root).join("crash")
}

/// Path to app-wide crash log directory in OS app data (used when no
/// workspace is open). Falls back to a `crash/` dir next to the executable
/// if app-data resolution fails.
pub fn app_crash_dir() -> PathBuf {
    crate::app_data::get_app_data_dir()
        .map(|d| d.join("crash"))
        .unwrap_or_else(|_| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.join("crash")))
                .unwrap_or_else(|| PathBuf::from("crash"))
        })
}

// ---------------------------------------------------------------------------
// Crash report schema (7.2)
// ---------------------------------------------------------------------------

/// A redacted, bounded crash report stored locally.
///
/// What is included (all bounded + redacted):
/// - `timestamp`, `app_version`, `platform`
/// - `error_summary` — first 512 chars of the error message, redacted
/// - `stack_excerpt` — first 2048 chars of a stack trace or output, redacted
///
/// What is NOT included: full file contents, full terminal history, API keys,
/// auth headers, or workspace data beyond the bounded excerpts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashReport {
    pub id: String,
    pub timestamp: String,
    pub app_version: String,
    pub platform: String,
    pub error_summary: String,
    pub stack_excerpt: String,
}

impl CrashReport {
    /// Build a new crash report, redacting secrets and bounding all strings.
    pub fn new(
        app_version: impl Into<String>,
        error_summary: impl Into<String>,
        stack_excerpt: impl Into<String>,
    ) -> Self {
        use crate::agentic::policy::redact_secrets;

        let raw_summary = error_summary.into();
        let raw_stack = stack_excerpt.into();

        let (summary_redacted, _) = redact_secrets(&raw_summary);
        let (stack_redacted, _) = redact_secrets(&raw_stack);

        let summary_bounded = bound_str(&summary_redacted, 512);
        let stack_bounded = bound_str(&stack_redacted, 2048);

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: now_rfc3339(),
            app_version: app_version.into(),
            platform: platform_string(),
            error_summary: summary_bounded,
            stack_excerpt: stack_bounded,
        }
    }
}

/// Write a crash report to `crash_dir` as a JSONL line (one report per line).
/// The crash directory is created lazily. Returns the path it was written to.
pub fn write_crash_report(crash_dir: &Path, report: &CrashReport) -> Result<PathBuf, LogError> {
    std::fs::create_dir_all(crash_dir)?;
    let path = crash_dir.join("crashes.jsonl");
    let mut line = serde_json::to_string(report)?;
    line.push('\n');
    let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
    f.write_all(line.as_bytes())?;
    Ok(path)
}

/// Write a crash report to the workspace crash directory, or the app crash
/// directory when `workspace_root` is `None`.
pub fn record_crash(
    workspace_root: Option<&Path>,
    app_version: &str,
    error_summary: &str,
    stack_excerpt: &str,
) -> Result<PathBuf, LogError> {
    let report = CrashReport::new(app_version, error_summary, stack_excerpt);
    let dir = workspace_root
        .map(workspace_crash_dir)
        .unwrap_or_else(app_crash_dir);
    write_crash_report(&dir, &report)
}

/// Read all crash reports from a crash directory. Skips malformed lines.
pub fn read_crash_reports(crash_dir: &Path) -> Vec<CrashReport> {
    let path = crash_dir.join("crashes.jsonl");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    content
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn bound_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    const SUFFIX: &str = "...";
    let suffix_chars = SUFFIX.chars().count();
    if max_chars <= suffix_chars {
        return s.chars().take(max_chars).collect();
    }
    let truncated: String = s.chars().take(max_chars - suffix_chars).collect();
    format!("{truncated}{SUFFIX}")
}

fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

fn platform_string() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum LogError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // 7.1 — workspace log path is under .gwenland/logs
    #[test]
    fn workspace_logs_dir_is_under_gwenland() {
        let dir = tempdir().unwrap();
        let logs = workspace_logs_dir(dir.path());

        assert_eq!(logs, dir.path().join(".gwenland").join("logs"));
    }

    // 7.1 — workspace crash dir is under logs/crash
    #[test]
    fn workspace_crash_dir_is_under_logs() {
        let dir = tempdir().unwrap();
        let crash = workspace_crash_dir(dir.path());

        assert_eq!(
            crash,
            dir.path().join(".gwenland").join("logs").join("crash")
        );
    }

    // 7.2 — crash report is created with correct fields
    #[test]
    fn crash_report_has_required_fields() {
        let r = CrashReport::new("1.0.0", "NullPointerException in foo()", "at bar:42");
        assert!(!r.id.is_empty());
        assert!(!r.timestamp.is_empty());
        assert_eq!(r.app_version, "1.0.0");
        assert_eq!(r.error_summary, "NullPointerException in foo()");
        assert_eq!(r.stack_excerpt, "at bar:42");
        assert!(r.platform.contains(std::env::consts::OS));
        assert!(r.platform.contains(std::env::consts::ARCH));
    }

    // 7.2 — crash report bounds long strings
    #[test]
    fn crash_report_bounds_long_strings() {
        let long = "x".repeat(10_000);
        let r = CrashReport::new("1.0.0", &long, &long);

        assert_eq!(r.error_summary.chars().count(), 512);
        assert_eq!(r.stack_excerpt.chars().count(), 2048);
        assert!(r.error_summary.ends_with("..."));
        assert!(r.stack_excerpt.ends_with("..."));
    }

    // 7.2 — crash report redacts secrets
    #[test]
    fn crash_report_redacts_secrets() {
        let secret_summary = "Failed: api_key=sk-ant-abcdefghijklmnopqrstuvwxyz01234";
        let r = CrashReport::new("1.0.0", secret_summary, "stack");
        assert!(
            !r.error_summary.contains("sk-ant-abcdefghijklmnopq"),
            "secret must not appear in crash report"
        );
        assert!(r.error_summary.contains("[REDACTED]"));
    }

    // 7.2 — write and read round-trip
    #[test]
    fn crash_write_and_read_round_trip() {
        let dir = tempdir().unwrap();
        let r = CrashReport::new("1.0.0", "oops", "stack trace here");
        let path = write_crash_report(dir.path(), &r).unwrap();
        assert!(path.exists());

        let reports = read_crash_reports(dir.path());
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].id, r.id);
        assert_eq!(reports[0].app_version, "1.0.0");
    }

    // 7.2 — multiple reports append without overwriting
    #[test]
    fn crash_reports_append_without_overwriting() {
        let dir = tempdir().unwrap();
        for i in 0..3 {
            let r = CrashReport::new("1.0.0", &format!("error {i}"), "stack");
            write_crash_report(dir.path(), &r).unwrap();
        }
        let reports = read_crash_reports(dir.path());
        assert_eq!(reports.len(), 3);
    }

    // 7.2 — malformed lines do not prevent reading valid ones
    #[test]
    fn malformed_crash_lines_skipped() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("crashes.jsonl");
        let r = CrashReport::new("1.0.0", "ok error", "stack");
        let valid_line = serde_json::to_string(&r).unwrap();
        std::fs::write(&path, format!("not-json\n{valid_line}\n")).unwrap();
        let reports = read_crash_reports(dir.path());
        assert_eq!(reports.len(), 1, "only the valid line should be read");
    }

    #[test]
    fn record_crash_writes_redacted_workspace_report() {
        let dir = tempdir().unwrap();
        let path = record_crash(
            Some(dir.path()),
            "1.2.3",
            "failed with api_key=sk-ant-abcdefghijklmnopqrstuvwxyz01234",
            "stack saw api_key=sk-ant-abcdefghijklmnopqrstuvwxyz56789\nframe",
        )
        .unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        let reports = read_crash_reports(&workspace_crash_dir(dir.path()));

        assert_eq!(path, workspace_crash_dir(dir.path()).join("crashes.jsonl"));
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].app_version, "1.2.3");
        assert!(reports[0].error_summary.contains("[REDACTED]"));
        assert!(reports[0].stack_excerpt.contains("[REDACTED]"));
        assert!(!raw.contains("sk-ant-abcdefghijklmnopqrstuvwxyz01234"));
        assert!(!raw.contains("sk-ant-abcdefghijklmnopqrstuvwxyz56789"));
    }

    // record_crash helper stores in workspace crash dir
    #[test]
    fn record_crash_uses_workspace_dir() {
        let dir = tempdir().unwrap();
        let path = record_crash(Some(dir.path()), "1.0.0", "oops", "stack").unwrap();
        assert!(path.starts_with(workspace_crash_dir(dir.path())));
    }
}
