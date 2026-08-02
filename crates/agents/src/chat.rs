use ade_core::error::AdeError;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const CHAT_SCHEMA: &str = "ade.chat.v1";
const MAX_TURNS: usize = 40;
const MAX_TOOL_RESULT_CHARS: usize = 4_000;
const MAX_TEXT_DELTA_CHARS: usize = 100_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatAttachmentMeta {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub absolute: Option<String>,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extracted_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatTurn {
    pub id: String,
    pub created_at: String,
    pub user: String,
    /// Agent stream events as received by Desktop (`type` tagged JSON).
    pub events: Vec<Value>,
    /// Optional structured attachment chips (path-first; mirrors Attached: block).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<ChatAttachmentMeta>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatThread {
    pub schema: String,
    pub id: String,
    pub updated_at: String,
    pub turns: Vec<ChatTurn>,
}

impl ChatThread {
    pub fn empty() -> Self {
        Self {
            schema: CHAT_SCHEMA.into(),
            id: Uuid::new_v4().to_string(),
            updated_at: Utc::now().to_rfc3339(),
            turns: Vec::new(),
        }
    }
}

pub struct ChatStore {
    root: PathBuf,
}

impl ChatStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn chat_dir(&self) -> PathBuf {
        self.root.join(".ade").join("chat")
    }

    fn thread_path(&self) -> PathBuf {
        self.chat_dir().join("thread.json")
    }

    /// Load the last thread for this workspace, or an empty thread if none.
    pub fn load(&self) -> Result<ChatThread, AdeError> {
        let path = self.thread_path();
        if !path.is_file() {
            return Ok(ChatThread::empty());
        }
        let raw = std::fs::read(&path)?;
        let thread: ChatThread = serde_json::from_slice(&raw)?;
        if thread.schema != CHAT_SCHEMA {
            return Err(AdeError::Config(format!(
                "unsupported chat schema '{}'",
                thread.schema
            )));
        }
        Ok(thread)
    }

    /// Replace the workspace thread (keeps last [`MAX_TURNS`], compacts bulky events).
    pub fn save(&self, turns: Vec<ChatTurn>) -> Result<ChatThread, AdeError> {
        let existing = self.load().unwrap_or_else(|_| ChatThread::empty());
        let mut compacted: Vec<ChatTurn> = turns
            .into_iter()
            .map(|mut turn| {
                if turn.id.trim().is_empty() {
                    turn.id = Uuid::new_v4().to_string();
                }
                if turn.created_at.trim().is_empty() {
                    turn.created_at = Utc::now().to_rfc3339();
                }
                turn.events = compact_events(turn.events);
                turn
            })
            .collect();
        if compacted.len() > MAX_TURNS {
            compacted = compacted.split_off(compacted.len() - MAX_TURNS);
        }
        let thread = ChatThread {
            schema: CHAT_SCHEMA.into(),
            id: if existing.turns.is_empty() {
                Uuid::new_v4().to_string()
            } else {
                existing.id
            },
            updated_at: Utc::now().to_rfc3339(),
            turns: compacted,
        };
        let dir = self.chat_dir();
        std::fs::create_dir_all(&dir)?;
        let payload = serde_json::to_vec_pretty(&thread)?;
        write_atomic(&self.thread_path(), &payload)?;
        Ok(thread)
    }

    pub fn clear(&self) -> Result<ChatThread, AdeError> {
        let thread = ChatThread::empty();
        let dir = self.chat_dir();
        std::fs::create_dir_all(&dir)?;
        let payload = serde_json::to_vec_pretty(&thread)?;
        write_atomic(&self.thread_path(), &payload)?;
        Ok(thread)
    }
}

