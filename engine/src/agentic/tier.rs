//! Agent autonomy tiers (M10 Wave 8).
//!
//! Three tiers control *how* the Wave-7 gates get satisfied, on top of a hard
//! safety floor that never lowers no matter the tier:
//!
//! - [`AgentTier::Ask`] (default): every mutating tool and terminal command
//!   requires explicit user approval (the Wave-7 behaviour).
//! - [`AgentTier::AcceptForMe`]: auto-approve only high-confidence, low-risk
//!   actions (small, in-workspace, non-destructive); gate everything else.
//! - [`AgentTier::FullControl`]: run autonomously, auto-approving safe and
//!   file-mutating actions; stop only at the summary and the hard floor.
//!
//! The hard floor: `Destructive`, `DependencyChanging`, and `Blocked` actions
//! ALWAYS require explicit confirmation, in every tier. `delete_file` mutations
//! are routed in as `Destructive`, so they hit the floor too.
//!
//! Pure engine code: no Tauri/UI. The runtime calls [`requires_user_approval`]
//! when it reaches a gated tool and either auto-mints the one-use approval (when
//! it returns `false`) or pauses for the user (when it returns `true`).

use serde::{Deserialize, Serialize};

use crate::agentic::tools::ToolSide;
use crate::agentic::validation::CommandRisk;

/// How the Wave-7 gates are satisfied. Persisted with the session; defaults to
/// the safest tier ([`AgentTier::Ask`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTier {
    /// Every mutating tool and terminal command needs explicit approval.
    #[default]
    Ask,
    /// Auto-approve high-confidence, low-risk actions; gate the rest.
    AcceptForMe,
    /// Run autonomously; pause only at the summary and the hard floor.
    FullControl,
}

impl AgentTier {
    /// Stable, user-facing label.
    pub fn label(self) -> &'static str {
        match self {
            AgentTier::Ask => "Ask",
            AgentTier::AcceptForMe => "Accept for Me",
            AgentTier::FullControl => "Full Control",
        }
    }
}

/// Confidence that auto-approving an action is safe (used by Accept-for-Me).
/// Conservative by construction — only `High` is ever auto-approved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionConfidence {
    Low,
    Medium,
    High,
}

/// Risk values that can never be auto-approved, regardless of tier.
fn is_hard_floor(risk: Option<CommandRisk>) -> bool {
    matches!(
        risk,
        Some(CommandRisk::Destructive | CommandRisk::DependencyChanging | CommandRisk::Blocked)
    )
}

/// Confidence for a file mutation from its size and path safety. Large or
/// out-of-workspace edits are never `High`, so Accept-for-Me will gate them.
pub fn mutation_confidence(changed_lines: usize, within_workspace: bool) -> ActionConfidence {
    if !within_workspace {
        return ActionConfidence::Low;
    }
    if changed_lines <= 40 {
        ActionConfidence::High
    } else if changed_lines <= 120 {
        ActionConfidence::Medium
    } else {
        ActionConfidence::Low
    }
}

/// Confidence for a terminal command from its risk classification. Only plain
/// safe checks are `High`; in-place writes are `Medium`; the rest are `Low`
/// (and the dangerous ones are caught by the hard floor anyway).
pub fn command_confidence(risk: CommandRisk) -> ActionConfidence {
    match risk {
        CommandRisk::SafeCheck => ActionConfidence::High,
        CommandRisk::FileMutating => ActionConfidence::Medium,
        _ => ActionConfidence::Low,
    }
}

