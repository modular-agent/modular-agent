//! Session persistence for LLM conversation histories.
//!
//! A session is an append-only log of [`SessionEntry`] records — finalized
//! chat [`Message`]s and non-destructive [`SessionEntry::Compaction`] markers
//! — identified by a [`SessionMeta`] header. Two [`SessionStore`]
//! implementations are provided: [`InMemorySessionStore`] (non-persistent)
//! and [`JsonlSessionStore`] (one JSONL file per session on disk).
//! [`build_context`] converts a stored entry log into the message list to
//! send to an LLM, honoring the most recent compaction.
//!
//! # Ownership and lifetime
//!
//! A `SessionStore` is owned by the component that creates it — typically a
//! single agent instance — and lives exactly as long as its owner. There is
//! no global registry and no static state; dropping the owner drops the
//! store (persisted JSONL files of course remain on disk).
//!
//! # Invariants
//!
//! 1. **Only finalized messages reach the store.** Callers must append only
//!    messages with `streaming == false`; partial streaming emissions and
//!    their id-based dedup/replacement stay in the caller's memory.
//! 2. **The store API is append-only.** No delete, update, or overwrite
//!    operations exist. Compaction is recorded as a new entry that changes
//!    how the context is *built*, never by rewriting history. A crash can
//!    lose at most the single in-flight entry (appends flush immediately).

#![cfg(feature = "llm")]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

use crate::error::AgentError;
use crate::llm::Message;

fn default_version() -> u32 {
    1
}

fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Header metadata identifying one session.
///
/// Serialized as the first line of a JSONL session file. Unknown fields are
/// ignored and optional fields default on deserialization, so files written
/// by newer versions still parse where possible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMeta {
    /// Unique session id (UUID v4), unique across process restarts.
    pub id: String,

    /// Schema version of the session file. Currently always 1.
    #[serde(default = "default_version")]
    pub version: u32,

    /// Reserved for future session forking; always `None` for now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_session: Option<String>,

    /// RFC3339 creation timestamp.
    #[serde(default)]
    pub created_at: String,
}

impl SessionMeta {
    /// Creates a header for a new session with a freshly generated UUID v4
    /// id and the current time as `created_at`.
    pub fn new() -> Self {
        Self {
            id: new_uuid(),
            version: 1,
            parent_session: None,
            created_at: now_rfc3339(),
        }
    }
}

impl Default for SessionMeta {
    fn default() -> Self {
        Self::new()
    }
}

/// One record in a session's append-only log.
///
/// Serialized as an internally tagged object (`{"type": "message", ...}`).
/// `parent_id` is reserved for future tree/fork support and is always `None`
/// for now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEntry {
    /// A finalized chat message.
    Message {
        /// Unique entry id (UUID v4).
        id: String,
        /// Reserved for future tree/fork support; always `None` for now.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_id: Option<String>,
        /// RFC3339 timestamp of when the entry was recorded.
        timestamp: String,
        /// The finalized message (`streaming == false`).
        message: Message,
    },

    /// A non-destructive compaction marker: earlier history is summarized
    /// but never removed from the store.
    Compaction {
        /// Unique entry id (UUID v4).
        id: String,
        /// Reserved for future tree/fork support; always `None` for now.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_id: Option<String>,
        /// RFC3339 timestamp of when the entry was recorded.
        timestamp: String,
        /// Summary of the history preceding `first_kept_id`.
        summary: String,
        /// Entry id of the first Message entry kept verbatim in the built
        /// context.
        first_kept_id: String,
        /// Token count of the history before compaction, if known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tokens_before: Option<u64>,
    },
}

impl SessionEntry {
    /// Creates a Message entry with a freshly generated UUID v4 id and the
    /// current time as timestamp.
    pub fn message(message: Message) -> Self {
        SessionEntry::Message {
            id: new_uuid(),
            parent_id: None,
            timestamp: now_rfc3339(),
            message,
        }
    }

    /// Creates a Compaction entry with a freshly generated UUID v4 id and
    /// the current time as timestamp.
    pub fn compaction(summary: String, first_kept_id: String, tokens_before: Option<u64>) -> Self {
        SessionEntry::Compaction {
            id: new_uuid(),
            parent_id: None,
            timestamp: now_rfc3339(),
            summary,
            first_kept_id,
            tokens_before,
        }
    }