fn compact_events(events: Vec<Value>) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let mut text_buf = String::new();

    let flush_text = |buf: &mut String, out: &mut Vec<Value>| {
        if buf.is_empty() {
            return;
        }
        let mut text = std::mem::take(buf);
        if text.len() > MAX_TEXT_DELTA_CHARS {
            text.truncate(MAX_TEXT_DELTA_CHARS);
            text.push('…');
        }
        out.push(serde_json::json!({ "type": "text_delta", "text": text }));
    };

    for event in events {
        let kind = event.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if kind == "text_delta" {
            if let Some(chunk) = event.get("text").and_then(|v| v.as_str()) {
                text_buf.push_str(chunk);
            }
            continue;
        }
        flush_text(&mut text_buf, &mut out);
        out.push(truncate_event(event));
    }
    flush_text(&mut text_buf, &mut out);
    out
}

fn truncate_event(mut event: Value) -> Value {
    let kind = event
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if kind == "tool_result" {
        if let Some(text) = event
            .get_mut("text")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
        {
            if text.len() > MAX_TOOL_RESULT_CHARS {
                let mut clipped = text;
                clipped.truncate(MAX_TOOL_RESULT_CHARS);
                clipped.push('…');
                event["text"] = Value::String(clipped);
            }
        }
    }
    event
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), AdeError> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_loads_and_clears_thread() {
        let root = std::env::temp_dir().join(format!("ade-chat-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let store = ChatStore::new(&root);
        let saved = store
            .save(vec![ChatTurn {
                id: "t1".into(),
                created_at: "2026-07-22T00:00:00Z".into(),
                user: "hello".into(),
                events: vec![
                    serde_json::json!({"type":"text_delta","text":"hi "}),
                    serde_json::json!({"type":"text_delta","text":"there"}),
                    serde_json::json!({"type":"completed","result":{"ok":true}}),
                ],
                attachments: None,
            }])
            .unwrap();
        assert_eq!(saved.turns.len(), 1);
        assert_eq!(saved.turns[0].events.len(), 2); // coalesced text + completed
        let text = saved.turns[0].events[0]
            .get("text")
            .and_then(|v| v.as_str());
        assert_eq!(text, Some("hi there"));

        let loaded = store.load().unwrap();
        assert_eq!(loaded.id, saved.id);
        assert_eq!(loaded.turns[0].user, "hello");

        let cleared = store.clear().unwrap();
        assert!(cleared.turns.is_empty());
        assert!(store.load().unwrap().turns.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn attachment_meta_round_trips_extract_and_transcript() {
        let root = std::env::temp_dir().join(format!("ade-chat-att-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let store = ChatStore::new(&root);
        let saved = store
            .save(vec![ChatTurn {
                id: "t1".into(),
                created_at: "2026-07-22T00:00:00Z".into(),
                user: "hello".into(),
                events: vec![],
                attachments: Some(vec![ChatAttachmentMeta {
                    id: "a1".into(),
                    name: "clip.mp3".into(),
                    path: ".ade/inbox/clip.mp3".into(),
                    absolute: None,
                    kind: "audio".into(),
                    mime: Some("audio/mpeg".into()),
                    size: Some(12),
                    fetched_path: None,
                    extracted_path: Some(".ade/inbox/extract.md".into()),
                    transcript_path: Some(".ade/inbox/transcript.md".into()),
                }]),
            }])
            .unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.turns[0].attachments, saved.turns[0].attachments);
        let meta = loaded.turns[0]
            .attachments
            .as_ref()
            .and_then(|items| items.first())
            .unwrap();
        assert_eq!(
            meta.extracted_path.as_deref(),
            Some(".ade/inbox/extract.md")
        );
        assert_eq!(
            meta.transcript_path.as_deref(),
            Some(".ade/inbox/transcript.md")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn caps_turn_count() {
        let root = std::env::temp_dir().join(format!("ade-chat-cap-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let store = ChatStore::new(&root);
        let turns = (0..(MAX_TURNS + 5))
            .map(|i| ChatTurn {
                id: format!("t{i}"),
                created_at: "2026-07-22T00:00:00Z".into(),
                user: format!("u{i}"),
                events: vec![],
                attachments: None,
            })
            .collect::<Vec<_>>();
        let saved = store.save(turns).unwrap();
        assert_eq!(saved.turns.len(), MAX_TURNS);
        assert_eq!(saved.turns[0].user, format!("u{}", 5));
        let _ = std::fs::remove_dir_all(root);
    }
}
