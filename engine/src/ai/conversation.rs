//! Conversation persistence (Requirements 16 & 17).
//!
//! Two stores work together:
//! - **Project-level JSONL** at `<project_root>/.gwenland/conversations/<id>.jsonl`
//!   holds the actual message turns, append-only, one ChatML line per turn.
//! - **Global manifest** at `~/.gwenland/ide/conversations/index.json` is a
//!   metadata-only registry so conversations across projects are discoverable
//!   without crawling folders. Mutated via temp-file + rename, serialized by a
//!   process-wide lock.
//!
//! API keys never appear in either store. The home base is overridable via the
//! `GWENLAND_HOME` env var so tests can point at a tempdir.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};

use crate::ai::error::AiError;

/// Current manifest schema version.
pub const MANIFEST_VERSION: u32 = 1;

/// One ChatML message inside a persisted turn.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TurnMessage {
    pub role: String,
    pub content: String,
}

/// One completed exchange: the (attachment-expanded) user message plus the
/// assistant reply, with provenance. Serialized as a single JSONL line.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationTurn {
    pub messages: Vec<TurnMessage>,
    pub timestamp: String,
    pub provider: String,
    pub model: String,
}

/// Manifest metadata for one conversation (no message content).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationMeta {
    pub id: String,
    pub project_path: String,
    pub jsonl_path: String,
    pub title: String,
    pub provider: String,
    pub model: String,
    pub created_at: String,
    pub updated_at: String,
    pub training_opt_in: bool,
}

/// The global registry file contents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub version: u32,
    pub entries: Vec<ConversationMeta>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            version: MANIFEST_VERSION,
            entries: Vec::new(),
        }
    }
}

/// Serializes all in-process manifest mutations (Requirement 17.7).
static MANIFEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn io_err(e: impl std::fmt::Display) -> AiError {
    AiError::StorageError(e.to_string())
}

fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

/// Home base for the global manifest. `GWENLAND_HOME` overrides for tests.
fn home_base() -> Result<PathBuf, AiError> {
    if let Ok(custom) = std::env::var("GWENLAND_HOME")
        && !custom.is_empty()
    {
        return Ok(PathBuf::from(custom));
    }
    dirs::home_dir()
        .ok_or_else(|| AiError::StorageError("could not determine home directory".into()))
}

/// `~/.gwenland/ide/conversations/index.json`.
pub fn global_manifest_path() -> Result<PathBuf, AiError> {
    Ok(home_base()?
        .join(".gwenland")
        .join("ide")
        .join("conversations")
        .join("index.json"))
}

/// `<project_root>/.gwenland/conversations/`.
pub fn project_conversations_dir(project_root: &Path) -> PathBuf {
    project_root.join(".gwenland").join("conversations")
}

fn load_manifest_at(path: &Path) -> Result<Manifest, AiError> {
    if !path.exists() {
        return Ok(Manifest::default());
    }
    let content = fs::read_to_string(path).map_err(io_err)?;
    if content.trim().is_empty() {
        return Ok(Manifest::default());
    }
    serde_json::from_str(&content)
        .map_err(|e| AiError::StorageError(format!("corrupt conversation manifest: {e}")))
}

fn save_manifest_at(path: &Path, manifest: &Manifest) -> Result<(), AiError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    let json = serde_json::to_string_pretty(manifest).map_err(io_err)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(io_err)?;
    fs::rename(&tmp, path).map_err(io_err)?;
    Ok(())
}

/// Load → mutate → save the manifest atomically under the process lock.
fn with_manifest<F, T>(f: F) -> Result<T, AiError>
where
    F: FnOnce(&mut Manifest) -> T,
{
    let _guard = MANIFEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let path = global_manifest_path()?;
    let mut manifest = load_manifest_at(&path)?;
    let out = f(&mut manifest);
    save_manifest_at(&path, &manifest)?;
    Ok(out)
}

/// Read-only manifest snapshot under the lock.
fn read_manifest() -> Result<Manifest, AiError> {
    let _guard = MANIFEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    load_manifest_at(&global_manifest_path()?)
}