    /// The entry's unique id.
    pub fn id(&self) -> &str {
        match self {
            SessionEntry::Message { id, .. } => id,
            SessionEntry::Compaction { id, .. } => id,
        }
    }
}

/// Append-only storage for session logs.
///
/// The trait surface is deliberately append-only (invariant 2 of the
/// [module docs](self)): implementations must not expose delete, update, or
/// overwrite operations.
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Creates a new session from its header and returns the session id.
    ///
    /// Errors if a session with `meta.id` already exists.
    async fn create(&self, meta: SessionMeta) -> Result<String, AgentError>;

    /// Appends one entry to an existing session.
    ///
    /// Callers must append only finalized messages (`message.streaming ==
    /// false`, invariant 1 of the [module docs](self)); partial streaming
    /// messages belong in the caller's memory, not in the store.
    async fn append(&self, session_id: &str, entry: SessionEntry) -> Result<(), AgentError>;

    /// Loads all entries of a session in append order.
    async fn load(&self, session_id: &str) -> Result<Vec<SessionEntry>, AgentError>;

    /// Lists the headers of all sessions in the store.
    async fn list(&self) -> Result<Vec<SessionMeta>, AgentError>;
}

fn session_not_found(session_id: &str) -> AgentError {
    AgentError::Other(format!("Session not found: {session_id}"))
}

type SessionMap = BTreeMap<String, (SessionMeta, Vec<SessionEntry>)>;

/// Non-persistent [`SessionStore`] holding all sessions in memory.
///
/// Owned by whoever creates it; there is no global registry. All contents
/// are lost when the store is dropped.
#[derive(Debug, Default)]
pub struct InMemorySessionStore {
    sessions: Mutex<SessionMap>,
}

impl InMemorySessionStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, SessionMap> {
        // A poisoned lock only means another thread panicked mid-operation;
        // the map itself is still structurally valid.
        self.sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn create(&self, meta: SessionMeta) -> Result<String, AgentError> {
        let mut sessions = self.lock();
        let id = meta.id.clone();
        if sessions.contains_key(&id) {
            return Err(AgentError::DuplicateId(id));
        }
        sessions.insert(id.clone(), (meta, Vec::new()));
        Ok(id)
    }

    async fn append(&self, session_id: &str, entry: SessionEntry) -> Result<(), AgentError> {
        let mut sessions = self.lock();
        let (_, entries) = sessions
            .get_mut(session_id)
            .ok_or_else(|| session_not_found(session_id))?;
        entries.push(entry);
        Ok(())
    }

    async fn load(&self, session_id: &str) -> Result<Vec<SessionEntry>, AgentError> {
        let sessions = self.lock();
        let (_, entries) = sessions
            .get(session_id)
            .ok_or_else(|| session_not_found(session_id))?;
        Ok(entries.clone())
    }

    async fn list(&self) -> Result<Vec<SessionMeta>, AgentError> {
        let sessions = self.lock();
        Ok(sessions.values().map(|(meta, _)| meta.clone()).collect())
    }
}

/// Persistent [`SessionStore`] writing one JSONL file per session.
///
/// Each session `<id>` lives in `<dir>/<id>.jsonl`: line 1 is the
/// [`SessionMeta`] header as JSON, and every following line is one
/// [`SessionEntry`] as JSON. Appends open the file in append mode, write a
/// single line, and flush immediately, so a crash loses at most the
/// in-flight entry. When loading, a partially written final line (a
/// crash-truncated tail without a trailing newline) is discarded and
/// truncated from the file, so later appends stay line-aligned instead of
/// fusing onto the fragment; an unparseable complete line is an error.
#[derive(Debug, Clone)]
pub struct JsonlSessionStore {
    dir: PathBuf,
}

impl JsonlSessionStore {
    /// Creates a store rooted at `dir`. The directory is created lazily on
    /// the first [`SessionStore::create`] call.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn session_path(&self, session_id: &str) -> PathBuf {
        self.dir.join(format!("{session_id}.jsonl"))
    }
}

