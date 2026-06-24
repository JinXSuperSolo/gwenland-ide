//! ReAct (reason -> act -> observe) loop driver (M10 Wave 7).
//!
//! Provider-neutral: the engine owns the transcript, the tool-call protocol
//! parser, the next-request message assembly, and the stop logic. It does NOT
//! call providers or execute tools — the Tauri layer streams the provider,
//! dispatches each parsed `ToolCall` (read tools via `tools::execute_local_tool`,
//! the rest Tauri-side through the Apply/Validation gates), feeds the
//! `ToolResult` back, and iterates until the loop finishes or is exhausted.
//!
//! Tool calls travel as a text protocol so every BYO provider works without
//! native function-calling: the model emits one fenced ```tool block of JSON
//! (`{"tool": ..., "args": {...}}`); no block means a final answer.

use serde_json::Value;

use crate::agentic::tools::{ToolCall, ToolKind, ToolResult, render_tool_specs};
use crate::ai::ChatMessage;

/// Safety cap on loop iterations so a confused model can't spin forever.
pub const DEFAULT_MAX_ITERATIONS: usize = 16;

/// One entry in the loop transcript.
#[derive(Debug, Clone)]
pub enum LoopTurn {
    /// The model's reply for a step, with the parsed tool call (None = finish).
    Assistant { text: String, tool_call: Option<ToolCall> },
    /// The observation produced by executing the preceding tool call.
    Tool { result: ToolResult },
}

/// The running state of one agent tool loop.
pub struct AgentLoop {
    pub goal: String,
    pub turns: Vec<LoopTurn>,
    pub iteration: usize,
    pub max_iterations: usize,
}

impl AgentLoop {
    pub fn new(goal: impl Into<String>) -> Self {
        Self {
            goal: goal.into(),
            turns: Vec::new(),
            iteration: 0,
            max_iterations: DEFAULT_MAX_ITERATIONS,
        }
    }

    /// Record the model's reply for a step and return the parsed tool call.
    /// `None` means the model produced a final answer — the orchestrator stops.
    pub fn record_assistant(&mut self, text: &str) -> Option<ToolCall> {
        let call = parse_tool_call(text);
        self.turns.push(LoopTurn::Assistant {
            text: text.to_string(),
            tool_call: call.clone(),
        });
        self.iteration += 1;
        call
    }

    /// Record a tool's observation, to be fed back on the next request.
    pub fn record_tool_result(&mut self, result: ToolResult) {
        self.turns.push(LoopTurn::Tool { result });
    }

    /// True once the iteration cap is reached (orchestrator must stop + summarize).
    pub fn is_exhausted(&self) -> bool {
        self.iteration >= self.max_iterations
    }

    /// Build the provider message list for the next request from the transcript.
    /// The system prompt (`prompts::AGENT_TOOL_SYSTEM`) is supplied separately by
    /// the Tauri layer via `MessageRequest.system`.
    pub fn build_messages(&self, context_summary: &str) -> Vec<ChatMessage> {
        let mut messages = vec![ChatMessage::user(self.initial_prompt(context_summary))];
        let mut last_tool: Option<&'static str> = None;
        for turn in &self.turns {
            match turn {
                LoopTurn::Assistant { text, tool_call } => {
                    messages.push(ChatMessage::assistant(text.clone()));
                    last_tool = tool_call.as_ref().map(|c| c.tool.name());
                }
                LoopTurn::Tool { result } => {
                    messages.push(ChatMessage::user(format_observation(last_tool, result)));
                    last_tool = None;
                }
            }
        }
        messages
    }

    fn initial_prompt(&self, context_summary: &str) -> String {
        let ctx = if context_summary.trim().is_empty() {
            "(none)".to_string()
        } else {
            context_summary.trim().to_string()
        };
        format!(
            "Goal:\n{}\n\nContext:\n{}\n\nAvailable tools:\n{}\nWork the act-observe loop: call ONE tool, or finish with a short summary when done.",
            self.goal,
            ctx,
            render_tool_specs(),
        )
    }
}