/// Does this action require explicit user approval under `tier`?
///
/// `risk` is `Some` for terminal commands (its classification) and for
/// destructive file ops (`delete_file` → `Some(Destructive)`); plain
/// `edit_file`/`write_file` pass `None`. `confidence` comes from
/// [`mutation_confidence`] / [`command_confidence`].
///
/// Returns `true` when the runtime must pause for the user, `false` when the
/// tier permits auto-approval. The hard floor forces `true` before any tier
/// logic runs.
pub fn requires_user_approval(
    side: ToolSide,
    risk: Option<CommandRisk>,
    confidence: ActionConfidence,
    tier: AgentTier,
) -> bool {
    // Read tools are never gated here.
    if matches!(side, ToolSide::Read) {
        return false;
    }
    // Hard floor: destructive / dependency-changing / blocked always pause.
    if is_hard_floor(risk) {
        return true;
    }
    match tier {
        AgentTier::Ask => true,
        AgentTier::AcceptForMe => confidence != ActionConfidence::High,
        AgentTier::FullControl => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_serializes_snake_case() {
        assert_eq!(
            serde_json::to_value(AgentTier::AcceptForMe).unwrap(),
            serde_json::json!("accept_for_me")
        );
        assert_eq!(AgentTier::default(), AgentTier::Ask);
    }

    #[test]
    fn ask_tier_gates_every_mutation_and_command() {
        // No matter the confidence/risk, Ask always pauses for mutations + terminal.
        for risk in [None, Some(CommandRisk::SafeCheck), Some(CommandRisk::FileMutating)] {
            assert!(requires_user_approval(
                ToolSide::Mutating,
                risk,
                ActionConfidence::High,
                AgentTier::Ask
            ));
        }
        assert!(requires_user_approval(
            ToolSide::Terminal,
            Some(CommandRisk::SafeCheck),
            ActionConfidence::High,
            AgentTier::Ask
        ));
    }

    #[test]
    fn hard_floor_always_requires_confirmation_in_every_tier() {
        for tier in [AgentTier::Ask, AgentTier::AcceptForMe, AgentTier::FullControl] {
            for risk in [
                CommandRisk::Destructive,
                CommandRisk::DependencyChanging,
                CommandRisk::Blocked,
            ] {
                assert!(
                    requires_user_approval(
                        ToolSide::Terminal,
                        Some(risk),
                        ActionConfidence::High,
                        tier
                    ),
                    "tier {tier:?} must gate {risk:?}"
                );
            }
            // delete_file enters as a destructive mutation and must always pause.
            assert!(requires_user_approval(
                ToolSide::Mutating,
                Some(CommandRisk::Destructive),
                ActionConfidence::High,
                tier
            ));
        }
    }

    #[test]
    fn accept_for_me_auto_approves_only_high_confidence() {
        // High-confidence small in-workspace edit → auto.
        assert!(!requires_user_approval(
            ToolSide::Mutating,
            None,
            ActionConfidence::High,
            AgentTier::AcceptForMe
        ));
        // Medium/Low confidence → still gated.
        assert!(requires_user_approval(
            ToolSide::Mutating,
            None,
            ActionConfidence::Medium,
            AgentTier::AcceptForMe
        ));
        // Safe check command (High) → auto; file-mutating command (Medium) → gated.
        assert!(!requires_user_approval(
            ToolSide::Terminal,
            Some(CommandRisk::SafeCheck),
            command_confidence(CommandRisk::SafeCheck),
            AgentTier::AcceptForMe
        ));
        assert!(requires_user_approval(
            ToolSide::Terminal,
            Some(CommandRisk::FileMutating),
            command_confidence(CommandRisk::FileMutating),
            AgentTier::AcceptForMe
        ));
    }

    #[test]
    fn full_control_auto_approves_safe_and_file_mutating_not_floor() {
        // Non-floor mutations + commands → auto, regardless of confidence.
        assert!(!requires_user_approval(
            ToolSide::Mutating,
            None,
            ActionConfidence::Low,
            AgentTier::FullControl
        ));
        assert!(!requires_user_approval(
            ToolSide::Terminal,
            Some(CommandRisk::FileMutating),
            ActionConfidence::Low,
            AgentTier::FullControl
        ));
        // But the floor still pauses it.
        assert!(requires_user_approval(
            ToolSide::Terminal,
            Some(CommandRisk::Destructive),
            ActionConfidence::Low,
            AgentTier::FullControl
        ));
    }

    #[test]
    fn mutation_confidence_is_conservative() {
        assert_eq!(mutation_confidence(10, true), ActionConfidence::High);
        assert_eq!(mutation_confidence(80, true), ActionConfidence::Medium);
        assert_eq!(mutation_confidence(500, true), ActionConfidence::Low);
        // Out of workspace is never high.
        assert_eq!(mutation_confidence(1, false), ActionConfidence::Low);
    }
}
