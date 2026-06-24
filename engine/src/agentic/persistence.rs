//! Lightweight session snapshot persistence for M10.
//!
//! This stays in the engine because it is pure data storage: no Tauri handles,
//! no UI state, no provider keys, and no runtime stream handles. Context item
//! contents are stripped before writing so restored sessions keep review state
//! without persisting source snippets or secret-looking text.

use std::path::{Path, PathBuf};

use crate::agentic::session::{AgentPhase, AgentSession};

fn sessions_dir() -> Result<PathBuf, String> {
    let dir = crate::app_data::get_app_data_dir()
        .map_err(|e| e.to_string())?
        .join("agent_sessions");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn session_path(session_id: &str) -> Result<PathBuf, String> {
    if session_id.is_empty()
        || session_id
            .chars()
            .any(|c| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
    {
        return Err("invalid agent session id".to_string());
    }
    Ok(sessions_dir()?.join(format!("{session_id}.json")))
}

pub fn session_for_persistence(session: &AgentSession) -> AgentSession {
    let mut snapshot = session.clone();
    for item in &mut snapshot.context.items {
        item.content = None;
    }
    snapshot
}

pub fn persist_session(session: &AgentSession) -> Result<(), String> {
    let snapshot = session_for_persistence(session);
    let json = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
    std::fs::write(session_path(&snapshot.id)?, json).map_err(|e| e.to_string())
}

pub fn restored_session(mut session: AgentSession) -> AgentSession {
    let interrupted_from = match session.phase {
        AgentPhase::DraftingPlan => Some(AgentPhase::Goal),
        AgentPhase::DraftingEdits => Some(AgentPhase::AwaitingPlanApproval),
        AgentPhase::ApplyingApprovedEdits => {
            if session.apply_report.is_some() {
                Some(AgentPhase::AwaitingValidationApproval)
            } else {
                Some(AgentPhase::AwaitingEditApproval)
            }
        }
        AgentPhase::Validating => Some(AgentPhase::AwaitingValidationApproval),
        AgentPhase::Summarizing => {
            if session.summary.is_some() {
                Some(AgentPhase::Complete)
            } else {
                Some(AgentPhase::AwaitingValidationApproval)
            }
        }
        _ => None,
    };
    if let Some(phase) = interrupted_from {
        session.phase = phase;
        session.interrupted = true;
    }
    for item in &mut session.context.items {
        item.content = None;
    }
    session
}

pub fn load_sessions(project_root: Option<&Path>) -> Result<Vec<AgentSession>, String> {
    let dir = sessions_dir()?;
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        files.push((modified, entry.path()));
    }
    files.sort_by_key(|b| std::cmp::Reverse(b.0));

    let root_filter = project_root.and_then(|root| root.canonicalize().ok());
    let mut sessions = Vec::new();
    for (_, path) in files {
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => continue,
        };
        let session = match serde_json::from_str::<AgentSession>(&text) {
            Ok(session) => restored_session(session),
            Err(_) => continue,
        };
        if let Some(root) = &root_filter {
            let Ok(session_root) = Path::new(&session.project_root).canonicalize() else {
                continue;
            };
            if &session_root != root {
                continue;
            }
        }
        sessions.push(session);
    }
    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::context::{ContextItem, ContextItemKind, ContextPreview};

    #[test]
    fn persistent_snapshot_strips_context_contents() {
        let mut preview = ContextPreview::new();
        preview.items.push(ContextItem::included(
            "ctx-1",
            ContextItemKind::ActiveFile,
            Some("src/main.rs".into()),
            "src/main.rs",
            Some("file body that should not be persisted".into()),
            "active editor",
        ));
        let session = AgentSession::new("session-1", "/project", "do it", "openai", "gpt", preview);

        let persisted = session_for_persistence(&session);

        assert_eq!(
            session.context.items[0].content.as_deref(),
            Some("file body that should not be persisted")
        );
        assert_eq!(persisted.context.items[0].content, None);
    }

    #[test]
    fn restored_streaming_session_is_interrupted_at_safe_phase() {
        let mut session = AgentSession::new(
            "session-1",
            "/project",
            "do it",
            "openai",
            "gpt",
            ContextPreview::new(),
        );
        session.phase = AgentPhase::DraftingEdits;

        let restored = restored_session(session);

        assert!(restored.interrupted);
        assert_eq!(restored.phase, AgentPhase::AwaitingPlanApproval);
    }
}
