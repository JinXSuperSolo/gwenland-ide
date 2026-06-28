//! Safety decision DTOs — what the Safety Engine concludes about an action.

use serde::{Deserialize, Serialize};

/// Risk label for an action or path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// Known-safe read-only local operation.
    Safe,
    /// Low risk, non-destructive, local mutation.
    Low,
    /// Meaningful local mutation (format, rename, write).
    Medium,
    /// Significant or hard-to-reverse local mutation.
    High,
    /// Irreversible local destruction (delete without recovery, force-push).
    Destructive,
    /// Action involves a path or value likely containing a secret.
    Secret,
    /// Action sends data to a remote service.
    Remote,
    /// Risk cannot be determined from available information.
    Unknown,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Safe => "safe",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Destructive => "destructive",
            Self::Secret => "secret",
            Self::Remote => "remote",
            Self::Unknown => "unknown",
        };
        write!(f, "{s}")
    }
}

/// What kind of confirmation the user must give.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationKind {
    /// No confirmation required.
    None,
    /// A simple yes/no prompt.
    Simple,
    /// A typed confirmation (user must type a specific word).
    Typed,
    /// An explicit danger acknowledgment with a specific warning message.
    DangerAck { warning: String },
}

/// The Safety Engine's verdict on a `SafetyAction`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyVerdict {
    /// The action is allowed without confirmation.
    Allow,
    /// The action requires user confirmation before proceeding.
    Ask,
    /// The action is blocked; the user must explicitly override via danger ack.
    Block,
}

impl std::fmt::Display for SafetyVerdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Allow => write!(f, "allow"),
            Self::Ask => write!(f, "ask"),
            Self::Block => write!(f, "block"),
        }
    }
}

/// The complete Safety Engine response to a `SafetyAction` evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyDecision {
    /// The action id this decision corresponds to.
    pub action_id: String,
    pub verdict: SafetyVerdict,
    pub risk: RiskLevel,
    /// Human-readable explanation (local string, never sent to remote).
    pub reason: String,
    /// What kind of confirmation is needed when `verdict` is `Ask`.
    pub confirmation: ConfirmationKind,
    /// Whether any protected path was matched.
    pub protected_path_matched: bool,
    /// Whether any secret path was matched.
    pub secret_path_matched: bool,
}

impl SafetyDecision {
    pub fn allow(action_id: impl Into<String>, risk: RiskLevel, reason: impl Into<String>) -> Self {
        Self {
            action_id: action_id.into(),
            verdict: SafetyVerdict::Allow,
            risk,
            reason: reason.into(),
            confirmation: ConfirmationKind::None,
            protected_path_matched: false,
            secret_path_matched: false,
        }
    }

    pub fn ask(
        action_id: impl Into<String>,
        risk: RiskLevel,
        reason: impl Into<String>,
        confirmation: ConfirmationKind,
    ) -> Self {
        Self {
            action_id: action_id.into(),
            verdict: SafetyVerdict::Ask,
            risk,
            reason: reason.into(),
            confirmation,
            protected_path_matched: false,
            secret_path_matched: false,
        }
    }

    pub fn block(action_id: impl Into<String>, risk: RiskLevel, reason: impl Into<String>) -> Self {
        Self {
            action_id: action_id.into(),
            verdict: SafetyVerdict::Block,
            risk,
            reason: reason.into(),
            confirmation: ConfirmationKind::DangerAck {
                warning: "This action is blocked by safety policy.".to_string(),
            },
            protected_path_matched: false,
            secret_path_matched: false,
        }
    }

    pub fn with_protected_path(mut self) -> Self {
        self.protected_path_matched = true;
        self
    }

    pub fn with_secret_path(mut self) -> Self {
        self.secret_path_matched = true;
        self
    }
}

/// Safety strictness level: how aggressively to gate risky actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SafetyStrictness {
    /// Ask for confirmation on medium-risk actions; block destructive/secret.
    #[default]
    Standard,
    /// Ask on low-risk; block medium/high/destructive/secret.
    Strict,
    /// Block everything except explicitly safe reads.
    Paranoid,
}
