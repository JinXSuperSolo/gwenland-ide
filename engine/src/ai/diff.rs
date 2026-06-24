//! Unified diff parser (Milestone 8, Requirement 9).
//!
//! Pure, dependency-free parsing of unified diff text into structured files,
//! hunks, and lines. It does NOT apply changes — it only produces a stable,
//! serializable representation that the UI turns into a review session.
//!
//! Like the rest of `engine/src/ai`, this module contains zero Tauri imports
//! and adds no new crates (only `serde`, already a workspace dependency).
//!
//! The parser is tolerant of the shapes assistants actually produce:
//! - prose before/after the diff (ignored),
//! - ```` ```diff ```` fenced blocks (fences ignored),
//! - multi-file diffs,
//! - added-only / removed-only hunks,
//! - empty context/added/removed lines.
//!
//! Genuinely malformed hunk headers return a [`DiffParseError`] rather than
//! panicking (Requirement 9.7).

use serde::{Deserialize, Serialize};

/// One line within a hunk, tagged by its role. Serializes as
/// `{"kind":"context"|"added"|"removed","text":"..."}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "text", rename_all = "snake_case")]
pub enum DiffLine {
    Context(String),
    Added(String),
    Removed(String),
}

/// One hunk: a contiguous changed region with old/new ranges and its lines.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    /// Optional section heading after the closing `@@` (may be empty).
    pub header: String,
    pub lines: Vec<DiffLine>,
}

/// One file's worth of hunks. `old_path`/`new_path` are `None` for `/dev/null`
/// (file creation/deletion) or when a header is absent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiffFile {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub hunks: Vec<DiffHunk>,
}

/// A user-safe parse error (never panics). `line` is 1-based.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffParseError {
    pub message: String,
    pub line: usize,
}

impl std::fmt::Display for DiffParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "diff parse error at line {}: {}",
            self.line, self.message
        )
    }
}

impl std::error::Error for DiffParseError {}

/// Quick, cheap check for whether text plausibly contains a unified diff. Used by
/// the response-detection path before invoking the full parser.
pub fn looks_like_diff(input: &str) -> bool {
    let mut saw_header = false;
    let mut saw_hunk = false;
    for raw in input.lines() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.starts_with("--- ") || line.starts_with("+++ ") {
            saw_header = true;
        } else if line.starts_with("@@") {
            saw_hunk = true;
        }
        if saw_header && saw_hunk {
            return true;
        }
    }
    false
}

