//! `textDocument/hover` DTO and response normalization.

use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_HOVER_CHARS: usize = 2_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspHover {
    pub contents: String,
}

pub fn normalize_hover(result: &Value) -> Option<LspHover> {
    let contents = hover_contents(result.get("contents")?)?;
    let contents = contents.trim();
    if contents.is_empty() {
        return None;
    }
    Some(LspHover {
        contents: clamp(contents),
    })
}

fn hover_contents(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => {
            let parts = items
                .iter()
                .filter_map(hover_contents)
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n\n"))
            }
        }
        Value::Object(map) => map
            .get("value")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| map.get("contents").and_then(hover_contents)),
        _ => None,
    }
}

fn clamp(text: &str) -> String {
    if text.chars().count() <= MAX_HOVER_CHARS {
        text.to_string()
    } else {
        text.chars().take(MAX_HOVER_CHARS).collect::<String>() + "..."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_markup_content() {
        let result = serde_json::json!({
            "contents": { "kind": "markdown", "value": "```rust\nfn main()\n```" }
        });
        let hover = normalize_hover(&result).unwrap();
        assert!(hover.contents.contains("fn main"));
    }

    #[test]
    fn normalizes_marked_string_arrays() {
        let result = serde_json::json!({
            "contents": [
                { "language": "rust", "value": "fn value() -> i32" },
                "plain docs"
            ]
        });
        let hover = normalize_hover(&result).unwrap();
        assert!(hover.contents.contains("fn value"));
        assert!(hover.contents.contains("plain docs"));
    }

    #[test]
    fn empty_hover_is_none() {
        let result = serde_json::json!({ "contents": "" });
        assert_eq!(normalize_hover(&result), None);
    }
}
