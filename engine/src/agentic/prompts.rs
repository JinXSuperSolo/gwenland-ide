//! Prompt builders for the agent loop (M10, design "Prompt Contracts").
//!
//! Pure string construction — concise, approval-gated, provider-neutral. The
//! planning prompt forbids diffs; the edit prompt requires unified diffs and
//! forbids claiming edits were applied; the summary prompt forbids leaking
//! hidden/secret context. No Tauri/UI here.

use crate::agentic::session::AgentPlan;

/// System prompt for the planning phase. The model must produce a plan only.
pub const PLAN_SYSTEM: &str = "\
You are GwenLand's coding agent operating in a strict, human-gated workflow.
This is the PLANNING phase. Produce a concise implementation PLAN ONLY.

Hard rules:
- DO NOT output code edits or unified diffs in this phase.
- DO NOT claim you have changed any files. You have not.
- The user must approve your plan before you are allowed to propose edits.

Structure your plan with these clearly labeled sections:
- Title: one line.
- Assumptions: bullet list of what you are assuming.
- Steps: numbered, each a short imperative action.
- Likely files: project-relative paths you expect to touch.
- Risks: anything that could go wrong or needs care.
- Missing context: anything you still need from the user (or 'none').
- Suggested validation: build/test/lint commands to verify the work.

Keep it tight. Prefer the smallest plan that achieves the goal.";

/// System prompt for the edit phase. The model must output unified diffs.
pub const EDIT_SYSTEM: &str = "\
You are GwenLand's coding agent. The user APPROVED the plan below. This is the
EDIT phase: implement ONLY the approved plan.

Hard rules:
- Output file changes as UNIFIED DIFFS only, with real project-relative paths:
  --- a/path/to/file
  +++ b/path/to/file
  @@ -line,count +line,count @@
  -removed line
  +added line
- Provide enough surrounding context lines for each hunk to apply cleanly.
- DO NOT claim the edits were applied — the user reviews and applies them.
- DO NOT perform destructive operations (delete/rename/whole-file replacement)
  unless the approved plan explicitly called for them.
- Keep changes scoped to the plan. Do not wander.
- You may add a brief note before the diffs, but the diffs are what gets applied.";

/// System prompt for the summary phase.
pub const SUMMARY_SYSTEM: &str = "\
You are GwenLand's coding agent, writing a final SUMMARY of a completed,
human-gated coding session. Summarize factually from the data provided:
the approved plan, what was applied, validation results, and any failures or
unresolved risks.

Hard rules:
- Be concise and factual. Do not invent results that are not in the data.
- Do not include secrets, API keys, or raw file contents.
- End with concrete suggested next steps (or 'none').";

/// System prompt for the Wave 7 tool-calling ReAct loop. Teaches the text-based
/// tool protocol and the safety gates. The dynamic tool list + goal/context go
/// in the user message (`agent_loop::AgentLoop::build_messages`).
pub const AGENT_TOOL_SYSTEM: &str = "\
You are GwenLand's coding agent running an act-observe (ReAct) loop with tools.

Each step, do EXACTLY ONE of:
1) Call one tool — output a single fenced block tagged `tool` with JSON, and
   nothing after it, then stop and wait for the observation:
   ```tool
   {\"tool\": \"read_file\", \"args\": {\"path\": \"src/main.rs\"}}
   ```
2) Finish — when the goal is met (or you need nothing more), output a short
   plain-text summary with NO tool block.

Hard rules:
- One tool call per step. Never emit multiple tool blocks or text after one.
- Read tools (read_file, list_dir, grep_search, file_search, get_git_diff,
  get_diagnostics) run immediately and return an observation.
- Mutating tools (edit_file, write_file, delete_file) and run_terminal_cmd are
  PROPOSALS: the user must approve them. Do not assume they succeeded until the
  observation says so. delete_file and destructive commands need explicit
  confirmation and may be rejected.
- Use real project-relative paths. Never read or write secret files such as
  .env or private keys.
- Never guess a file's path. Before edit_file or delete_file on an existing
  file, confirm the exact path first with file_search (or read_file). If an
  edit/delete observation says the path was not found, call file_search for the
  file name and retry with the correct path — do not repeat the bad path.
- Prefer the smallest set of steps. When done, finish with the summary.";