#[async_trait]
impl SessionStore for JsonlSessionStore {
    async fn create(&self, meta: SessionMeta) -> Result<String, AgentError> {
        tokio::fs::create_dir_all(&self.dir)
            .await
            .map_err(|e| AgentError::IoError(format!("Failed to create session dir: {e}")))?;
        let id = meta.id.clone();
        let path = self.session_path(&id);
        // create_new makes the existence check atomic with file creation.
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    AgentError::DuplicateId(id.clone())
                } else {
                    AgentError::IoError(format!("Failed to create session file: {e}"))
                }
            })?;
        let mut line = serde_json::to_string(&meta).map_err(|e| {
            AgentError::SerializationError(format!("Failed to serialize meta: {e}"))
        })?;
        line.push('\n');
        file.write_all(line.as_bytes())
            .await
            .map_err(|e| AgentError::IoError(format!("Failed to write session header: {e}")))?;
        file.flush()
            .await
            .map_err(|e| AgentError::IoError(format!("Failed to flush session file: {e}")))?;
        Ok(id)
    }

    async fn append(&self, session_id: &str, entry: SessionEntry) -> Result<(), AgentError> {
        let path = self.session_path(session_id);
        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    session_not_found(session_id)
                } else {
                    AgentError::IoError(format!("Failed to open session file: {e}"))
                }
            })?;
        let mut line = serde_json::to_string(&entry).map_err(|e| {
            AgentError::SerializationError(format!("Failed to serialize entry: {e}"))
        })?;
        line.push('\n');
        file.write_all(line.as_bytes())
            .await
            .map_err(|e| AgentError::IoError(format!("Failed to append session entry: {e}")))?;
        file.flush()
            .await
            .map_err(|e| AgentError::IoError(format!("Failed to flush session file: {e}")))?;
        Ok(())
    }

    async fn load(&self, session_id: &str) -> Result<Vec<SessionEntry>, AgentError> {
        let path = self.session_path(session_id);
        let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                session_not_found(session_id)
            } else {
                AgentError::IoError(format!("Failed to read session file: {e}"))
            }
        })?;
        // Every successful append ends with a newline, so content after the
        // last '\n' is a crash-truncated in-flight entry. Discard it and
        // truncate the file so later appends stay line-aligned instead of
        // fusing onto the fragment.
        let complete_len = if content.ends_with('\n') {
            content.len()
        } else {
            content.rfind('\n').map(|i| i + 1).unwrap_or(0)
        };
        if complete_len < content.len() {
            log::warn!("Discarding crash-truncated final line in session {session_id}");
            let file = tokio::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .await
                .map_err(|e| {
                    AgentError::IoError(format!("Failed to open session file for repair: {e}"))
                })?;
            file.set_len(complete_len as u64).await.map_err(|e| {
                AgentError::IoError(format!("Failed to truncate session file: {e}"))
            })?;
        }
        let mut lines = content[..complete_len].lines();
        let Some(header) = lines.next() else {
            return Err(AgentError::JsonParseError(format!(
                "Session file for {session_id} is empty (missing header line)"
            )));
        };
        serde_json::from_str::<SessionMeta>(header).map_err(|e| {
            AgentError::JsonParseError(format!("Invalid session header for {session_id}: {e}"))
        })?;
        let mut entries = Vec::new();
        for (i, line) in lines.enumerate() {
            let entry = serde_json::from_str::<SessionEntry>(line).map_err(|e| {
                AgentError::JsonParseError(format!(
                    "Invalid entry at line {} of session {session_id}: {e}",
                    i + 2
                ))
            })?;
            entries.push(entry);
        }
        Ok(entries)
    }

    async fn list(&self) -> Result<Vec<SessionMeta>, AgentError> {
        let mut read_dir = match tokio::fs::read_dir(&self.dir).await {
            Ok(rd) => rd,
            // No directory yet simply means no sessions have been created.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(AgentError::IoError(format!(
                    "Failed to read session dir: {e}"
                )));
            }
        };
        let mut metas = Vec::new();
        loop {
            let entry = match read_dir.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(e) => {
                    return Err(AgentError::IoError(format!(
                        "Failed to read session dir entry: {e}"
                    )));
                }
            };
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            // Session files grow without bound; only the header line is
            // needed here, so never read the whole file.
            let file = match tokio::fs::File::open(&path).await {
                Ok(file) => file,
                Err(e) => {
                    log::warn!("Skipping unreadable session file {}: {e}", path.display());
                    continue;
                }
            };
            let mut header = String::new();
            match tokio::io::BufReader::new(file).read_line(&mut header).await {
                Ok(0) => {
                    log::warn!("Skipping empty session file {}", path.display());
                    continue;
                }
                Ok(_) => {}
                Err(e) => {
                    log::warn!("Skipping unreadable session file {}: {e}", path.display());
                    continue;
                }
            }
            match serde_json::from_str::<SessionMeta>(header.trim_end()) {
                Ok(meta) => metas.push(meta),
                Err(e) => {
                    log::warn!(
                        "Skipping session file with invalid header {}: {e}",
                        path.display()
                    );
                }
            }
        }
        Ok(metas)
    }
}