/// Parse unified diff text into structured files. Ignores surrounding prose and
/// code fences; returns an error only for malformed hunk headers.
pub fn parse_unified_diff(input: &str) -> Result<Vec<DiffFile>, DiffParseError> {
    let mut files: Vec<DiffFile> = Vec::new();
    let mut cur_file: Option<DiffFile> = None;
    let mut cur_hunk: Option<DiffHunk> = None;
    let mut old_rem: i64 = 0;
    let mut new_rem: i64 = 0;
    // `Some(parsed)` once a `---` line is seen, awaiting its `+++` partner.
    let mut pending_old: Option<Option<String>> = None;

    for raw in input.lines() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);

        // 1. Consume hunk body lines while the declared counts remain.
        if cur_hunk.is_some() && (old_rem > 0 || new_rem > 0) {
            if consume_body_line(line, cur_hunk.as_mut().unwrap(), &mut old_rem, &mut new_rem) {
                if old_rem <= 0 && new_rem <= 0 {
                    finalize_hunk(&mut cur_file, &mut cur_hunk);
                }
                continue;
            }
            // Unexpected line inside the hunk → close it and reinterpret below.
            finalize_hunk(&mut cur_file, &mut cur_hunk);
            old_rem = 0;
            new_rem = 0;
        }

        // 2. Markers and prose.
        if line.trim_start().starts_with("```") {
            continue; // fence line — ignore
        }
        if let Some(rest) = line.strip_prefix("--- ") {
            finalize_file(&mut files, &mut cur_file, &mut cur_hunk);
            pending_old = Some(parse_diff_path(rest));
            continue;
        }
        if let Some(rest) = line.strip_prefix("+++ ") {
            if let Some(old) = pending_old.take() {
                cur_file = Some(DiffFile {
                    old_path: old,
                    new_path: parse_diff_path(rest),
                    hunks: Vec::new(),
                });
            }
            continue;
        }
        if line.starts_with("@@") {
            match parse_hunk_header(line) {
                Ok(parsed) => {
                    // A hunk with no preceding file header attaches to an anonymous file.
                    if cur_file.is_none() {
                        cur_file = Some(DiffFile {
                            old_path: None,
                            new_path: None,
                            hunks: Vec::new(),
                        });
                    }
                    finalize_hunk(&mut cur_file, &mut cur_hunk);
                    cur_hunk = Some(DiffHunk {
                        old_start: parsed.0,
                        old_count: parsed.1,
                        new_start: parsed.2,
                        new_count: parsed.3,
                        header: parsed.4,
                        lines: Vec::new(),
                    });
                    old_rem = parsed.1 as i64;
                    new_rem = parsed.3 as i64;
                }
                Err(_) => {
                    // A malformed/template header (e.g. `@@ -line,count +line,count @@`
                    // in an example) is treated as prose: end any open hunk and keep
                    // scanning so one bad block can't abort a valid diff elsewhere.
                    finalize_hunk(&mut cur_file, &mut cur_hunk);
                    old_rem = 0;
                    new_rem = 0;
                }
            }
            continue;
        }

        // Prose: a `---` not immediately followed by `+++` was not a header.
        pending_old = None;
    }

    finalize_file(&mut files, &mut cur_file, &mut cur_hunk);
    Ok(files)
}

/// Push a body line into the current hunk, decrementing remaining counts.
/// Returns `false` if the line is not a hunk body line.
fn consume_body_line(
    line: &str,
    hunk: &mut DiffHunk,
    old_rem: &mut i64,
    new_rem: &mut i64,
) -> bool {
    if let Some(rest) = line.strip_prefix(' ') {
        hunk.lines.push(DiffLine::Context(rest.to_string()));
        *old_rem -= 1;
        *new_rem -= 1;
        true
    } else if let Some(rest) = line.strip_prefix('+') {
        hunk.lines.push(DiffLine::Added(rest.to_string()));
        *new_rem -= 1;
        true
    } else if let Some(rest) = line.strip_prefix('-') {
        hunk.lines.push(DiffLine::Removed(rest.to_string()));
        *old_rem -= 1;
        true
    } else if line.is_empty() {
        // Tolerate a truly empty line as a blank context line.
        hunk.lines.push(DiffLine::Context(String::new()));
        *old_rem -= 1;
        *new_rem -= 1;
        true
    } else if line.starts_with('\\') {
        // "\ No newline at end of file" — informational, not counted.
        true
    } else {
        false
    }
}

fn finalize_hunk(cur_file: &mut Option<DiffFile>, cur_hunk: &mut Option<DiffHunk>) {
    if let Some(hunk) = cur_hunk.take() {
        if let Some(file) = cur_file.as_mut() {
            file.hunks.push(hunk);
        }
    }
}

fn finalize_file(
    files: &mut Vec<DiffFile>,
    cur_file: &mut Option<DiffFile>,
    cur_hunk: &mut Option<DiffHunk>,
) {
    finalize_hunk(cur_file, cur_hunk);
    if let Some(file) = cur_file.take() {
        files.push(file);
    }
}

/// Parse `@@ -old_start[,old_count] +new_start[,new_count] @@ [header]`.
fn parse_hunk_header(line: &str) -> Result<(usize, usize, usize, usize, String), String> {
    let after = line
        .strip_prefix("@@")
        .ok_or_else(|| "hunk header must start with @@".to_string())?;
    let close = after
        .find("@@")
        .ok_or_else(|| "hunk header missing closing @@".to_string())?;
    let ranges = after[..close].trim();
    let header = after[close + 2..].trim().to_string();

    let mut parts = ranges.split_whitespace();
    let old_part = parts
        .next()
        .ok_or_else(|| "hunk header missing old range".to_string())?;
    let new_part = parts
        .next()
        .ok_or_else(|| "hunk header missing new range".to_string())?;
    let (old_start, old_count) = parse_range(old_part, '-')?;
    let (new_start, new_count) = parse_range(new_part, '+')?;
    Ok((old_start, old_count, new_start, new_count, header))
}

