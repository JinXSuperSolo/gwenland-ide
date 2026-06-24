//! Tolerant plan-text normalization (M10, Requirement 2 / Wave 2 task 2.4).
//!
//! The planning prompt asks the provider for labeled sections (Title,
//! Assumptions, Steps, Likely files, Risks, Missing context, Suggested
//! validation). Models are not perfectly structured, so this parser is lenient:
//! it recognizes markdown headings (`## Steps`), bold labels (`**Steps**`), and
//! `Label:` lines, and gathers bullet/numbered items beneath each. Anything it
//! cannot place is ignored rather than fatal. Pure engine code, fully testable.

use crate::agentic::new_id;
use crate::agentic::session::{AgentPlan, PlanStep, PlanStepStatus};
use crate::agentic::validation::ValidationCommand;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    None,
    Assumptions,
    Steps,
    Files,
    Risks,
    Missing,
    Validation,
}

/// Parse provider plan text into a structured [`AgentPlan`].
pub fn parse_plan(plan_id: &str, text: &str) -> AgentPlan {
    let mut title = String::new();
    let mut assumptions = Vec::new();
    let mut steps_raw = Vec::new();
    let mut likely_files = Vec::new();
    let mut risks = Vec::new();
    let mut missing = Vec::new();
    let mut validation_raw = Vec::new();
    let mut section = Section::None;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || is_fence(line) {
            continue;
        }

        if let Some((canon, inline)) = detect_header(line) {
            match canon {
                Section::None => {
                    // A "Title:" header — its value is on the same line.
                    if let Some(v) = inline {
                        if title.is_empty() {
                            title = v;
                        }
                    }
                }
                other => {
                    section = other;
                    if let Some(v) = inline {
                        push_into(
                            other,
                            clean_bullet(&v),
                            &mut assumptions,
                            &mut steps_raw,
                            &mut likely_files,
                            &mut risks,
                            &mut missing,
                            &mut validation_raw,
                        );
                    }
                }
            }
            continue;
        }

        let item = clean_bullet(line);
        if item.is_empty() {
            continue;
        }
        match section {
            Section::None => {
                if title.is_empty() {
                    title = item;
                }
            }
            other => push_into(
                other,
                item,
                &mut assumptions,
                &mut steps_raw,
                &mut likely_files,
                &mut risks,
                &mut missing,
                &mut validation_raw,
            ),
        }
    }

    if title.is_empty() {
        title = "Implementation plan".to_string();
    }

    let steps = steps_raw
        .into_iter()
        .enumerate()
        .map(|(i, s)| PlanStep {
            id: format!("step-{}", i + 1),
            label: short_label(&s),
            description: s,
            status: PlanStepStatus::Pending,
        })
        .collect();

    let suggested_validation = validation_raw
        .into_iter()
        .filter(|c| !is_none_word(c))
        .map(|c| {
            let cmd = strip_code_ticks(&c);
            ValidationCommand::new(new_id(), cmd, ".", "suggested by plan")
        })
        .collect();

    let missing_context = missing.into_iter().filter(|m| !is_none_word(m)).collect();

    AgentPlan {
        id: plan_id.to_string(),
        title,
        assumptions,
        steps,
        likely_files,
        risks,
        suggested_validation,
        missing_context,
    }
}

#[allow(clippy::too_many_arguments)]
fn push_into(
    section: Section,
    item: String,
    assumptions: &mut Vec<String>,
    steps: &mut Vec<String>,
    files: &mut Vec<String>,
    risks: &mut Vec<String>,
    missing: &mut Vec<String>,
    validation: &mut Vec<String>,
) {
    if item.is_empty() {
        return;
    }
    match section {
        Section::Assumptions => assumptions.push(item),
        Section::Steps => steps.push(item),
        Section::Files => files.push(item),
        Section::Risks => risks.push(item),
        Section::Missing => missing.push(item),
        Section::Validation => validation.push(item),
        Section::None => {}
    }
}