/// Render a tool observation as a user turn for the model to read.
fn format_observation(tool: Option<&str>, result: &ToolResult) -> String {
    let label = tool.unwrap_or("tool");
    if result.ok {
        format!("Observation — {label} ok:\n{}", result.content)
    } else {
        format!(
            "Observation — {label} error:\n{}",
            result.error.clone().unwrap_or_default()
        )
    }
}

// --- Tool-call protocol parsing --------------------------------------------

/// Parse a single tool call from assistant text. Tries fenced code blocks first
/// (any ```lang), then the whole text as bare JSON. Returns `None` when there is
/// no valid tool call (i.e. the model gave a final answer).
pub fn parse_tool_call(text: &str) -> Option<ToolCall> {
    for block in fenced_blocks(text) {
        if let Some(call) = parse_tool_json(&block) {
            return Some(call);
        }
    }
    parse_tool_json(text.trim())
}

fn parse_tool_json(s: &str) -> Option<ToolCall> {
    let value: Value = serde_json::from_str(s.trim()).ok()?;
    let name = value.get("tool").and_then(|v| v.as_str())?;
    let tool = ToolKind::from_name(name)?;
    let args = value
        .get("args")
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));
    Some(ToolCall {
        id: crate::agentic::new_id(),
        tool,
        args,
    })
}

/// Extract the bodies of all ```...``` fenced blocks, dropping the language tag.
fn fenced_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("```") {
        let after = &rest[start + 3..];
        let body_start = after.find('\n').map(|n| n + 1).unwrap_or(after.len());
        let body = &after[body_start..];
        match body.find("```") {
            Some(end) => {
                blocks.push(body[..end].to_string());
                rest = &body[end + 3..];
            }
            None => break,
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fenced_tool_call() {
        let text = "I'll read it.\n```tool\n{\"tool\": \"read_file\", \"args\": {\"path\": \"a.rs\"}}\n```";
        let call = parse_tool_call(text).expect("a tool call");
        assert_eq!(call.tool, ToolKind::ReadFile);
        assert_eq!(call.args.get("path").unwrap().as_str(), Some("a.rs"));
    }

    #[test]
    fn parses_bare_json_tool_call() {
        let call = parse_tool_call(r#"{"tool":"list_dir","args":{"path":"src"}}"#).unwrap();
        assert_eq!(call.tool, ToolKind::ListDir);
    }

    #[test]
    fn prose_is_not_a_tool_call() {
        assert!(parse_tool_call("All done — I updated the file and tests pass.").is_none());
        // Unknown tool name is ignored.
        assert!(parse_tool_call("```tool\n{\"tool\":\"nuke\"}\n```").is_none());
    }

    #[test]
    fn record_assistant_tracks_iteration_and_call() {
        let mut lp = AgentLoop::new("do it");
        let call = lp.record_assistant("```tool\n{\"tool\":\"read_file\",\"args\":{\"path\":\"a\"}}\n```");
        assert!(call.is_some());
        assert_eq!(lp.iteration, 1);
        assert!(!lp.is_exhausted());

        lp.max_iterations = 1;
        assert!(lp.is_exhausted());
    }

    #[test]
    fn build_messages_alternates_user_assistant_observation() {
        let mut lp = AgentLoop::new("goal");
        let call = lp
            .record_assistant("```tool\n{\"tool\":\"read_file\",\"args\":{\"path\":\"a\"}}\n```")
            .unwrap();
        lp.record_tool_result(ToolResult::ok(&call.id, "file body"));
        let msgs = lp.build_messages("active file: a");
        assert_eq!(msgs.len(), 3); // initial user, assistant, observation
        assert_eq!(msgs[0].role, "user");
        assert!(msgs[0].content.contains("goal"));
        assert!(msgs[0].content.contains("read_file")); // tool specs listed
        assert_eq!(msgs[1].role, "assistant");
        assert_eq!(msgs[2].role, "user");
        assert!(msgs[2].content.contains("read_file ok"));
        assert!(msgs[2].content.contains("file body"));
    }
}