fn meta_for(id: &str) -> Result<ConversationMeta, AiError> {
    read_manifest()?
        .entries
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| AiError::StorageError(format!("unknown conversation: {id}")))
}

/// Public manifest lookup for one conversation (used by `ai_send` to resolve the
/// conversation's stored provider/model).
pub fn get_conversation(id: &str) -> Result<ConversationMeta, AiError> {
    meta_for(id)
}

/// Create a new conversation: canonicalize the project root, create the
/// conversations dir + an empty JSONL file, and register a manifest entry with
/// `training_opt_in = false` (Requirements 16.1, 16.2, 17.3, 18.1).
pub fn new_conversation(
    project_root: &Path,
    title: &str,
    provider: &str,
    model: &str,
) -> Result<ConversationMeta, AiError> {
    let canon = fs::canonicalize(project_root)
        .map_err(|e| AiError::StorageError(format!("invalid project root: {e}")))?;
    let dir = project_conversations_dir(&canon);
    fs::create_dir_all(&dir).map_err(io_err)?;

    let id = format!("conv-{}", uuid::Uuid::new_v4());
    let jsonl = dir.join(format!("{id}.jsonl"));
    fs::File::create(&jsonl).map_err(io_err)?; // empty, append-only

    let now = now_rfc3339();
    let title = if title.trim().is_empty() {
        "New Conversation".to_string()
    } else {
        title.trim().to_string()
    };
    let meta = ConversationMeta {
        id,
        project_path: canon.to_string_lossy().into_owned(),
        jsonl_path: jsonl.to_string_lossy().into_owned(),
        title,
        provider: provider.to_string(),
        model: model.to_string(),
        created_at: now.clone(),
        updated_at: now,
        training_opt_in: false,
    };

    let meta_clone = meta.clone();
    with_manifest(move |m| m.entries.push(meta_clone))?;
    Ok(meta)
}

/// List conversations, newest first, skipping entries whose JSONL is gone
/// (Requirements 17.8, 17.9).
pub fn list_conversations() -> Result<Vec<ConversationMeta>, AiError> {
    let mut entries = read_manifest()?.entries;
    entries.retain(|e| Path::new(&e.jsonl_path).exists());
    entries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(entries)
}

/// Parse a conversation's JSONL into turns, skipping blank lines and failing on
/// malformed non-blank lines (Requirements 16.5, 16.10, 19.1).
pub fn load_turns(conversation_id: &str) -> Result<Vec<ConversationTurn>, AiError> {
    let meta = meta_for(conversation_id)?;
    let content = match fs::read_to_string(&meta.jsonl_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_err(e)),
    };
    let mut turns = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let turn: ConversationTurn = serde_json::from_str(line)
            .map_err(|e| AiError::StorageError(format!("malformed JSONL line {}: {e}", idx + 1)))?;
        turns.push(turn);
    }
    Ok(turns)
}

/// Append one completed turn (Requirements 16.3-16.8). Builds exactly one
/// compact JSON line with a single trailing newline and flushes, then bumps the
/// manifest `updated_at`.
pub fn append_turn(conversation_id: &str, turn: &ConversationTurn) -> Result<(), AiError> {
    let meta = meta_for(conversation_id)?;
    let line = serde_json::to_string(turn).map_err(io_err)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&meta.jsonl_path)
        .map_err(io_err)?;
    file.write_all(line.as_bytes()).map_err(io_err)?;
    file.write_all(b"\n").map_err(io_err)?;
    file.flush().map_err(io_err)?;

    let id = conversation_id.to_string();
    let now = now_rfc3339();
    with_manifest(move |m| {
        if let Some(e) = m.entries.iter_mut().find(|e| e.id == id) {
            e.updated_at = now;
        }
    })
}