/// Build the planning user prompt from the goal and a context summary string.
pub fn build_plan_user_prompt(goal: &str, context_summary: &str) -> String {
    let context_block = if context_summary.trim().is_empty() {
        "(no additional context attached)".to_string()
    } else {
        context_summary.trim().to_string()
    };
    format!(
        "Goal:\n{goal}\n\nContext included in this request:\n{context_block}\n\n\
Produce the plan now. Remember: plan only, no diffs."
    )
}

/// Build the edit user prompt from the approved plan and a context summary.
pub fn build_edit_user_prompt(plan: &AgentPlan, context_summary: &str) -> String {
    let mut out = String::new();
    out.push_str("Approved plan:\n");
    out.push_str(&format!("Title: {}\n", plan.title));
    if !plan.steps.is_empty() {
        out.push_str("Steps:\n");
        for (i, step) in plan.steps.iter().enumerate() {
            out.push_str(&format!(
                "{}. {} — {}\n",
                i + 1,
                step.label,
                step.description
            ));
        }
    }
    if !plan.likely_files.is_empty() {
        out.push_str(&format!("Likely files: {}\n", plan.likely_files.join(", ")));
    }
    if !plan.risks.is_empty() {
        out.push_str(&format!("Risks: {}\n", plan.risks.join("; ")));
    }
    if !context_summary.trim().is_empty() {
        out.push_str("\nContext included in this request:\n");
        out.push_str(context_summary.trim());
        out.push('\n');
    }
    out.push_str("\nImplement the approved plan now as unified diffs only.");
    out
}

/// Build the summary user prompt from the structured run facts.
pub fn build_summary_user_prompt(
    goal: &str,
    plan_title: &str,
    applied: &[String],
    failed: &[String],
    validation_lines: &[String],
    unresolved_risks: &[String],
) -> String {
    let fmt_list = |items: &[String]| {
        if items.is_empty() {
            "none".to_string()
        } else {
            items
                .iter()
                .map(|i| format!("- {i}"))
                .collect::<Vec<_>>()
                .join("\n")
        }
    };
    format!(
        "Goal:\n{goal}\n\nApproved plan: {plan_title}\n\nApplied changes:\n{}\n\nFailed/skipped:\n{}\n\nValidation:\n{}\n\nUnresolved risks:\n{}\n\nWrite the final summary now.",
        fmt_list(applied),
        fmt_list(failed),
        fmt_list(validation_lines),
        fmt_list(unresolved_risks),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::session::{AgentPlan, PlanStep, PlanStepStatus};

    fn plan() -> AgentPlan {
        AgentPlan {
            id: "p".into(),
            title: "Add feature".into(),
            assumptions: vec!["builds on main".into()],
            steps: vec![PlanStep {
                id: "s1".into(),
                label: "Edit lib".into(),
                description: "add fn".into(),
                status: PlanStepStatus::Pending,
            }],
            likely_files: vec!["src/lib.rs".into()],
            risks: vec!["none".into()],
            suggested_validation: vec![],
            missing_context: vec![],
        }
    }

    #[test]
    fn plan_prompt_forbids_diffs() {
        assert!(PLAN_SYSTEM.to_lowercase().contains("plan only"));
        assert!(PLAN_SYSTEM.to_lowercase().contains("do not output"));
    }

    #[test]
    fn edit_prompt_requires_unified_diffs_and_forbids_claiming_applied() {
        assert!(EDIT_SYSTEM.to_lowercase().contains("unified diff"));
        assert!(EDIT_SYSTEM.to_lowercase().contains("do not claim"));
    }

    #[test]
    fn plan_user_prompt_includes_goal_and_context() {
        let p = build_plan_user_prompt("make it fast", "active file: src/main.rs");
        assert!(p.contains("make it fast"));
        assert!(p.contains("src/main.rs"));
    }

    #[test]
    fn plan_user_prompt_handles_empty_context() {
        let p = build_plan_user_prompt("goal", "   ");
        assert!(p.contains("no additional context"));
    }

    #[test]
    fn edit_user_prompt_includes_plan_steps() {
        let p = build_edit_user_prompt(&plan(), "");
        assert!(p.contains("Add feature"));
        assert!(p.contains("Edit lib"));
        assert!(p.to_lowercase().contains("unified diffs only"));
    }

    #[test]
    fn summary_prompt_lists_facts() {
        let p = build_summary_user_prompt(
            "goal",
            "Add feature",
            &["src/lib.rs".into()],
            &[],
            &["cargo test: passed".into()],
            &[],
        );
        assert!(p.contains("src/lib.rs"));
        assert!(p.contains("cargo test: passed"));
        assert!(p.contains("none")); // empty failed/risks render as none
    }
}
