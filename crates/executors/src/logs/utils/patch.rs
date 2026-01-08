use std::sync::Arc;

use json_patch::Patch;
use serde::{Deserialize, Serialize};
use serde_json::{from_value, json, to_value};
use ts_rs::TS;
use workspace_utils::{diff::Diff, msg_store::MsgStore};

use crate::logs::{NormalizedEntry, utils::EntryIndexProvider};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, TS)]
#[serde(rename_all = "lowercase")]
enum PatchOperation {
    Add,
    Replace,
    Remove,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "type", content = "content")]
pub enum PatchType {
    NormalizedEntry(NormalizedEntry),
    Stdout(String),
    Stderr(String),
    Diff(Diff),
}

#[derive(Serialize)]
struct PatchEntry {
    op: PatchOperation,
    path: String,
    value: PatchType,
}

pub fn escape_json_pointer_segment(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

/// Helper functions to create JSON patches for conversation entries
pub struct ConversationPatch;

impl ConversationPatch {
    /// Create an ADD patch for a new conversation entry at the given index
    pub fn add_normalized_entry(entry_index: usize, entry: NormalizedEntry) -> Patch {
        let patch_entry = PatchEntry {
            op: PatchOperation::Add,
            path: format!("/entries/{entry_index}"),
            value: PatchType::NormalizedEntry(entry),
        };

        from_value(json!([patch_entry])).unwrap()
    }

    /// Create an ADD patch for a new string at the given index
    pub fn add_stdout(entry_index: usize, entry: String) -> Patch {
        let patch_entry = PatchEntry {
            op: PatchOperation::Add,
            path: format!("/entries/{entry_index}"),
            value: PatchType::Stdout(entry),
        };

        from_value(json!([patch_entry])).unwrap()
    }

    /// Create an ADD patch for a new string at the given index
    pub fn add_stderr(entry_index: usize, entry: String) -> Patch {
        let patch_entry = PatchEntry {
            op: PatchOperation::Add,
            path: format!("/entries/{entry_index}"),
            value: PatchType::Stderr(entry),
        };

        from_value(json!([patch_entry])).unwrap()
    }

    /// Create an ADD patch for a new diff at the given index
    pub fn add_diff(entry_index: String, diff: Diff) -> Patch {
        let patch_entry = PatchEntry {
            op: PatchOperation::Add,
            path: format!("/entries/{entry_index}"),
            value: PatchType::Diff(diff),
        };

        from_value(json!([patch_entry])).unwrap()
    }

    /// Create an ADD patch for a new diff at the given index
    pub fn replace_diff(entry_index: String, diff: Diff) -> Patch {
        let patch_entry = PatchEntry {
            op: PatchOperation::Replace,
            path: format!("/entries/{entry_index}"),
            value: PatchType::Diff(diff),
        };

        from_value(json!([patch_entry])).unwrap()
    }

    /// Create a REMOVE patch for removing a diff
    pub fn remove_diff(entry_index: String) -> Patch {
        from_value(json!([{
            "op": PatchOperation::Remove,
            "path": format!("/entries/{entry_index}"),
        }]))
        .unwrap()
    }

    /// Create a REPLACE patch for updating an existing conversation entry at the given index
    pub fn replace(entry_index: usize, entry: NormalizedEntry) -> Patch {
        let patch_entry = PatchEntry {
            op: PatchOperation::Replace,
            path: format!("/entries/{entry_index}"),
            value: PatchType::NormalizedEntry(entry),
        };

        from_value(json!([patch_entry])).unwrap()
    }

    pub fn remove(entry_index: usize) -> Patch {
        from_value(json!([{
            "op": PatchOperation::Remove,
            "path": format!("/entries/{entry_index}"),
        }]))
        .unwrap()
    }
}

/// Extract the entry index and `NormalizedEntry` from a JsonPatch if it contains one
pub fn extract_normalized_entry_from_patch(patch: &Patch) -> Option<(usize, NormalizedEntry)> {
    let value = to_value(patch).ok()?;
    let ops = value.as_array()?;
    ops.iter().rev().find_map(|op| {
        let path = op.get("path")?.as_str()?;
        let entry_index = path.strip_prefix("/entries/")?.parse::<usize>().ok()?;

        let value = op.get("value")?;
        (value.get("type")?.as_str()? == "NORMALIZED_ENTRY")
            .then(|| value.get("content"))
            .flatten()
            .and_then(|c| from_value::<NormalizedEntry>(c.clone()).ok())
            .map(|entry| (entry_index, entry))
    })
}

pub fn upsert_normalized_entry(
    msg_store: &Arc<MsgStore>,
    index: usize,
    normalized_entry: NormalizedEntry,
    is_new: bool,
) {
    if is_new {
        msg_store.push_patch(ConversationPatch::add_normalized_entry(
            index,
            normalized_entry,
        ));
    } else {
        msg_store.push_patch(ConversationPatch::replace(index, normalized_entry));
    }
}

pub fn add_normalized_entry(
    msg_store: &Arc<MsgStore>,
    index_provider: &EntryIndexProvider,
    normalized_entry: NormalizedEntry,
) -> usize {
    let index = index_provider.next();
    upsert_normalized_entry(msg_store, index, normalized_entry, true);
    index
}

pub fn replace_normalized_entry(
    msg_store: &Arc<MsgStore>,
    index: usize,
    normalized_entry: NormalizedEntry,
) {
    upsert_normalized_entry(msg_store, index, normalized_entry, false);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::logs::{NormalizedEntry, NormalizedEntryType, utils::EntryIndexProvider};

    #[test]
    fn escape_json_pointer_segment_escapes_tilde_and_slash() {
        assert_eq!(escape_json_pointer_segment("a/b~c"), "a~1b~0c");
    }

    #[test]
    fn extract_normalized_entry_from_patch_reads_entry() {
        let entry = NormalizedEntry {
            timestamp: None,
            entry_type: NormalizedEntryType::UserMessage,
            content: "hello".to_string(),
            metadata: None,
        };
        let value = serde_json::to_value(PatchType::NormalizedEntry(entry.clone()))
            .expect("value");
        let patch: Patch = serde_json::from_value(json!([{
            "op": "add",
            "path": "/entries/2",
            "value": value,
        }]))
        .expect("patch");

        let (index, extracted) =
            extract_normalized_entry_from_patch(&patch).expect("normalized entry");
        assert_eq!(index, 2);
        assert_eq!(extracted.content, "hello");
        assert!(matches!(
            extracted.entry_type,
            NormalizedEntryType::UserMessage
        ));
    }

    #[test]
    fn add_and_replace_normalized_entries_update_store() {
        let store = Arc::new(MsgStore::new());
        let index_provider = EntryIndexProvider::test_new();

        let first = NormalizedEntry {
            timestamp: None,
            entry_type: NormalizedEntryType::UserMessage,
            content: "first".to_string(),
            metadata: None,
        };
        let index = add_normalized_entry(&store, &index_provider, first);
        let (entries, _) = store.normalized_history_page(10, None);
        let stored: NormalizedEntry =
            serde_json::from_value(entries[0].entry_json["content"].clone()).expect("entry");
        assert_eq!(stored.content, "first");

        let second = NormalizedEntry {
            timestamp: None,
            entry_type: NormalizedEntryType::AssistantMessage,
            content: "second".to_string(),
            metadata: None,
        };
        replace_normalized_entry(&store, index, second);
        let (entries, _) = store.normalized_history_page(10, None);
        let stored: NormalizedEntry =
            serde_json::from_value(entries[0].entry_json["content"].clone()).expect("entry");
        assert_eq!(stored.content, "second");
        assert!(matches!(
            stored.entry_type,
            NormalizedEntryType::AssistantMessage
        ));
    }

    #[test]
    fn extract_normalized_entry_from_patch_returns_none_for_invalid_path() {
        let entry = NormalizedEntry {
            timestamp: None,
            entry_type: NormalizedEntryType::UserMessage,
            content: "hello".to_string(),
            metadata: None,
        };
        let value = serde_json::to_value(PatchType::NormalizedEntry(entry)).expect("value");
        let patch: Patch = serde_json::from_value(json!([{
            "op": "add",
            "path": "/entries/not-a-number",
            "value": value,
        }]))
        .expect("patch");

        assert!(extract_normalized_entry_from_patch(&patch).is_none());
    }

    #[test]
    fn extract_normalized_entry_from_patch_returns_none_for_malformed_entry() {
        let patch: Patch = serde_json::from_value(json!([{
            "op": "add",
            "path": "/entries/1",
            "value": {
                "type": "NORMALIZED_ENTRY",
                "content": "bad",
            },
        }]))
        .expect("patch");

        assert!(extract_normalized_entry_from_patch(&patch).is_none());
    }
}