/// Builds the LLM context from a session's entry log.
///
/// With no [`SessionEntry::Compaction`] entry, returns all Message entries'
/// messages in append order.
///
/// Otherwise the *last* Compaction entry wins: the result starts with a user
/// message whose text is `"[Conversation summary]\n"` followed by the
/// compaction's `summary`, then contains every Message entry from the one
/// whose entry id equals `first_kept_id` through the end of the log. If no
/// Message entry matches `first_kept_id`, the summary message is instead
/// followed by all Message entries positioned *after* that Compaction entry.
///
/// Entries before the cut are dropped from the returned context only — the
/// store keeps the full history (invariant 2 of the [module docs](self)).
pub fn build_context(entries: &[SessionEntry]) -> Vec<Message> {
    let last_compaction = entries.iter().enumerate().rev().find_map(|(i, e)| match e {
        SessionEntry::Compaction {
            summary,
            first_kept_id,
            ..
        } => Some((i, summary, first_kept_id)),
        _ => None,
    });

    let Some((compaction_index, summary, first_kept_id)) = last_compaction else {
        return entries.iter().filter_map(entry_message).collect();
    };

    let mut context = vec![Message::user(format!("[Conversation summary]\n{summary}"))];
    let start = entries
        .iter()
        .position(|e| matches!(e, SessionEntry::Message { id, .. } if id == first_kept_id))
        .unwrap_or(compaction_index + 1);
    context.extend(entries[start..].iter().filter_map(entry_message));
    context
}

