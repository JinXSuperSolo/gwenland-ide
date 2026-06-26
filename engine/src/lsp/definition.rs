//! `textDocument/definition` DTO and response normalization.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::root::file_uri_to_path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspDefinitionLocation {
    pub path: String,
    pub line: u32,
    pub character: u32,
}

pub fn normalize_definition(result: &Value) -> Option<LspDefinitionLocation> {
    match result {
        Value::Array(items) => items.iter().find_map(normalize_location),
        Value::Object(_) => normalize_location(result),
        _ => None,
    }
}

fn normalize_location(value: &Value) -> Option<LspDefinitionLocation> {
    let uri = value
        .get("uri")
        .and_then(Value::as_str)
        .or_else(|| value.get("targetUri").and_then(Value::as_str))?;
    let range = value
        .get("range")
        .or_else(|| value.get("targetSelectionRange"))
        .or_else(|| value.get("targetRange"))?;
    let start = range.get("start")?;
    let line = start.get("line").and_then(Value::as_u64).unwrap_or(0) as u32;
    let character = start.get("character").and_then(Value::as_u64).unwrap_or(0) as u32;
    let path = file_uri_to_path(uri)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| uri.to_string());
    Some(LspDefinitionLocation {
        path,
        line,
        character,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_location_array() {
        let result = serde_json::json!([
            {
                "uri": "file:///C:/proj/src/lib.rs",
                "range": { "start": { "line": 10, "character": 4 }, "end": { "line": 10, "character": 8 } }
            }
        ]);
        let loc = normalize_definition(&result).unwrap();
        assert!(loc.path.ends_with("proj\\src\\lib.rs") || loc.path.ends_with("proj/src/lib.rs"));
        assert_eq!(loc.line, 10);
        assert_eq!(loc.character, 4);
    }

    #[test]
    fn normalizes_location_link() {
        let result = serde_json::json!({
            "targetUri": "file:///tmp/main.ts",
            "targetSelectionRange": {
                "start": { "line": 2, "character": 1 },
                "end": { "line": 2, "character": 5 }
            }
        });
        let loc = normalize_definition(&result).unwrap();
        assert!(loc.path.ends_with("/tmp/main.ts") || loc.path.ends_with("\\tmp\\main.ts"));
        assert_eq!(loc.line, 2);
        assert_eq!(loc.character, 1);
    }

    #[test]
    fn null_definition_is_none() {
        assert_eq!(normalize_definition(&Value::Null), None);
    }
}