/// Convenience: build a completed user+assistant turn (stamping the current
/// time) and append it. Used by the streaming command on `ai://done`.
pub fn record_turn(
    conversation_id: &str,
    user_content: &str,
    assistant_content: &str,
    provider: &str,
    model: &str,
) -> Result<(), AiError> {
    let turn = ConversationTurn {
        messages: vec![
            TurnMessage {
                role: "user".into(),
                content: user_content.to_string(),
            },
            TurnMessage {
                role: "assistant".into(),
                content: assistant_content.to_string(),
            },
        ],
        timestamp: now_rfc3339(),
        provider: provider.to_string(),
        model: model.to_string(),
    };
    append_turn(conversation_id, &turn)
}

/// Rename only the manifest title (Requirement 17.9).
pub fn rename_conversation(conversation_id: &str, new_title: &str) -> Result<(), AiError> {
    let id = conversation_id.to_string();
    let title = new_title.trim().to_string();
    with_manifest(move |m| {
        if let Some(e) = m.entries.iter_mut().find(|e| e.id == id) {
            e.title = title;
            e.updated_at = now_rfc3339();
        }
    })
}

/// Delete the JSONL file and the manifest entry. A stale delete (already gone)
/// is success (Requirement 17.10).
pub fn delete_conversation(conversation_id: &str) -> Result<(), AiError> {
    let _guard = MANIFEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let path = global_manifest_path()?;
    let mut manifest = load_manifest_at(&path)?;
    if let Some(pos) = manifest
        .entries
        .iter()
        .position(|e| e.id == conversation_id)
    {
        let entry = manifest.entries.remove(pos);
        // Ignore a missing file: stale delete still succeeds.
        if Path::new(&entry.jsonl_path).exists() {
            let _ = fs::remove_file(&entry.jsonl_path);
        }
        save_manifest_at(&path, &manifest)?;
    }
    Ok(())
}

