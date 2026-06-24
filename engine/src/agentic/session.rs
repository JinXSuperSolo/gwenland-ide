//! Agent session + phase machine (M10, Requirements 1, 2, 8).
//!
//! The session is the single source of truth for one human-gated coding task:
//! its goal, context, plan, approvals, change sets, validation runs, and
//! summary. The phase machine enforces the loop's invariants — most importantly
//! that edits cannot be drafted before the plan is approved, edits cannot be
//! applied before a hunk/file is approved, and validation cannot run before a
//! command is approved (Requirement 1.2-1.4).
//!
//! Pure engine code: no Tauri, no provider keys (those live only in the OS
//! keychain and are fetched Tauri-side at send time).

use serde::{Deserialize, Serialize};

use crate::agentic::changeset::{ApplyReport, ChangeSet};
use crate::agentic::context::ContextPreview;
use crate::agentic::summary::AgentSummary;
use crate::agentic::validation::{ValidationCommand, ValidationRun};

/// The phases of the human-gated loop (Requirement 1.1). `Goal` is the initial
/// state after creation, before the first plan is requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPhase {
    /// Session created; goal + context being assembled, no plan requested yet.
    Goal,
    /// A plan request is streaming.
    DraftingPlan,
    /// Plan streamed; awaiting approve/reject/revise.
    AwaitingPlanApproval,
    /// An edit request is streaming (only reachable after plan approval).
    DraftingEdits,
    /// Edits parsed into a change set; awaiting hunk/file approval.
    AwaitingEditApproval,
    /// Approved hunks are being applied.
    ApplyingApprovedEdits,
    /// Awaiting approval of a validation command.
    AwaitingValidationApproval,
    /// An approved validation command is running.
    Validating,
    /// Building the final summary.
    Summarizing,
    /// Terminal: finished successfully.
    Complete,
    /// Terminal: unrecoverable failure.
    Failed,
    /// Terminal: user-cancelled.
    Cancelled,
}

impl AgentPhase {
    /// Terminal phases accept no further transitions.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            AgentPhase::Complete | AgentPhase::Failed | AgentPhase::Cancelled
        )
    }
}

/// Status of a single plan step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStepStatus {
    Pending,
    InProgress,
    Done,
    Skipped,
}

/// One step of an implementation plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
    pub id: String,
    pub label: String,
    pub description: String,
    pub status: PlanStepStatus,
}

/// A model-produced plan the user must approve before edits are requested
/// (Requirement 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPlan {
    pub id: String,
    pub title: String,
    pub assumptions: Vec<String>,
    pub steps: Vec<PlanStep>,
    pub likely_files: Vec<String>,
    pub risks: Vec<String>,
    pub suggested_validation: Vec<ValidationCommand>,
    /// Context the plan says it still needs from the user (Requirement 2.3).
    pub missing_context: Vec<String>,
}

/// What an approval unlocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalKind {
    /// Approves a plan, unlocking edit drafting.
    Plan,
    /// Approves a change set's hunks, unlocking apply.
    Edits,
    /// Approves one validation command, unlocking its run.
    ValidationCommand,
}

/// A one-use, session-scoped approval token tied to a specific target
/// (Requirement 1, design "Command contracts"). `consumed` is flipped when the
/// unlocked action runs, so an approval cannot be replayed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub id: String,
    pub kind: ApprovalKind,
    /// The plan id / change-set id / command id this approves.
    pub target_id: String,
    pub created_at: String,
    pub consumed: bool,
}

impl ApprovalRecord {
    pub fn new(kind: ApprovalKind, target_id: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            target_id: target_id.into(),
            created_at: crate::agentic::now_rfc3339(),
            consumed: false,
        }
    }
}

/// A transition the phase machine refused, with a user-safe reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseError {
    /// The session is in a terminal phase.
    Terminal(AgentPhase),
    /// `from -> to` is not a legal edge.
    Illegal { from: AgentPhase, to: AgentPhase },
    /// `to` is gated on an approval/state that is not satisfied.
    MissingApproval { to: AgentPhase, what: &'static str },
}

impl std::fmt::Display for PhaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PhaseError::Terminal(p) => write!(f, "session is in terminal phase {p:?}"),
            PhaseError::Illegal { from, to } => write!(f, "illegal transition {from:?} -> {to:?}"),
            PhaseError::MissingApproval { to, what } => {
                write!(f, "cannot enter {to:?}: {what}")
            }
        }
    }
}

impl std::error::Error for PhaseError {}