fn entry_message(entry: &SessionEntry) -> Option<Message> {
    match entry {
        SessionEntry::Message { message, .. } => Some(message.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message_entry(id: &str, text: &str) -> SessionEntry {
        SessionEntry::Message {
            id: id.to_string(),
            parent_id: None,
            timestamp: now_rfc3339(),
            message: Message::user(text.to_string()),
        }
    }

    fn compaction_entry(id: &str, summary: &str, first_kept_id: &str) -> SessionEntry {
        SessionEntry::Compaction {
            id: id.to_string(),
            parent_id: None,
            timestamp: now_rfc3339(),
            summary: summary.to_string(),
            first_kept_id: first_kept_id.to_string(),
            tokens_before: Some(1000),
        }
    }

    // InMemorySessionStore tests

    #[tokio::test]
    async fn test_in_memory_round_trip() {
        let store = InMemorySessionStore::new();
        let meta = SessionMeta::new();
        let id = store.create(meta.clone()).await.unwrap();
        assert_eq!(id, meta.id);

        let e1 = message_entry("e1", "hello");
        let e2 = message_entry("e2", "world");
        store.append(&id, e1.clone()).await.unwrap();
        store.append(&id, e2.clone()).await.unwrap();

        let entries = store.load(&id).await.unwrap();
        assert_eq!(entries, vec![e1, e2]);

        let metas = store.list().await.unwrap();
        assert_eq!(metas, vec![meta]);
    }

    #[tokio::test]
    async fn test_in_memory_duplicate_create_errors() {
        let store = InMemorySessionStore::new();
        let meta = SessionMeta::new();
        store.create(meta.clone()).await.unwrap();
        assert!(store.create(meta).await.is_err());
    }

    #[tokio::test]
    async fn test_in_memory_unknown_session_errors() {
        let store = InMemorySessionStore::new();
        assert!(store.load("missing").await.is_err());
        assert!(
            store
                .append("missing", message_entry("e1", "x"))
                .await
                .is_err()
        );
    }

    // JsonlSessionStore tests

    #[tokio::test]
    async fn test_jsonl_replay_and_list_with_new_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonlSessionStore::new(dir.path());
        let meta = SessionMeta::new();
        let id = store.create(meta.clone()).await.unwrap();

        let e1 = message_entry("e1", "hello");
        let e2 = SessionEntry::compaction("summary".to_string(), "e1".to_string(), Some(42));
        store.append(&id, e1.clone()).await.unwrap();
        store.append(&id, e2.clone()).await.unwrap();

        // A brand-new store over the same directory must replay identically.
        let reopened = JsonlSessionStore::new(dir.path());
        let entries = reopened.load(&id).await.unwrap();
        assert_eq!(entries, vec![e1, e2]);

        let metas = reopened.list().await.unwrap();
        assert_eq!(metas, vec![meta]);
    }

    #[tokio::test]
    async fn test_jsonl_duplicate_create_errors() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonlSessionStore::new(dir.path());
        let meta = SessionMeta::new();
        store.create(meta.clone()).await.unwrap();
        assert!(store.create(meta).await.is_err());
    }

    #[tokio::test]
    async fn test_jsonl_truncated_final_line_is_discarded_and_repaired() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonlSessionStore::new(dir.path());
        let meta = SessionMeta::new();
        let id = store.create(meta).await.unwrap();
        let e1 = message_entry("e1", "hello");
        store.append(&id, e1.clone()).await.unwrap();

        // Simulate a crash-truncated tail.
        let path = dir.path().join(format!("{id}.jsonl"));
        let mut content = std::fs::read_to_string(&path).unwrap();
        let intact_len = content.len();
        content.push_str("{\"type\":\"message\",\"id\":\"e2");
        std::fs::write(&path, content).unwrap();

        let entries = store.load(&id).await.unwrap();
        assert_eq!(entries, vec![e1.clone()]);

        // load() truncated the partial tail away...
        let repaired = std::fs::read_to_string(&path).unwrap();
        assert_eq!(repaired.len(), intact_len);
        assert!(repaired.ends_with('\n'));

        // ...so appends after the resume do not fuse onto the fragment.
        let e2 = message_entry("e2", "world");
        store.append(&id, e2.clone()).await.unwrap();
        let e3 = message_entry("e3", "again");
        store.append(&id, e3.clone()).await.unwrap();
        let entries = store.load(&id).await.unwrap();
        assert_eq!(entries, vec![e1, e2, e3]);
    }

    #[tokio::test]
    async fn test_jsonl_unparseable_complete_final_line_errors() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonlSessionStore::new(dir.path());
        let meta = SessionMeta::new();
        let id = store.create(meta).await.unwrap();
        store
            .append(&id, message_entry("e1", "hello"))
            .await
            .unwrap();

        // A garbage line that ends with a newline is real corruption, not a
        // crash-truncated tail, and must not be silently dropped.
        let path = dir.path().join(format!("{id}.jsonl"));
        let mut content = std::fs::read_to_string(&path).unwrap();
        content.push_str("not json\n");
        std::fs::write(&path, content).unwrap();

        assert!(store.load(&id).await.is_err());
    }

    #[tokio::test]
    async fn test_jsonl_unparseable_middle_line_errors() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonlSessionStore::new(dir.path());
        let meta = SessionMeta::new();
        let id = store.create(meta).await.unwrap();
        store
            .append(&id, message_entry("e1", "hello"))
            .await
            .unwrap();

        let path = dir.path().join(format!("{id}.jsonl"));
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        // header, garbage, valid entry
        let corrupted = format!("{}\nnot json\n{}\n", lines[0], lines[1]);
        std::fs::write(&path, corrupted).unwrap();

        assert!(store.load(&id).await.is_err());
    }

    #[tokio::test]
    async fn test_jsonl_unknown_session_errors_and_empty_dir_lists_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonlSessionStore::new(dir.path().join("never-created"));
        assert!(store.load("missing").await.is_err());
        assert!(
            store
                .append("missing", message_entry("e1", "x"))
                .await
                .is_err()
        );
        assert_eq!(store.list().await.unwrap(), vec![]);
    }

    // build_context tests

    #[test]
    fn test_build_context_no_compaction() {
        let entries = vec![message_entry("e1", "a"), message_entry("e2", "b")];
        let context = build_context(&entries);
        assert_eq!(context.len(), 2);
        assert_eq!(context[0].text(), "a");
        assert_eq!(context[1].text(), "b");
    }

    #[test]
    fn test_build_context_with_compaction_drops_prefix() {
        let entries = vec![
            message_entry("e1", "old1"),
            message_entry("e2", "old2"),
            message_entry("e3", "kept"),
            message_entry("e4", "tail"),
            compaction_entry("c1", "the summary", "e3"),
        ];
        let context = build_context(&entries);
        assert_eq!(context.len(), 3);
        assert_eq!(context[0].role, "user");
        assert_eq!(context[0].text(), "[Conversation summary]\nthe summary");
        assert_eq!(context[1].text(), "kept");
        assert_eq!(context[2].text(), "tail");
    }

    #[test]
    fn test_build_context_last_compaction_wins() {
        let entries = vec![
            message_entry("e1", "a"),
            compaction_entry("c1", "first summary", "e1"),
            message_entry("e2", "b"),
            message_entry("e3", "c"),
            compaction_entry("c2", "second summary", "e3"),
            message_entry("e4", "d"),
        ];
        let context = build_context(&entries);
        assert_eq!(context.len(), 3);
        assert_eq!(context[0].text(), "[Conversation summary]\nsecond summary");
        assert_eq!(context[1].text(), "c");
        assert_eq!(context[2].text(), "d");
    }

    #[test]
    fn test_build_context_first_kept_id_not_found_falls_back() {
        let entries = vec![
            message_entry("e1", "old"),
            compaction_entry("c1", "the summary", "nonexistent"),
            message_entry("e2", "after1"),
            message_entry("e3", "after2"),
        ];
        let context = build_context(&entries);
        assert_eq!(context.len(), 3);
        assert_eq!(context[0].text(), "[Conversation summary]\nthe summary");
        assert_eq!(context[1].text(), "after1");
        assert_eq!(context[2].text(), "after2");
    }

    // Serde tests

    #[test]
    fn test_session_entry_message_serde_round_trip() {
        let entry = message_entry("e1", "hello");
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["type"], serde_json::json!("message"));
        // parent_id is None and must not emit a key.
        assert!(json.get("parent_id").is_none());
        let restored: SessionEntry = serde_json::from_value(json).unwrap();
        assert_eq!(restored, entry);
    }

    #[test]
    fn test_session_entry_compaction_serde_round_trip() {
        let entry = compaction_entry("c1", "summary", "e1");
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["type"], serde_json::json!("compaction"));
        assert_eq!(json["tokens_before"], serde_json::json!(1000));
        let restored: SessionEntry = serde_json::from_value(json).unwrap();
        assert_eq!(restored, entry);
    }

    #[test]
    fn test_session_entry_unknown_field_ignored() {
        let json = serde_json::json!({
            "type": "message",
            "id": "e1",
            "timestamp": "2026-07-19T00:00:00+00:00",
            "message": { "role": "user", "content": "hi" },
            "some_future_field": true,
        });
        let entry: SessionEntry = serde_json::from_value(json).unwrap();
        assert_eq!(entry.id(), "e1");
    }

    #[test]
    fn test_session_meta_serde_round_trip_and_forward_compat() {
        let meta = SessionMeta::new();
        let json = serde_json::to_value(&meta).unwrap();
        // parent_session is None and must not emit a key.
        assert!(json.get("parent_session").is_none());
        let restored: SessionMeta = serde_json::from_value(json).unwrap();
        assert_eq!(restored, meta);

        // Unknown fields are ignored; missing optional fields default.
        let json = serde_json::json!({
            "id": "s1",
            "some_future_field": "x",
        });
        let meta: SessionMeta = serde_json::from_value(json).unwrap();
        assert_eq!(meta.id, "s1");
        assert_eq!(meta.version, 1);
        assert_eq!(meta.parent_session, None);
        assert_eq!(meta.created_at, "");
    }

    #[test]
    fn test_generated_ids_are_unique() {
        let m1 = SessionMeta::new();
        let m2 = SessionMeta::new();
        assert_ne!(m1.id, m2.id);
        let e1 = SessionEntry::message(Message::user("a".to_string()));
        let e2 = SessionEntry::message(Message::user("a".to_string()));
        assert_ne!(e1.id(), e2.id());
    }
}