/// Set a conversation's training opt-in flag (Requirement 18.6/18.7). Only ever
/// called from an explicit user action; never flips to true implicitly.
pub fn set_training_opt_in(conversation_id: &str, opt_in: bool) -> Result<(), AiError> {
    let id = conversation_id.to_string();
    with_manifest(move |m| {
        if let Some(e) = m.entries.iter_mut().find(|e| e.id == id) {
            e.training_opt_in = opt_in;
            e.updated_at = now_rfc3339();
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// A turn serializes to exactly one newline-free JSON line and round-trips
        /// for arbitrary content (newlines, Unicode, quotes, code fences) —
        /// Requirements 16.5 / 20.5.
        #[test]
        fn conversation_turn_json_round_trip(user in ".*", assistant in ".*") {
            let turn = ConversationTurn {
                messages: vec![
                    TurnMessage { role: "user".into(), content: user },
                    TurnMessage { role: "assistant".into(), content: assistant },
                ],
                timestamp: "2026-06-20T00:00:00Z".into(),
                provider: "anthropic".into(),
                model: "claude-opus-4-8".into(),
            };
            let line = serde_json::to_string(&turn).unwrap();
            prop_assert!(!line.contains('\n'), "compact JSON must be single-line");
            let back: ConversationTurn = serde_json::from_str(&line).unwrap();
            prop_assert_eq!(turn, back);
        }
    }

    /// Serializes conversation tests: they share the process-global
    /// `GWENLAND_HOME` env var, so they must not overlap even when `cargo test`
    /// runs in parallel (the default). Each `TestEnv` holds this for its lifetime.
    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Point GWENLAND_HOME at a temp home and use a temp project root, while
    /// holding `TEST_LOCK` so the env override can't race another test.
    struct TestEnv {
        _home: tempfile::TempDir,
        project: tempfile::TempDir,
        // Declared last so the lock is released only after the temp dirs drop.
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl TestEnv {
        fn new() -> Self {
            // Recover a poisoned lock so one panicking test never wedges the rest.
            let guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let home = tempfile::tempdir().unwrap();
            unsafe { std::env::set_var("GWENLAND_HOME", home.path()) };
            Self {
                _home: home,
                project: tempfile::tempdir().unwrap(),
                _guard: guard,
            }
        }
        fn root(&self) -> &Path {
            self.project.path()
        }
    }

    fn turn(user: &str, assistant: &str) -> ConversationTurn {
        ConversationTurn {
            messages: vec![
                TurnMessage {
                    role: "user".into(),
                    content: user.into(),
                },
                TurnMessage {
                    role: "assistant".into(),
                    content: assistant.into(),
                },
            ],
            timestamp: "2026-06-20T00:00:00Z".into(),
            provider: "anthropic".into(),
            model: "claude-opus-4-8".into(),
        }
    }

    #[test]
    fn new_conversation_creates_jsonl_and_manifest_entry() {
        let env = TestEnv::new();
        let meta = new_conversation(env.root(), "Hello", "anthropic", "claude-opus-4-8").unwrap();
        assert!(meta.id.starts_with("conv-"));
        assert!(!meta.training_opt_in);
        assert!(Path::new(&meta.jsonl_path).exists());

        let list = list_conversations().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, meta.id);
        assert_eq!(list[0].title, "Hello");
    }

    #[test]
    fn append_then_load_round_trips_including_newlines_and_fences() {
        let env = TestEnv::new();
        let meta = new_conversation(env.root(), "t", "openai", "gpt-4o").unwrap();
        let t = turn("multi\nline", "```rust\nfn main() {}\n```\nDone");
        append_turn(&meta.id, &t).unwrap();

        let loaded = load_turns(&meta.id).unwrap();
        assert_eq!(loaded, vec![t]);
    }

    #[test]
    fn append_writes_one_line_per_turn() {
        let env = TestEnv::new();
        let meta = new_conversation(env.root(), "t", "openai", "gpt-4o").unwrap();
        append_turn(&meta.id, &turn("a", "b")).unwrap();
        append_turn(&meta.id, &turn("c", "d")).unwrap();
        let raw = fs::read_to_string(&meta.jsonl_path).unwrap();
        assert_eq!(raw.lines().filter(|l| !l.trim().is_empty()).count(), 2);
        assert!(raw.ends_with('\n'));
    }

    #[test]
    fn rename_updates_only_title() {
        let env = TestEnv::new();
        let meta = new_conversation(env.root(), "old", "openai", "gpt-4o").unwrap();
        rename_conversation(&meta.id, "new").unwrap();
        let list = list_conversations().unwrap();
        assert_eq!(list[0].title, "new");
        assert_eq!(list[0].id, meta.id);
    }

    #[test]
    fn delete_removes_file_and_entry_and_is_idempotent() {
        let env = TestEnv::new();
        let meta = new_conversation(env.root(), "t", "openai", "gpt-4o").unwrap();
        delete_conversation(&meta.id).unwrap();
        assert!(!Path::new(&meta.jsonl_path).exists());
        assert!(list_conversations().unwrap().is_empty());
        // Stale delete is still success.
        delete_conversation(&meta.id).unwrap();
    }

    #[test]
    fn stale_entry_is_skipped_on_list() {
        let env = TestEnv::new();
        let meta = new_conversation(env.root(), "t", "openai", "gpt-4o").unwrap();
        fs::remove_file(&meta.jsonl_path).unwrap();
        assert!(list_conversations().unwrap().is_empty());
    }

    #[test]
    fn training_opt_in_defaults_false_and_updates_explicitly() {
        let env = TestEnv::new();
        let meta = new_conversation(env.root(), "t", "openai", "gpt-4o").unwrap();
        assert!(!list_conversations().unwrap()[0].training_opt_in);
        set_training_opt_in(&meta.id, true).unwrap();
        assert!(list_conversations().unwrap()[0].training_opt_in);
    }

    #[test]
    fn load_turns_errors_on_malformed_line() {
        let env = TestEnv::new();
        let meta = new_conversation(env.root(), "t", "openai", "gpt-4o").unwrap();
        fs::write(&meta.jsonl_path, "not json\n").unwrap();
        assert!(matches!(
            load_turns(&meta.id),
            Err(AiError::StorageError(_))
        ));
    }
}
