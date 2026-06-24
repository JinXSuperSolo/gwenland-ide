//! Session summary model + deterministic local fallback (M10, Requirement 8.6).
//!
//! The summary closes the loop. A model-written narrative is preferred, but when
//! the provider call fails we still produce a useful, fully deterministic
//! summary computed locally from the apply report and validation runs — so a
//! provider outage never blocks completion. Pure engine code.

use serde::{Deserialize, Serialize};

use crate::agentic::changeset::ApplyReport;
use crate::agentic::validation::{ValidationRun, ValidationStatus};

/// The final report for a completed session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSummary {
    pub id: String,
    pub goal: String,
    pub plan_title: String,
    pub changed_files: Vec<String>,
    pub applied_count: usize,
    pub rejected_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
    /// One-line validation roll-up, e.g. "1 passed, 1 failed".
    pub validation_status: String,
    pub unresolved_risks: Vec<String>,
    pub follow_ups: Vec<String>,
    /// Human-readable narrative (model-written, or the deterministic fallback).
    pub text: String,
    /// True when `text` came from the local fallback rather than a provider.
    pub local_fallback: bool,
}

/// One-line roll-up of validation runs.
pub fn validation_status_line(runs: &[ValidationRun]) -> String {
    if runs.is_empty() {
        return "no validation run".to_string();
    }
    let mut passed = 0;
    let mut failed = 0;
    let mut other = 0;
    for r in runs {
        match r.status {
            ValidationStatus::Passed => passed += 1,
            ValidationStatus::Failed => failed += 1,
            _ => other += 1,
        }
    }
    let mut parts = Vec::new();
    if passed > 0 {
        parts.push(format!("{passed} passed"));
    }
    if failed > 0 {
        parts.push(format!("{failed} failed"));
    }
    if other > 0 {
        parts.push(format!("{other} other"));
    }
    parts.join(", ")
}

/// Build a deterministic summary from the run facts, with no provider call.
/// Used both as the Wave 6 fallback and as the structured backbone a model
/// narrative is attached to.
pub fn build_local_summary(
    id: impl Into<String>,
    goal: &str,
    plan_title: &str,
    report: &ApplyReport,
    runs: &[ValidationRun],
    unresolved_risks: Vec<String>,
) -> AgentSummary {
    let changed_files: Vec<String> = report.applied.iter().map(|o| o.path.clone()).collect();
    let validation_status = validation_status_line(runs);

    let mut follow_ups = Vec::new();
    if !report.failed.is_empty() {
        follow_ups.push("Revisit failed hunks and request revised edits.".to_string());
    }
    if runs.iter().any(|r| r.status == ValidationStatus::Failed) {
        follow_ups.push("Address validation failures, then re-run checks.".to_string());
    }
    if report.applied.is_empty() {
        follow_ups.push("No changes were applied; refine the plan or approve hunks.".to_string());
    }

    let text = format!(
        "Goal: {goal}\nPlan: {plan_title}\nApplied {} change(s){}; {} rejected, {} failed, {} skipped.\nValidation: {validation_status}.",
        report.applied.len(),
        if changed_files.is_empty() {
            String::new()
        } else {
            format!(" to {}", changed_files.join(", "))
        },
        report.rejected.len(),
        report.failed.len(),
        report.skipped.len(),
    );

    AgentSummary {
        id: id.into(),
        goal: goal.to_string(),
        plan_title: plan_title.to_string(),
        changed_files,
        applied_count: report.applied.len(),
        rejected_count: report.rejected.len(),
        failed_count: report.failed.len(),
        skipped_count: report.skipped.len(),
        validation_status,
        unresolved_risks,
        follow_ups,
        text,
        local_fallback: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::changeset::{ApplyOutcome, ApplyReport};

    fn outcome(path: &str) -> ApplyOutcome {
        ApplyOutcome {
            file_id: "f".into(),
            path: path.into(),
            hunk_ids: vec!["h".into()],
            message: "applied".into(),
        }
    }

    #[test]
    fn local_summary_counts_and_lists_changed_files() {
        let report = ApplyReport {
            applied: vec![outcome("src/a.rs"), outcome("src/b.rs")],
            failed: vec![outcome("src/c.rs")],
            ..Default::default()
        };
        let s = build_local_summary("sum-1", "do it", "My Plan", &report, &[], vec![]);
        assert_eq!(s.applied_count, 2);
        assert_eq!(s.failed_count, 1);
        assert_eq!(s.changed_files, vec!["src/a.rs", "src/b.rs"]);
        assert!(s.local_fallback);
        assert!(s.text.contains("My Plan"));
        // A failed hunk produces a follow-up suggestion.
        assert!(s.follow_ups.iter().any(|f| f.contains("failed hunks")));
    }

    #[test]
    fn validation_status_line_summarizes_runs() {
        assert_eq!(validation_status_line(&[]), "no validation run");
        let runs = vec![
            ValidationRun {
                id: "r1".into(),
                command_id: "c1".into(),
                status: ValidationStatus::Passed,
                exit_code: Some(0),
                output_excerpt: String::new(),
                started_at: "t".into(),
                finished_at: Some("t".into()),
            },
            ValidationRun {
                id: "r2".into(),
                command_id: "c2".into(),
                status: ValidationStatus::Failed,
                exit_code: Some(1),
                output_excerpt: String::new(),
                started_at: "t".into(),
                finished_at: Some("t".into()),
            },
        ];
        assert_eq!(validation_status_line(&runs), "1 passed, 1 failed");
    }
}