/// A line is a section header if, after stripping markdown markers, it begins
/// with a known label and looks like a heading (markdown `#`, bold, a trailing
/// `:`, or just the short label alone). Returns the canonical section plus any
/// inline value after a `:`.
fn detect_header(line: &str) -> Option<(Section, Option<String>)> {
    let bare = line.trim_start_matches('#').trim().trim_matches('*').trim();
    let lower = bare.to_ascii_lowercase();

    let (canon, keyword): (Section, &str) = if lower.starts_with("title") {
        (Section::None, "title")
    } else if lower.starts_with("assumption") {
        (Section::Assumptions, "assumption")
    } else if lower.starts_with("step") {
        (Section::Steps, "step")
    } else if lower.starts_with("likely file") || lower.starts_with("file") {
        (Section::Files, "file")
    } else if lower.starts_with("risk") {
        (Section::Risks, "risk")
    } else if lower.starts_with("missing") {
        (Section::Missing, "missing")
    } else if lower.starts_with("suggested validation")
        || lower.starts_with("validation")
        || lower.starts_with("suggested command")
    {
        (Section::Validation, "validation")
    } else {
        return None;
    };

    // Distinguish a real header from a content line that merely starts with the
    // keyword (e.g. "Step 1: edit file" is a step, not a header). Accept it as a
    // header when it is a markdown heading, ends after the label (optionally with
    // a colon), or has a colon close to the start.
    let is_markdown_heading =
        line.trim_start().starts_with('#') || line.trim_start().starts_with('*');
    let colon = bare.find(':');
    let label_only = lower.trim_end_matches(':').trim() == keyword
        || lower.trim_end_matches('s').trim_end_matches(':').trim() == keyword;

    let accept = is_markdown_heading
        || label_only
        || colon.map(|c| c <= keyword.len() + 12).unwrap_or(false);
    if !accept {
        return None;
    }

    let inline = colon.and_then(|c| {
        let v = bare[c + 1..].trim();
        if v.is_empty() {
            None
        } else {
            Some(v.to_string())
        }
    });
    Some((canon, inline))
}

fn is_fence(line: &str) -> bool {
    line.starts_with("```")
}

/// Strip a leading bullet/number marker and surrounding markdown emphasis.
fn clean_bullet(line: &str) -> String {
    let mut s = line.trim();
    // Leading list markers: -, *, +, •, or "1." / "1)".
    if let Some(rest) = s
        .strip_prefix("- ")
        .or_else(|| s.strip_prefix("* "))
        .or_else(|| s.strip_prefix("+ "))
        .or_else(|| s.strip_prefix("• "))
    {
        s = rest.trim();
    } else {
        // numbered: digits followed by . or )
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i > 0 && i < bytes.len() && (bytes[i] == b'.' || bytes[i] == b')') {
            s = s[i + 1..].trim_start();
        }
    }
    s.trim_matches('*').trim().to_string()
}

fn short_label(s: &str) -> String {
    let words: Vec<&str> = s.split_whitespace().take(6).collect();
    let mut label = words.join(" ");
    if label.len() > 60 {
        label.truncate(57);
        label.push_str("...");
    }
    label
}

fn is_none_word(s: &str) -> bool {
    let t = s.trim().trim_end_matches('.').to_ascii_lowercase();
    t == "none" || t == "n/a" || t == "na" || t.is_empty()
}

fn strip_code_ticks(s: &str) -> String {
    s.trim().trim_matches('`').trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
## Title: Add a dark mode toggle

## Assumptions
- The app already has a theme store
- CSS variables drive colors

## Steps
1. Add a `theme` field to the settings store
2. Render a toggle button in the status bar
3. Persist the choice

## Likely files
- frontend/ui/src/lib/stores/settings.ts
- frontend/ui/src/lib/components/StatusBar.svelte

## Risks
- Flash of unstyled content on load

## Missing context
none

## Suggested validation
- `pnpm check`
- `pnpm build`
";

    #[test]
    fn parses_all_sections() {
        let plan = parse_plan("plan-1", SAMPLE);
        assert_eq!(plan.id, "plan-1");
        assert_eq!(plan.title, "Add a dark mode toggle");
        assert_eq!(plan.assumptions.len(), 2);
        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.steps[0].status, PlanStepStatus::Pending);
        assert!(plan.steps[0].label.starts_with("Add a"));
        assert_eq!(plan.likely_files.len(), 2);
        assert_eq!(plan.risks.len(), 1);
        assert!(plan.missing_context.is_empty(), "'none' should be dropped");
        assert_eq!(plan.suggested_validation.len(), 2);
        assert_eq!(plan.suggested_validation[0].command, "pnpm check");
    }

    #[test]
    fn step_content_lines_are_not_treated_as_headers() {
        // "Step 1: ..." style lines under a Steps header are steps, not headers.
        let text = "Steps:\nStep 1: do the first thing\nStep 2: do the second thing\n";
        let plan = parse_plan("p", text);
        assert_eq!(plan.steps.len(), 2);
    }

    #[test]
    fn falls_back_to_default_title_and_empty_sections() {
        let plan = parse_plan(
            "p",
            "Just some prose with no structure at all that is one line",
        );
        assert!(!plan.title.is_empty());
        assert!(plan.steps.is_empty());
        assert!(plan.suggested_validation.is_empty());
    }

    #[test]
    fn validation_commands_are_classified() {
        let text = "Suggested validation:\n- rm -rf dist\n- cargo test\n";
        let plan = parse_plan("p", text);
        assert_eq!(plan.suggested_validation.len(), 2);
        use crate::agentic::validation::CommandRisk;
        assert_eq!(plan.suggested_validation[0].risk, CommandRisk::Destructive);
        assert_eq!(plan.suggested_validation[1].risk, CommandRisk::SafeCheck);
    }
}