/// One human-gated coding task. Serializable for persistence (Requirement 8.1),
/// but note: runtime stream handles and provider keys are deliberately NOT part
/// of this struct (Requirement 8.2/8.3) — they live Tauri-side / in the keychain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub project_root: String,
    pub goal: String,
    pub phase: AgentPhase,
    /// True when this session was restored after a non-resumable in-flight
    /// phase. Runtime stream handles are never persisted, so streaming phases
    /// resume at the nearest safe review point with this flag set.
    #[serde(default)]
    pub interrupted: bool,
    pub provider: String,
    pub model: String,
    /// Autonomy tier governing how Wave-7 gates are satisfied (M10 Wave 8).
    /// Defaults to the safest tier; `#[serde(default)]` keeps older persisted
    /// sessions loadable.
    #[serde(default)]
    pub tier: crate::agentic::tier::AgentTier,
    pub context: ContextPreview,
    pub plan: Option<AgentPlan>,
    pub approvals: Vec<ApprovalRecord>,
    pub change_sets: Vec<ChangeSet>,
    pub apply_report: Option<ApplyReport>,
    pub validation_runs: Vec<ValidationRun>,
    pub summary: Option<AgentSummary>,
}

impl AgentSession {
    /// Create a session in the `Goal` phase.
    pub fn new(
        id: impl Into<String>,
        project_root: impl Into<String>,
        goal: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
        context: ContextPreview,
    ) -> Self {
        Self {
            id: id.into(),
            project_root: project_root.into(),
            goal: goal.into(),
            phase: AgentPhase::Goal,
            interrupted: false,
            provider: provider.into(),
            model: model.into(),
            tier: crate::agentic::tier::AgentTier::default(),
            context,
            plan: None,
            approvals: Vec::new(),
            change_sets: Vec::new(),
            apply_report: None,
            validation_runs: Vec::new(),
            summary: None,
        }
    }

    /// The most recent change set, if any.
    pub fn latest_change_set(&self) -> Option<&ChangeSet> {
        self.change_sets.last()
    }

    /// Change the autonomy tier (M10 Wave 8). Allowed only "between iterations":
    /// not while terminal, and not mid-stream/apply/validation so a tier can't be
    /// lowered out from under an action that is already in flight. Returns whether
    /// the change was applied.
    pub fn set_tier(&mut self, tier: crate::agentic::tier::AgentTier) -> bool {
        let mid_action = matches!(
            self.phase,
            AgentPhase::DraftingPlan
                | AgentPhase::DraftingEdits
                | AgentPhase::ApplyingApprovedEdits
                | AgentPhase::Validating
                | AgentPhase::Summarizing
        );
        if self.phase.is_terminal() || mid_action {
            return false;
        }
        self.tier = tier;
        true
    }

    /// Is there an unconsumed approval of `kind` for `target_id`?
    pub fn has_active_approval(&self, kind: ApprovalKind, target_id: &str) -> bool {
        self.approvals
            .iter()
            .any(|a| a.kind == kind && a.target_id == target_id && !a.consumed)
    }

    /// Record an approval (does not transition phase).
    pub fn record_approval(
        &mut self,
        kind: ApprovalKind,
        target_id: impl Into<String>,
    ) -> ApprovalRecord {
        let record = ApprovalRecord::new(kind, target_id);
        self.approvals.push(record.clone());
        record
    }

    /// Consume the first matching unconsumed approval. Returns true if one was
    /// consumed (enforces one-use; Requirement design "Approval ids are one-use").
    pub fn consume_approval(&mut self, kind: ApprovalKind, target_id: &str) -> bool {
        if let Some(a) = self
            .approvals
            .iter_mut()
            .find(|a| a.kind == kind && a.target_id == target_id && !a.consumed)
        {
            a.consumed = true;
            true
        } else {
            false
        }
    }

