//! Diagnostics DTOs and severity mapping (Milestone 6).
//!
//! The engine exposes stable, UI-focused DTOs rather than raw protocol structs
//! (Requirement 13.6). The `publishDiagnostics` parsing and the latest-state
//! store are added in Wave 3; this file defines the shared types and severity
//! mapping they build on.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// UI-facing diagnostic severity. Maps from LSP's 1–4 integer scale
/// (Requirement 10.8); a missing value defaults to `Information`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl DiagnosticSeverity {
    /// Map an LSP severity integer (1=error … 4=hint) to a UI severity.
    /// Unknown or missing values fall back to `Information`.
    pub fn from_lsp(value: Option<u64>) -> DiagnosticSeverity {
        match value {
            Some(1) => DiagnosticSeverity::Error,
            Some(2) => DiagnosticSeverity::Warning,
            Some(3) => DiagnosticSeverity::Information,
            Some(4) => DiagnosticSeverity::Hint,
            _ => DiagnosticSeverity::Information,
        }
    }
}

/// Zero-based line/character range (LSP positions are UTF-16 code units; the UI
/// converts to CodeMirror offsets).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspRange {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

/// One normalized diagnostic for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspDiagnostic {
    pub range: LspRange,
    pub severity: DiagnosticSeverity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
}

/// Parse a `textDocument/publishDiagnostics` params object into the document
/// URI and its normalized diagnostics (Requirement 10.1–10.3/10.8). An empty
/// diagnostics array is preserved (it clears prior diagnostics, Requirement
/// 10.9). Returns `None` only when the params have no `uri`.
pub fn parse_publish_diagnostics(params: &Value) -> Option<(String, Vec<LspDiagnostic>)> {
    let uri = params.get("uri")?.as_str()?.to_string();
    let items = params
        .get("diagnostics")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().map(parse_one).collect())
        .unwrap_or_default();
    Some((uri, items))
}

/// Latest diagnostics per document URI (Requirement 10.1). Empty diagnostics
/// clear the entry (Requirement 10.9); closing a document removes it entirely
/// (Requirement 10.10).
#[derive(Debug, Default)]
pub struct DiagnosticsStore {
    by_uri: HashMap<String, Vec<LspDiagnostic>>,
}

impl DiagnosticsStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the diagnostics for `uri`. An empty list clears (removes) the
    /// entry so stale markers do not linger.
    pub fn set(&mut self, uri: String, diagnostics: Vec<LspDiagnostic>) {
        if diagnostics.is_empty() {
            self.by_uri.remove(&uri);
        } else {
            self.by_uri.insert(uri, diagnostics);
        }
    }

    pub fn get(&self, uri: &str) -> Option<&Vec<LspDiagnostic>> {
        self.by_uri.get(uri)
    }

    /// Drop all diagnostics for a closed document.
    pub fn clear_uri(&mut self, uri: &str) {
        self.by_uri.remove(uri);
    }

    pub fn len(&self) -> usize {
        self.by_uri.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_uri.is_empty()
    }
}

