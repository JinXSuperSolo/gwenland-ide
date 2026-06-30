//! Confirmation policy: maps (action kind, protected path, strictness) → verdict.
//!
//! Rules (in precedence order):
//! 1. Secret paths → block (unless explicitly user-approved with danger ack).
//! 2. Remote/export actions → ask.
//! 3. Unknown actions → ask (or block under Paranoid strictness).
//! 4. Destructive Git actions → ask (or block under Strict/Paranoid).
//! 5. Protected paths → ask (or block under Paranoid).
//! 6. All other actions → allow (or ask under Strict, block under Paranoid for writes).

use crate::safety::SafetyStrictness;
use crate::safety::action::{SafetyAction, SafetyActionKind};
use crate::safety::decision::{ConfirmationKind, RiskLevel, SafetyDecision};
use crate::safety::protected_paths::ProtectedPathRegistry;

/// Evaluate `action` against `registry` and `strictness`, returning a
/// `SafetyDecision`. This is the single authoritative policy function —
/// no Tauri/UI/browser imports may appear here.
pub fn evaluate(
    action: &SafetyAction,
    registry: &ProtectedPathRegistry,
    strictness: SafetyStrictness,
) -> SafetyDecision {
    let id = &action.id;

    match &action.kind {
        // ---- File reads --------------------------------------------------
        SafetyActionKind::FileRead { path } => {
            if let Some((entry, is_secret)) = registry.classify(path) {
                if is_secret {
                    return secret_block(id, path);
                }
                return protected_ask(id, path, entry.risk, strictness);
            }
            SafetyDecision::allow(id, RiskLevel::Safe, "local file read")
        }

        // ---- File create -------------------------------------------------
        SafetyActionKind::FileCreate { path } => {
            if let Some((entry, is_secret)) = registry.classify(path) {
                if is_secret {
                    return secret_block(id, path);
                }
                return protected_ask(id, path, entry.risk, strictness);
            }
            maybe_ask(
                id,
                RiskLevel::Low,
                "create local file",
                strictness,
                SafetyStrictness::Paranoid,
            )
        }

        // ---- File write --------------------------------------------------
        SafetyActionKind::FileWrite { path } => {
            if let Some((entry, is_secret)) = registry.classify(path) {
                if is_secret {
                    return secret_block(id, path);
                }
                return protected_ask(id, path, entry.risk, strictness);
            }
            maybe_ask(
                id,
                RiskLevel::Medium,
                "write to local file",
                strictness,
                SafetyStrictness::Strict,
            )
        }

        // ---- File delete (always ask) ------------------------------------
        SafetyActionKind::FileDelete { path } => {
            if let Some((_entry, is_secret)) = registry.classify(path)
                && is_secret
            {
                return secret_block(id, path);
            }
            match strictness {
                SafetyStrictness::Paranoid => SafetyDecision::block(
                    id,
                    RiskLevel::Destructive,
                    "file delete blocked in paranoid mode",
                ),
                _ => SafetyDecision::ask(
                    id,
                    RiskLevel::High,
                    "deleting a file is irreversible without recovery",
                    ConfirmationKind::Simple,
                ),
            }
        }

        // ---- File rename / copy ------------------------------------------
        SafetyActionKind::FileRename { old_path, new_path } => {
            for p in [old_path.as_str(), new_path.as_str()] {
                if let Some((_e, is_secret)) = registry.classify(p)
                    && is_secret
                {
                    return secret_block(id, p);
                }
            }
            maybe_ask(
                id,
                RiskLevel::Medium,
                "rename/move file",
                strictness,
                SafetyStrictness::Strict,
            )
        }

        SafetyActionKind::FileCopy { src, dest } => {
            for p in [src.as_str(), dest.as_str()] {
                if let Some((_e, is_secret)) = registry.classify(p)
                    && is_secret
                {
                    return secret_block(id, p);
                }
            }
            maybe_ask(
                id,
                RiskLevel::Low,
                "copy file",
                strictness,
                SafetyStrictness::Paranoid,
            )
        }

        // ---- Terminal commands -------------------------------------------
        SafetyActionKind::TerminalCommand { command } => {
            use crate::agentic::policy::classify_command;
            use crate::agentic::validation::CommandRisk;

            let risk = classify_command(command);
            match risk {
                CommandRisk::SafeCheck => maybe_ask(
                    id,
                    RiskLevel::Safe,
                    "safe check command",
                    strictness,
                    SafetyStrictness::Paranoid,
                ),
                CommandRisk::FileMutating => SafetyDecision::ask(
                    id,
                    RiskLevel::Medium,
                    "command mutates files",
                    ConfirmationKind::Simple,
                ),
                CommandRisk::DependencyChanging => SafetyDecision::ask(
                    id,
                    RiskLevel::Medium,
                    "command changes dependencies",
                    ConfirmationKind::Simple,
                ),
                CommandRisk::Destructive => match strictness {
                    SafetyStrictness::Paranoid => SafetyDecision::block(
                        id,
                        RiskLevel::Destructive,
                        "destructive command blocked in paranoid mode",
                    ),
                    _ => SafetyDecision::ask(
                        id,
                        RiskLevel::Destructive,
                        "destructive command requires confirmation",
                        ConfirmationKind::DangerAck {
                            warning: format!(
                                "This command may be irreversible: {}",
                                truncate(command, 80)
                            ),
                        },
                    ),
                },
                CommandRisk::Blocked => SafetyDecision::ask(
                    id,
                    RiskLevel::Unknown,
                    "unrecognized command requires confirmation",
                    ConfirmationKind::Simple,
                ),
            }
        }

        // ---- Git safe reads ----------------------------------------------
        SafetyActionKind::GitRead => SafetyDecision::allow(id, RiskLevel::Safe, "local git read"),

        // ---- Git commit -------------------------------------------------
        SafetyActionKind::GitCommit { .. } => maybe_ask(
            id,
            RiskLevel::Medium,
            "git commit",
            strictness,
            SafetyStrictness::Strict,
        ),

        // ---- Git checkout -----------------------------------------------
        SafetyActionKind::GitCheckout { .. } => maybe_ask(
            id,
            RiskLevel::Medium,
            "git checkout",
            strictness,
            SafetyStrictness::Strict,
        ),

        // ---- Git branch delete ------------------------------------------
        SafetyActionKind::GitBranchDelete { branch } => match strictness {
            SafetyStrictness::Paranoid => SafetyDecision::block(
                id,
                RiskLevel::High,
                "branch delete blocked in paranoid mode",
            ),
            _ => SafetyDecision::ask(
                id,
                RiskLevel::High,
                format!("deleting branch '{branch}' is hard to reverse"),
                ConfirmationKind::Typed,
            ),
        },

        // ---- Git destructive --------------------------------------------
        SafetyActionKind::GitDestructive { summary } => match strictness {
            SafetyStrictness::Paranoid | SafetyStrictness::Strict => SafetyDecision::block(
                id,
                RiskLevel::Destructive,
                format!("destructive git action blocked: {}", truncate(summary, 120)),
            ),
            _ => SafetyDecision::ask(
                id,
                RiskLevel::Destructive,
                format!("destructive git action: {}", truncate(summary, 120)),
                ConfirmationKind::DangerAck {
                    warning: "This git operation may be irreversible.".to_string(),
                },
            ),
        },

        // ---- Git remote (always ask) ------------------------------------
        SafetyActionKind::GitRemote { summary } => SafetyDecision::ask(
            id,
            RiskLevel::Remote,
            format!("remote git operation: {}", truncate(summary, 120)),
            ConfirmationKind::Simple,
        ),

        // ---- AI context boundary ----------------------------------------
        SafetyActionKind::AiContextInclude {
            has_secret_path,
            path_count,
        } => {
            if *has_secret_path {
                return SafetyDecision::block(
                    id,
                    RiskLevel::Secret,
                    "AI context includes a secret path — blocked to prevent accidental exposure",
                )
                .with_secret_path();
            }
            match strictness {
                SafetyStrictness::Paranoid => SafetyDecision::ask(
                    id,
                    RiskLevel::Medium,
                    format!("sending {path_count} file(s) to AI provider"),
                    ConfirmationKind::Simple,
                ),
                _ => SafetyDecision::allow(
                    id,
                    RiskLevel::Low,
                    format!("sending {path_count} non-secret file(s) to AI provider"),
                ),
            }
        }

        SafetyActionKind::AiResponseStore => {
            SafetyDecision::allow(id, RiskLevel::Safe, "storing AI response locally")
        }

        // ---- Extension permission ----------------------------------------
        SafetyActionKind::ExtensionPermission { permission, .. } => {
            // Mirrors Wave 6 default permission matrix (task 6.2).
            match permission.as_str() {
                "read_workspace" => {
                    SafetyDecision::allow(id, RiskLevel::Low, "read_workspace: allowed")
                }
                "write_file" => SafetyDecision::ask(
                    id,
                    RiskLevel::Medium,
                    "extension write_file permission",
                    ConfirmationKind::Simple,
                ),
                "delete_file" => SafetyDecision::block(
                    id,
                    RiskLevel::Destructive,
                    "extension delete_file: blocked",
                ),
                "run_terminal" => SafetyDecision::ask(
                    id,
                    RiskLevel::High,
                    "extension run_terminal permission",
                    ConfirmationKind::Simple,
                ),
                "access_git" => SafetyDecision::ask(
                    id,
                    RiskLevel::Medium,
                    "extension access_git permission",
                    ConfirmationKind::Simple,
                ),
                "access_env" => {
                    SafetyDecision::block(id, RiskLevel::Secret, "extension access_env: blocked")
                }
                "access_database" => {
                    SafetyDecision::block(id, RiskLevel::High, "extension access_database: blocked")
                }
                _ => SafetyDecision::block(
                    id,
                    RiskLevel::Unknown,
                    "unknown extension permission: blocked by default",
                ),
            }
        }

        // ---- Remote / export (always ask) --------------------------------
        SafetyActionKind::RemoteExport {
            destination_summary,
        } => SafetyDecision::ask(
            id,
            RiskLevel::Remote,
            format!(
                "exporting data remotely to: {}",
                truncate(destination_summary, 80)
            ),
            ConfirmationKind::DangerAck {
                warning: "Data will leave your machine.".to_string(),
            },
        ),

        // ---- Unknown (fail conservative) ---------------------------------
        SafetyActionKind::Unknown { summary } => match strictness {
            SafetyStrictness::Paranoid => SafetyDecision::block(
                id,
                RiskLevel::Unknown,
                format!(
                    "unknown action blocked in paranoid mode: {}",
                    truncate(summary, 80)
                ),
            ),
            _ => SafetyDecision::ask(
                id,
                RiskLevel::Unknown,
                format!(
                    "unrecognized action requires confirmation: {}",
                    truncate(summary, 80)
                ),
                ConfirmationKind::Simple,
            ),
        },
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn secret_block(action_id: &str, path: &str) -> SafetyDecision {
    SafetyDecision::block(
        action_id,
        RiskLevel::Secret,
        format!("path matches a secret pattern: {}", truncate(path, 80)),
    )
    .with_secret_path()
    .with_protected_path()
}

fn protected_ask(
    action_id: &str,
    path: &str,
    risk: RiskLevel,
    strictness: SafetyStrictness,
) -> SafetyDecision {
    let d = match strictness {
        SafetyStrictness::Paranoid => SafetyDecision::block(
            action_id,
            risk,
            format!(
                "protected path blocked in paranoid mode: {}",
                truncate(path, 80)
            ),
        ),
        _ => SafetyDecision::ask(
            action_id,
            risk,
            format!("path is protected: {}", truncate(path, 80)),
            ConfirmationKind::Simple,
        ),
    };
    d.with_protected_path()
}

/// Allow under `Standard`, ask under `Strict`, block under `Paranoid`.
/// `ask_threshold`: the minimum strictness at which we start asking (not allow).
fn maybe_ask(
    action_id: &str,
    risk: RiskLevel,
    reason: &str,
    strictness: SafetyStrictness,
    ask_threshold: SafetyStrictness,
) -> SafetyDecision {
    // Strictness ordinal: Standard < Strict < Paranoid.
    let ord = |s: SafetyStrictness| match s {
        SafetyStrictness::Standard => 0u8,
        SafetyStrictness::Strict => 1,
        SafetyStrictness::Paranoid => 2,
    };

    if ord(strictness) >= ord(ask_threshold) {
        if strictness == SafetyStrictness::Paranoid {
            SafetyDecision::block(
                action_id,
                risk,
                format!("{reason}: blocked in paranoid mode"),
            )
        } else {
            SafetyDecision::ask(
                action_id,
                risk,
                format!("{reason}: confirmation required"),
                ConfirmationKind::Simple,
            )
        }
    } else {
        SafetyDecision::allow(action_id, risk, reason)
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety::action::{Actor, SafetyAction, SafetyActionKind};
    use crate::safety::decision::SafetyVerdict;

    fn registry() -> ProtectedPathRegistry {
        ProtectedPathRegistry::defaults()
    }

    fn eval(kind: SafetyActionKind, strictness: SafetyStrictness) -> SafetyDecision {
        let action = SafetyAction::new(Actor::Agent, kind, "/workspace");
        evaluate(&action, &registry(), strictness)
    }

    #[test]
    fn secret_path_read_is_blocked() {
        let d = eval(
            SafetyActionKind::FileRead {
                path: ".env".into(),
            },
            SafetyStrictness::Standard,
        );
        assert_eq!(d.verdict, SafetyVerdict::Block);
        assert_eq!(d.risk, RiskLevel::Secret);
        assert!(d.secret_path_matched);
    }

    #[test]
    fn git_internals_read_asks_by_default() {
        let d = eval(
            SafetyActionKind::FileRead {
                path: ".git/config".into(),
            },
            SafetyStrictness::Standard,
        );
        assert_eq!(d.verdict, SafetyVerdict::Ask);
        assert!(d.protected_path_matched);
    }

    #[test]
    fn unknown_actions_fail_conservative() {
        let d = eval(
            SafetyActionKind::Unknown {
                summary: "frobnicate".into(),
            },
            SafetyStrictness::Standard,
        );
        assert!(matches!(
            d.verdict,
            SafetyVerdict::Ask | SafetyVerdict::Block
        ));
    }

    #[test]
    fn paranoid_blocks_unknown_actions() {
        let d = eval(
            SafetyActionKind::Unknown {
                summary: "frobnicate".into(),
            },
            SafetyStrictness::Paranoid,
        );
        assert_eq!(d.verdict, SafetyVerdict::Block);
    }

    #[test]
    fn remote_export_always_asks() {
        let d = eval(
            SafetyActionKind::RemoteExport {
                destination_summary: "cloud".into(),
            },
            SafetyStrictness::Standard,
        );
        assert_eq!(d.verdict, SafetyVerdict::Ask);
        assert_eq!(d.risk, RiskLevel::Remote);
    }

    #[test]
    fn destructive_terminal_command_blocks_in_paranoid() {
        let d = eval(
            SafetyActionKind::TerminalCommand {
                command: "rm -rf .".into(),
            },
            SafetyStrictness::Paranoid,
        );

        assert_eq!(d.verdict, SafetyVerdict::Block);
        assert_eq!(d.risk, RiskLevel::Destructive);
    }

    #[test]
    fn ai_context_with_secret_is_blocked() {
        let d = eval(
            SafetyActionKind::AiContextInclude {
                path_count: 3,
                has_secret_path: true,
            },
            SafetyStrictness::Standard,
        );
        assert_eq!(d.verdict, SafetyVerdict::Block);
        assert!(d.secret_path_matched);
    }

    #[test]
    fn extension_unknown_permission_is_blocked() {
        let d = eval(
            SafetyActionKind::ExtensionPermission {
                extension_id: "foo".into(),
                permission: "hack_the_planet".into(),
            },
            SafetyStrictness::Standard,
        );
        assert_eq!(d.verdict, SafetyVerdict::Block);
    }

    #[test]
    fn extension_read_workspace_is_allowed() {
        let d = eval(
            SafetyActionKind::ExtensionPermission {
                extension_id: "foo".into(),
                permission: "read_workspace".into(),
            },
            SafetyStrictness::Standard,
        );
        assert_eq!(d.verdict, SafetyVerdict::Allow);
    }

    #[test]
    fn strictness_changes_decisions_predictably() {
        // A normal file write: Standard→allow, Strict→ask, Paranoid→block.
        let std = eval(
            SafetyActionKind::FileWrite {
                path: "src/main.rs".into(),
            },
            SafetyStrictness::Standard,
        );
        let strict = eval(
            SafetyActionKind::FileWrite {
                path: "src/main.rs".into(),
            },
            SafetyStrictness::Strict,
        );
        let paranoid = eval(
            SafetyActionKind::FileWrite {
                path: "src/main.rs".into(),
            },
            SafetyStrictness::Paranoid,
        );
        assert_eq!(std.verdict, SafetyVerdict::Allow);
        assert_eq!(strict.verdict, SafetyVerdict::Ask);
        assert_eq!(paranoid.verdict, SafetyVerdict::Block);
    }

    #[test]
    fn git_read_is_always_allowed() {
        for s in [
            SafetyStrictness::Standard,
            SafetyStrictness::Strict,
            SafetyStrictness::Paranoid,
        ] {
            let d = eval(SafetyActionKind::GitRead, s);
            assert_eq!(
                d.verdict,
                SafetyVerdict::Allow,
                "git_read must always be allowed"
            );
        }
    }
}
