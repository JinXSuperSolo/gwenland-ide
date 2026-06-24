//! Completion DTO and (Wave 5) response normalization (Milestone 6).
//!
//! This file defines the stable completion DTO now; the `CompletionList` /
//! `CompletionItem[]` normalization and insert-text precedence logic are added
//! in Wave 5 (task 9.2).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One normalized completion option for the CodeMirror autocomplete source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspCompletionOption {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    pub insert_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// Max documentation length kept per item (a noisy server can attach huge docs).
const MAX_DOC_LEN: usize = 500;

/// Normalize a `textDocument/completion` result into stable DTOs. Accepts both
/// response shapes (Requirement 11.4): a bare `CompletionItem[]` array or a
/// `CompletionList { items: [...] }` object. Anything else yields no options.
pub fn normalize_completion(result: &Value) -> Vec<LspCompletionOption> {
    let items: Vec<Value> = if let Some(arr) = result.as_array() {
        arr.clone()
    } else if let Some(arr) = result.get("items").and_then(Value::as_array) {
        arr.clone()
    } else {
        Vec::new()
    };
    items.iter().map(normalize_item).collect()
}

fn normalize_item(item: &Value) -> LspCompletionOption {
    let label = item
        .get("label")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // Insert-text precedence: textEdit.newText > insertText > label (Req 11.7).
    let mut insert_text = item
        .get("textEdit")
        .and_then(|te| te.get("newText"))
        .and_then(Value::as_str)
        .or_else(|| item.get("insertText").and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| label.clone());

    // Snippet safety (Requirement 11.8): we advertise snippetSupport=false, but a
    // server may still mark an item as a snippet (insertTextFormat == 2). Rather
    // than insert raw `${1:...}` placeholders, fall back to the plain label.
    let is_snippet = item.get("insertTextFormat").and_then(Value::as_u64) == Some(2);
    if is_snippet {
        insert_text = label.clone();
    }

    LspCompletionOption {
        label,
        detail: item
            .get("detail")
            .and_then(Value::as_str)
            .map(str::to_string),
        documentation: parse_documentation(item.get("documentation")),
        insert_text,
        kind: item
            .get("kind")
            .and_then(Value::as_u64)
            .map(kind_name)
            .map(str::to_string),
    }
}

/// LSP `documentation` is `string | MarkupContent { kind, value }`. Extract plain
/// text and clamp the length.
fn parse_documentation(doc: Option<&Value>) -> Option<String> {
    let text = match doc {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(o)) => o.get("value").and_then(Value::as_str)?.to_string(),
        _ => return None,
    };
    if text.is_empty() {
        return None;
    }
    if text.chars().count() > MAX_DOC_LEN {
        Some(text.chars().take(MAX_DOC_LEN).collect::<String>() + "…")
    } else {
        Some(text)
    }
}

/// Map an LSP `CompletionItemKind` integer to a CodeMirror-friendly type string.
fn kind_name(kind: u64) -> &'static str {
    match kind {
        2 => "method",
        3 => "function",
        4 => "function", // constructor
        5 | 10 => "property",
        6 => "variable",
        7 | 22 => "class", // class / struct
        8 => "interface",
        9 => "namespace", // module
        13 | 20 => "enum",
        14 => "keyword",
        21 => "constant",
        25 => "type", // type parameter
        _ => "text",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_option_round_trips() {
        let o = LspCompletionOption {
            label: "println!".into(),
            detail: Some("macro".into()),
            documentation: None,
            insert_text: "println!".into(),
            kind: Some("function".into()),
        };
        let json = serde_json::to_string(&o).unwrap();
        let back: LspCompletionOption = serde_json::from_str(&json).unwrap();
        assert_eq!(o, back);
    }

    #[test]
    fn normalizes_completion_list_shape() {
        let result = serde_json::json!({
            "isIncomplete": false,
            "items": [
                { "label": "foo", "kind": 3, "detail": "fn foo()" },
                { "label": "bar", "kind": 6 }
            ]
        });
        let opts = normalize_completion(&result);
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].label, "foo");
        assert_eq!(opts[0].kind.as_deref(), Some("function"));
        assert_eq!(opts[0].detail.as_deref(), Some("fn foo()"));
        assert_eq!(opts[0].insert_text, "foo"); // falls back to label
        assert_eq!(opts[1].kind.as_deref(), Some("variable"));
    }

    #[test]
    fn normalizes_completion_item_array_shape() {
        let result = serde_json::json!([
            { "label": "alpha" },
            { "label": "beta", "insertText": "beta()" }
        ]);
        let opts = normalize_completion(&result);
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].insert_text, "alpha");
        assert_eq!(opts[1].insert_text, "beta()");
    }

    #[test]
    fn insert_text_precedence_prefers_text_edit() {
        // textEdit.newText wins over insertText and label.
        let item = serde_json::json!([{
            "label": "label_val",
            "insertText": "insert_val",
            "textEdit": { "range": {}, "newText": "edit_val" }
        }]);
        assert_eq!(normalize_completion(&item)[0].insert_text, "edit_val");

        // Without textEdit, insertText wins over label.
        let item = serde_json::json!([{ "label": "label_val", "insertText": "insert_val" }]);
        assert_eq!(normalize_completion(&item)[0].insert_text, "insert_val");

        // Bare label.
        let item = serde_json::json!([{ "label": "label_val" }]);
        assert_eq!(normalize_completion(&item)[0].insert_text, "label_val");
    }

    #[test]
    fn snippet_items_fall_back_to_label() {
        // insertTextFormat == 2 (snippet): we must not insert raw placeholders.
        let item = serde_json::json!([{
            "label": "match",
            "insertText": "match ${1:expr} {\n\t$0\n}",
            "insertTextFormat": 2
        }]);
        let opts = normalize_completion(&item);
        assert_eq!(opts[0].insert_text, "match");
    }

    #[test]
    fn documentation_supports_string_and_markup() {
        let item = serde_json::json!([{ "label": "a", "documentation": "plain doc" }]);
        assert_eq!(
            normalize_completion(&item)[0].documentation.as_deref(),
            Some("plain doc")
        );

        let item = serde_json::json!([{
            "label": "b",
            "documentation": { "kind": "markdown", "value": "**markup** doc" }
        }]);
        assert_eq!(
            normalize_completion(&item)[0].documentation.as_deref(),
            Some("**markup** doc")
        );
    }

    #[test]
    fn unknown_shape_yields_no_options() {
        assert!(normalize_completion(&Value::Null).is_empty());
        assert!(normalize_completion(&serde_json::json!({"foo": "bar"})).is_empty());
    }
}