fn parse_one(d: &Value) -> LspDiagnostic {
    let range = d.get("range");
    let pos = |key: &str, field: &str| -> u32 {
        range
            .and_then(|r| r.get(key))
            .and_then(|p| p.get(field))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32
    };
    LspDiagnostic {
        range: LspRange {
            start_line: pos("start", "line"),
            start_character: pos("start", "character"),
            end_line: pos("end", "line"),
            end_character: pos("end", "character"),
        },
        severity: DiagnosticSeverity::from_lsp(d.get("severity").and_then(Value::as_u64)),
        source: d.get("source").and_then(Value::as_str).map(str::to_string),
        // `code` may be a string or a number in LSP; normalize both to a string.
        code: d.get("code").and_then(|c| match c {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        }),
        message: d
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_maps_all_lsp_levels() {
        assert_eq!(
            DiagnosticSeverity::from_lsp(Some(1)),
            DiagnosticSeverity::Error
        );
        assert_eq!(
            DiagnosticSeverity::from_lsp(Some(2)),
            DiagnosticSeverity::Warning
        );
        assert_eq!(
            DiagnosticSeverity::from_lsp(Some(3)),
            DiagnosticSeverity::Information
        );
        assert_eq!(
            DiagnosticSeverity::from_lsp(Some(4)),
            DiagnosticSeverity::Hint
        );
    }

    #[test]
    fn missing_or_unknown_severity_defaults_to_information() {
        assert_eq!(
            DiagnosticSeverity::from_lsp(None),
            DiagnosticSeverity::Information
        );
        assert_eq!(
            DiagnosticSeverity::from_lsp(Some(99)),
            DiagnosticSeverity::Information
        );
    }

    #[test]
    fn parse_publish_diagnostics_normalizes_fields() {
        let params = serde_json::json!({
            "uri": "file:///proj/src/main.rs",
            "diagnostics": [
                {
                    "range": {
                        "start": { "line": 3, "character": 4 },
                        "end": { "line": 3, "character": 9 }
                    },
                    "severity": 1,
                    "source": "rustc",
                    "code": "E0425",
                    "message": "cannot find value"
                },
                {
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 1 }
                    },
                    "severity": 2,
                    "code": 4321,
                    "message": "unused"
                }
            ]
        });
        let (uri, diags) = parse_publish_diagnostics(&params).unwrap();
        assert_eq!(uri, "file:///proj/src/main.rs");
        assert_eq!(diags.len(), 2);

        assert_eq!(diags[0].severity, DiagnosticSeverity::Error);
        assert_eq!(diags[0].range.start_line, 3);
        assert_eq!(diags[0].range.end_character, 9);
        assert_eq!(diags[0].source.as_deref(), Some("rustc"));
        assert_eq!(diags[0].code.as_deref(), Some("E0425"));

        assert_eq!(diags[1].severity, DiagnosticSeverity::Warning);
        // numeric code normalized to string
        assert_eq!(diags[1].code.as_deref(), Some("4321"));
        assert_eq!(diags[1].source, None);
    }

    #[test]
    fn parse_publish_diagnostics_missing_severity_defaults_information() {
        let params = serde_json::json!({
            "uri": "file:///a.py",
            "diagnostics": [
                { "range": { "start": {"line":1,"character":0}, "end": {"line":1,"character":2} }, "message": "x" }
            ]
        });
        let (_, diags) = parse_publish_diagnostics(&params).unwrap();
        assert_eq!(diags[0].severity, DiagnosticSeverity::Information);
    }

    #[test]
    fn parse_publish_diagnostics_empty_array_is_preserved() {
        let params = serde_json::json!({ "uri": "file:///a.ts", "diagnostics": [] });
        let (uri, diags) = parse_publish_diagnostics(&params).unwrap();
        assert_eq!(uri, "file:///a.ts");
        assert!(diags.is_empty());
    }

    #[test]
    fn parse_publish_diagnostics_without_uri_is_none() {
        let params = serde_json::json!({ "diagnostics": [] });
        assert!(parse_publish_diagnostics(&params).is_none());
    }

    #[test]
    fn store_set_get_and_empty_clears() {
        let mut store = DiagnosticsStore::new();
        let uri = "file:///x.rs".to_string();
        let diag = LspDiagnostic {
            range: LspRange {
                start_line: 0,
                start_character: 0,
                end_line: 0,
                end_character: 1,
            },
            severity: DiagnosticSeverity::Error,
            source: None,
            code: None,
            message: "boom".into(),
        };
        store.set(uri.clone(), vec![diag]);
        assert_eq!(store.get(&uri).map(|d| d.len()), Some(1));
        assert_eq!(store.len(), 1);

        // Empty diagnostics clear the previous state (Requirement 10.9).
        store.set(uri.clone(), vec![]);
        assert!(store.get(&uri).is_none());
        assert!(store.is_empty());
    }

    #[test]
    fn store_clear_uri_removes_closed_document() {
        let mut store = DiagnosticsStore::new();
        let uri = "file:///x.rs".to_string();
        store.set(
            uri.clone(),
            vec![LspDiagnostic {
                range: LspRange {
                    start_line: 0,
                    start_character: 0,
                    end_line: 0,
                    end_character: 0,
                },
                severity: DiagnosticSeverity::Hint,
                source: None,
                code: None,
                message: "m".into(),
            }],
        );
        store.clear_uri(&uri);
        assert!(store.is_empty());
    }

    #[test]
    fn diagnostic_dto_round_trips() {
        let d = LspDiagnostic {
            range: LspRange {
                start_line: 1,
                start_character: 2,
                end_line: 1,
                end_character: 5,
            },
            severity: DiagnosticSeverity::Warning,
            source: Some("rustc".into()),
            code: Some("E0308".into()),
            message: "mismatched types".into(),
        };
        let json = serde_json::to_string(&d).unwrap();
        let back: LspDiagnostic = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
    }
}