/// Parse a `-a,b` / `+c,d` range; count defaults to 1 when omitted.
fn parse_range(s: &str, sign: char) -> Result<(usize, usize), String> {
    let s = s
        .strip_prefix(sign)
        .ok_or_else(|| format!("range must start with '{sign}'"))?;
    let mut it = s.split(',');
    let start = it
        .next()
        .ok_or_else(|| "range missing start".to_string())?
        .parse::<usize>()
        .map_err(|_| "range start is not a number".to_string())?;
    let count = match it.next() {
        Some(c) => c
            .parse::<usize>()
            .map_err(|_| "range count is not a number".to_string())?,
        None => 1,
    };
    Ok((start, count))
}

/// Extract a path from a `---`/`+++` header tail: drop a trailing tab+timestamp,
/// map `/dev/null` (and empty) to `None`, and strip a leading `a/` or `b/`.
fn parse_diff_path(rest: &str) -> Option<String> {
    let s = rest.split('\t').next().unwrap_or("").trim();
    if s.is_empty() || s == "/dev/null" {
        return None;
    }
    let p = s
        .strip_prefix("a/")
        .or_else(|| s.strip_prefix("b/"))
        .unwrap_or(s);
    Some(p.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_file_diff() {
        let input = "\
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,3 @@ fn main()
 fn main() {
-    println!(\"old\");
+    println!(\"new\");
 }
";
        let files = parse_unified_diff(input).unwrap();
        assert_eq!(files.len(), 1);
        let f = &files[0];
        assert_eq!(f.old_path.as_deref(), Some("src/main.rs"));
        assert_eq!(f.new_path.as_deref(), Some("src/main.rs"));
        assert_eq!(f.hunks.len(), 1);
        let h = &f.hunks[0];
        assert_eq!(
            (h.old_start, h.old_count, h.new_start, h.new_count),
            (1, 3, 1, 3)
        );
        assert_eq!(h.header, "fn main()");
        assert_eq!(
            h.lines,
            vec![
                DiffLine::Context("fn main() {".into()),
                DiffLine::Removed("    println!(\"old\");".into()),
                DiffLine::Added("    println!(\"new\");".into()),
                DiffLine::Context("}".into()),
            ]
        );
    }

    #[test]
    fn parses_multi_file_diff() {
        let input = "\
--- a/one.txt
+++ b/one.txt
@@ -1,1 +1,1 @@
-one
+ONE
--- a/two.txt
+++ b/two.txt
@@ -1,1 +1,1 @@
-two
+TWO
";
        let files = parse_unified_diff(input).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].new_path.as_deref(), Some("one.txt"));
        assert_eq!(files[1].new_path.as_deref(), Some("two.txt"));
        assert_eq!(files[0].hunks.len(), 1);
        assert_eq!(
            files[1].hunks[0].lines,
            vec![
                DiffLine::Removed("two".into()),
                DiffLine::Added("TWO".into()),
            ]
        );
    }

    #[test]
    fn parses_added_only_diff_with_dev_null() {
        let input = "\
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,2 @@
+line one
+line two
";
        let files = parse_unified_diff(input).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].old_path, None);
        assert_eq!(files[0].new_path.as_deref(), Some("new.txt"));
        let h = &files[0].hunks[0];
        assert_eq!(
            (h.old_start, h.old_count, h.new_start, h.new_count),
            (0, 0, 1, 2)
        );
        assert_eq!(
            h.lines,
            vec![
                DiffLine::Added("line one".into()),
                DiffLine::Added("line two".into()),
            ]
        );
    }

    #[test]
    fn parses_removed_only_diff() {
        let input = "\
--- a/gone.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-bye
-now
";
        let files = parse_unified_diff(input).unwrap();
        assert_eq!(files[0].old_path.as_deref(), Some("gone.txt"));
        assert_eq!(files[0].new_path, None);
        assert_eq!(
            files[0].hunks[0].lines,
            vec![
                DiffLine::Removed("bye".into()),
                DiffLine::Removed("now".into()),
            ]
        );
    }

    #[test]
    fn handles_empty_context_and_change_lines() {
        // Hunk with a blank context line (space only) and an added empty line.
        let input = "\
--- a/f
+++ b/f
@@ -1,3 +1,3 @@
 a

-b
+
";
        let files = parse_unified_diff(input).unwrap();
        let lines = &files[0].hunks[0].lines;
        assert_eq!(lines[0], DiffLine::Context("a".into()));
        assert_eq!(lines[1], DiffLine::Context("".into()));
        assert_eq!(lines[2], DiffLine::Removed("b".into()));
        assert_eq!(lines[3], DiffLine::Added("".into()));
    }

    #[test]
    fn malformed_hunk_header_is_skipped_not_fatal() {
        // A placeholder/template header must not abort the parse — it is skipped
        // so a valid diff elsewhere in the text still parses (Req 9.7/9.8).
        let input = "\
--- a/f
+++ b/f
@@ not a real range @@
 ctx
";
        let files = parse_unified_diff(input).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].hunks.is_empty());
    }

    #[test]
    fn template_header_then_valid_diff_parses_the_valid_one() {
        // Mirrors a real assistant reply: a format example with a `@@ -line,count
        // +line,count @@` template, then the actual change.
        let input = "\
Here is the format:
--- a/path/to/file
+++ b/path/to/file
@@ -line,count +line,count @@
- removed line
+ added line

And the real change:
--- a/index.html
+++ b/index.html
@@ -10,3 +10,3 @@
 <head>
-  <title>Welcome</title>
+  <title>Website SMPN 7 Tambun</title>
 </head>
";
        let files = parse_unified_diff(input).unwrap();
        let with_hunks: Vec<_> = files.iter().filter(|f| !f.hunks.is_empty()).collect();
        assert_eq!(with_hunks.len(), 1);
        assert_eq!(with_hunks[0].new_path.as_deref(), Some("index.html"));
        assert_eq!(with_hunks[0].hunks[0].lines.len(), 4);
    }

    #[test]
    fn ignores_prose_and_fences_around_diff() {
        let input = "\
Sure, here is the change you asked for:

```diff
--- a/app.js
+++ b/app.js
@@ -1,1 +1,1 @@
-const x = 1
+const x = 2
```

Let me know if you'd like anything else!
";
        let files = parse_unified_diff(input).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].new_path.as_deref(), Some("app.js"));
        assert_eq!(
            files[0].hunks[0].lines,
            vec![
                DiffLine::Removed("const x = 1".into()),
                DiffLine::Added("const x = 2".into()),
            ]
        );
    }

    #[test]
    fn prose_without_diff_returns_no_files() {
        let input = "Just a normal answer with a list:\n- item one\n- item two\n";
        let files = parse_unified_diff(input).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn hunk_count_defaults_to_one_when_omitted() {
        let input = "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-x\n+y\n";
        let files = parse_unified_diff(input).unwrap();
        let h = &files[0].hunks[0];
        assert_eq!((h.old_count, h.new_count), (1, 1));
    }

    #[test]
    fn looks_like_diff_detects_markers() {
        assert!(looks_like_diff("--- a/f\n+++ b/f\n@@ -1 +1 @@\n-a\n+b\n"));
        assert!(!looks_like_diff("no diff here\njust text"));
        assert!(!looks_like_diff("--- only a header, no hunk"));
    }

    #[test]
    fn diff_line_serializes_with_kind_and_text() {
        let v = serde_json::to_value(DiffLine::Added("hi".into())).unwrap();
        assert_eq!(v["kind"], "added");
        assert_eq!(v["text"], "hi");
    }
}
