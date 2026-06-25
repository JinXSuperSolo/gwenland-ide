//! Agentic coding workflow (Milestone 10).
//!
//! A human-gated plan -> approve -> edit -> validate -> summarize loop. This
//! module is the engine half: a pure state machine, safety policy, context
//! filtering, prompt construction, change-set parsing, validation classification,
//! and summary model. It has ZERO Tauri/UI imports — the Tauri command bridge,
//! managed `AgentManager`, provider streaming, and command execution live in
//! `frontend/src/`, and the Svelte UI in `frontend/ui/src/lib/`.
//!
//! Strict invariants enforced here:
//! - edits cannot be drafted before the plan is approved,
//! - edits cannot be applied before a hunk/file is approved,
//! - validation cannot run before a command is approved,
//! - secret-looking files are excluded from context by default,
//! - all paths are workspace-scoped and canonicalized,
//! - no provider keys ever live in session state.

pub mod agent_loop;
pub mod changeset;
pub mod context;
pub mod memory;
pub mod persistence;
pub mod plan_parse;
pub mod policy;
pub mod prompts;
pub mod session;
pub mod summary;
pub mod tier;
pub mod tools;
pub mod validation;

pub use agent_loop::{AgentLoop, DEFAULT_MAX_ITERATIONS, LoopTurn, parse_tool_call};
pub use memory::{
    MemoryBudget, MemoryError, MemoryNote, MemorySearchResult, MemoryWriteTarget,
    memory_conversation_dir, memory_project_dir, parse_keyword_array, parse_memory_note,
    project_name_from_root, render_memory_block, sanitize_note_filename, sanitize_segment,
    search_memory, write_memory_note,
};
pub use changeset::{
    ApplyOutcome, ApplyReport, ApprovalState, ChangeSet, FileChangeKind, ProposedFileChange,
    ProposedHunk, apply_approved_hunks_to_text, change_set_from_diff_files, change_set_from_text,
};
pub use context::{
    ContextItem, ContextItemKind, ContextOmission, ContextPreview, MAX_CONTEXT_ITEMS,
    MAX_ITEM_BYTES, MAX_TOTAL_CONTEXT_BYTES, OmissionReason, omission_for_path,
};
pub use persistence::{load_sessions, persist_session, restored_session, session_for_persistence};
pub use plan_parse::parse_plan;
pub use policy::{
    PolicyError, canonical_within_workspace, classify_command, is_excluded_path, is_secret_path,
    is_within_workspace, redact_secrets,
};
pub use prompts::{
    AGENT_TOOL_SYSTEM, EDIT_SYSTEM, PLAN_SYSTEM, SUMMARY_SYSTEM, build_edit_user_prompt,
    build_plan_user_prompt, build_summary_user_prompt,
};
pub use session::{
    AgentPhase, AgentPlan, AgentSession, ApprovalKind, ApprovalRecord, PhaseError, PlanStep,
    PlanStepStatus,
};
pub use summary::{AgentSummary, build_local_summary, validation_status_line};
pub use tier::{
    ActionConfidence, AgentTier, command_confidence, mutation_confidence, requires_user_approval,
};
pub use tools::{
    ALL_TOOLS, MutationPreflight, PathResolution, ToolCall, ToolKind, ToolResult, ToolSide,
    ToolSpec, execute_local_tool, preflight_mutation_path, render_tool_specs,
    resolve_workspace_file,
};
pub use validation::{CommandRisk, ValidationCommand, ValidationRun, ValidationStatus};

/// Shared RFC-3339 UTC timestamp used across agentic DTOs. Mirrors the helper in
/// `crate::ai::conversation` (kept private there) so timestamps are consistent.
pub(crate) fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

/// Fresh random id (v4 UUID) for sessions, plans, change sets, etc. Exposed so
/// the Tauri layer can mint ids without depending on `uuid` itself.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