    /// Whether `to` is reachable from the current phase, including approval
    /// gates. Pure check; does not mutate.
    pub fn can_transition(&self, to: AgentPhase) -> Result<(), PhaseError> {
        let from = self.phase;
        if from.is_terminal() {
            return Err(PhaseError::Terminal(from));
        }
        // Any non-terminal phase may be cancelled (Req 1.5) or fail.
        if matches!(to, AgentPhase::Cancelled | AgentPhase::Failed) {
            return Ok(());
        }
        if !is_legal_edge(from, to) {
            return Err(PhaseError::Illegal { from, to });
        }
        // Approval gates for the protected targets.
        match to {
            AgentPhase::DraftingEdits => {
                let plan_id = self.plan.as_ref().map(|p| p.id.as_str()).unwrap_or("");
                if plan_id.is_empty() || !self.has_active_approval(ApprovalKind::Plan, plan_id) {
                    return Err(PhaseError::MissingApproval {
                        to,
                        what: "the plan must be approved first",
                    });
                }
            }
            AgentPhase::ApplyingApprovedEdits => {
                let approved = self
                    .latest_change_set()
                    .map(|cs| cs.has_approved_change())
                    .unwrap_or(false);
                if !approved {
                    return Err(PhaseError::MissingApproval {
                        to,
                        what: "at least one hunk or file must be approved first",
                    });
                }
            }
            AgentPhase::Validating => {
                let has_cmd_approval = self
                    .approvals
                    .iter()
                    .any(|a| a.kind == ApprovalKind::ValidationCommand && !a.consumed);
                if !has_cmd_approval {
                    return Err(PhaseError::MissingApproval {
                        to,
                        what: "a validation command must be approved first",
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Transition to `to`, enforcing [`can_transition`]. On success the phase is
    /// updated and `Ok(())` returned.
    ///
    /// [`can_transition`]: AgentSession::can_transition
    pub fn transition(&mut self, to: AgentPhase) -> Result<(), PhaseError> {
        self.can_transition(to)?;
        self.phase = to;
        Ok(())
    }
}

/// The legal forward edges of the loop (cancel/fail handled separately). Kept as
/// one readable table so the whole machine is auditable at a glance.
fn is_legal_edge(from: AgentPhase, to: AgentPhase) -> bool {
    use AgentPhase::*;
    matches!(
        (from, to),
        (Goal, DraftingPlan)
            | (DraftingPlan, AwaitingPlanApproval)
            // Re-request a plan (retry after failure or revision request).
            | (AwaitingPlanApproval, DraftingPlan)
            | (AwaitingPlanApproval, DraftingEdits)
            // Provider/edit-stream failure returns to plan approval for retry.
            | (DraftingEdits, AwaitingPlanApproval)
            | (DraftingEdits, AwaitingEditApproval)
            // Re-request edits, or step back to revise the plan.
            | (AwaitingEditApproval, DraftingEdits)
            | (AwaitingEditApproval, DraftingPlan)
            | (AwaitingEditApproval, ApplyingApprovedEdits)
            | (ApplyingApprovedEdits, AwaitingEditApproval)
            | (ApplyingApprovedEdits, AwaitingValidationApproval)
            | (ApplyingApprovedEdits, Summarizing)
            | (AwaitingEditApproval, Summarizing)
            | (AwaitingValidationApproval, Validating)
            | (AwaitingValidationApproval, Summarizing)
            // After a run, approve another command or move on.
            | (Validating, AwaitingValidationApproval)
            | (Validating, Summarizing)
            // Validation failure can loop back to a revised edit/plan request.
            | (Validating, DraftingEdits)
            | (AwaitingValidationApproval, DraftingEdits)
            | (Summarizing, Complete)
            // A streamed plan can also surface a request for more context, going
            // back to Goal.
            | (DraftingPlan, Goal)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::changeset::{ApprovalState, change_set_from_text};
    use crate::agentic::context::ContextPreview;

    fn session() -> AgentSession {
        AgentSession::new(
            "s1",
            "/proj",
            "do the thing",
            "anthropic",
            "claude-x",
            ContextPreview::new(),
        )
    }

    fn plan(id: &str) -> AgentPlan {
        AgentPlan {
            id: id.into(),
            title: "Plan".into(),
            assumptions: vec![],
            steps: vec![],
            likely_files: vec![],
            risks: vec![],
            suggested_validation: vec![],
            missing_context: vec![],
        }
    }

    #[test]
    fn starts_in_goal_phase() {
        assert_eq!(session().phase, AgentPhase::Goal);
    }

    #[test]
    fn happy_path_plan_flow() {
        let mut s = session();
        assert!(s.transition(AgentPhase::DraftingPlan).is_ok());
        assert!(s.transition(AgentPhase::AwaitingPlanApproval).is_ok());
    }

    #[test]
    fn edits_require_plan_approval() {
        let mut s = session();
        s.plan = Some(plan("plan-1"));
        s.phase = AgentPhase::AwaitingPlanApproval;

        // Without an approval record, the edit transition is rejected.
        let err = s.transition(AgentPhase::DraftingEdits).unwrap_err();
        assert!(matches!(err, PhaseError::MissingApproval { .. }));
        assert_eq!(
            s.phase,
            AgentPhase::AwaitingPlanApproval,
            "phase must not change on rejection"
        );

        // Approve the plan, then the transition is allowed.
        s.record_approval(ApprovalKind::Plan, "plan-1");
        assert!(s.transition(AgentPhase::DraftingEdits).is_ok());
    }

    #[test]
    fn approval_for_wrong_plan_does_not_unlock_edits() {
        let mut s = session();
        s.plan = Some(plan("plan-2"));
        s.phase = AgentPhase::AwaitingPlanApproval;
        s.record_approval(ApprovalKind::Plan, "plan-1"); // stale/other plan
        assert!(s.transition(AgentPhase::DraftingEdits).is_err());
    }

    #[test]
    fn edit_stream_failure_can_return_to_plan_review() {
        let mut s = session();
        s.plan = Some(plan("plan-1"));
        s.phase = AgentPhase::AwaitingPlanApproval;
        s.record_approval(ApprovalKind::Plan, "plan-1");
        assert!(s.transition(AgentPhase::DraftingEdits).is_ok());
        assert!(s.transition(AgentPhase::AwaitingPlanApproval).is_ok());
    }

    #[test]
    fn apply_requires_approved_hunk() {
        let mut s = session();
        s.phase = AgentPhase::AwaitingEditApproval;
        let cs = change_set_from_text("plan-1", "--- a/f\n+++ b/f\n@@ -1,1 +1,1 @@\n-a\n+b\n");
        s.change_sets.push(cs);

        // No hunk approved yet.
        assert!(s.transition(AgentPhase::ApplyingApprovedEdits).is_err());

        // Approve a hunk, then apply transition is allowed.
        let hunk_id = s.change_sets[0].files[0].hunks[0].id.clone();
        s.change_sets[0].set_hunk_approval(&hunk_id, ApprovalState::Approved);
        assert!(s.transition(AgentPhase::ApplyingApprovedEdits).is_ok());
    }

    #[test]
    fn validation_requires_command_approval() {
        let mut s = session();
        s.phase = AgentPhase::AwaitingValidationApproval;
        assert!(s.transition(AgentPhase::Validating).is_err());
        s.record_approval(ApprovalKind::ValidationCommand, "cmd-1");
        assert!(s.transition(AgentPhase::Validating).is_ok());
    }

    #[test]
    fn summary_can_start_from_edit_review_or_validation_review() {
        let mut from_edit_review = session();
        from_edit_review.phase = AgentPhase::AwaitingEditApproval;
        assert!(from_edit_review.transition(AgentPhase::Summarizing).is_ok());

        let mut from_validation = session();
        from_validation.phase = AgentPhase::AwaitingValidationApproval;
        assert!(from_validation.transition(AgentPhase::Summarizing).is_ok());
    }

    #[test]
    fn cancel_allowed_from_any_non_terminal_phase() {
        for phase in [
            AgentPhase::Goal,
            AgentPhase::DraftingPlan,
            AgentPhase::AwaitingPlanApproval,
            AgentPhase::AwaitingEditApproval,
            AgentPhase::Validating,
        ] {
            let mut s = session();
            s.phase = phase;
            assert!(
                s.transition(AgentPhase::Cancelled).is_ok(),
                "cancel from {phase:?}"
            );
        }
    }

    #[test]
    fn terminal_phases_reject_all_transitions() {
        for phase in [
            AgentPhase::Complete,
            AgentPhase::Failed,
            AgentPhase::Cancelled,
        ] {
            let mut s = session();
            s.phase = phase;
            assert!(matches!(
                s.transition(AgentPhase::DraftingPlan),
                Err(PhaseError::Terminal(_))
            ));
        }
    }

    #[test]
    fn illegal_skip_is_rejected() {
        let mut s = session();
        // Goal straight to applying edits is not a legal edge.
        assert!(matches!(
            s.transition(AgentPhase::ApplyingApprovedEdits),
            Err(PhaseError::MissingApproval { .. }) | Err(PhaseError::Illegal { .. })
        ));
        assert_eq!(s.phase, AgentPhase::Goal);
    }

    #[test]
    fn approvals_are_one_use() {
        let mut s = session();
        s.record_approval(ApprovalKind::ValidationCommand, "cmd-1");
        assert!(s.consume_approval(ApprovalKind::ValidationCommand, "cmd-1"));
        // Second consume fails — token cannot be replayed.
        assert!(!s.consume_approval(ApprovalKind::ValidationCommand, "cmd-1"));
    }

    #[test]
    fn session_round_trips_through_serde() {
        let mut s = session();
        s.plan = Some(plan("p"));
        let json = serde_json::to_string(&s).unwrap();
        let back: AgentSession = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
