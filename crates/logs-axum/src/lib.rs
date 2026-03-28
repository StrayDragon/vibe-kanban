use std::collections::BTreeSet;

use axum::{extract::ws::Message, response::sse::Event};
use futures::{StreamExt, TryStreamExt};
use json_patch::{Patch, PatchOperation};
use logs_protocol::{
    LogMsg,
    log_msg::{EV_FINISHED, EV_INVALIDATE, EV_JSON_PATCH, EV_SESSION_ID, EV_STDERR, EV_STDOUT},
};
use logs_store::{MsgStore, SequencedLogMsg};
use serde_json::Value;

pub trait LogMsgAxumExt {
    fn to_sse_event(&self) -> Event;
    fn to_ws_message(&self) -> Result<Message, serde_json::Error>;
    fn to_ws_message_unchecked(&self) -> Message;
}

impl LogMsgAxumExt for LogMsg {
    fn to_sse_event(&self) -> Event {
        match self {
            LogMsg::Stdout(s) => Event::default().event(EV_STDOUT).data(s.clone()),
            LogMsg::Stderr(s) => Event::default().event(EV_STDERR).data(s.clone()),
            LogMsg::JsonPatch(patch) => {
                let data = serde_json::to_string(patch).unwrap_or_else(|_| "[]".to_string());
                Event::default().event(EV_JSON_PATCH).data(data)
            }
            LogMsg::SessionId(s) => Event::default().event(EV_SESSION_ID).data(s.clone()),
            LogMsg::Finished => Event::default().event(EV_FINISHED).data(""),
        }
    }

    fn to_ws_message(&self) -> Result<Message, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(Message::Text(json.into()))
    }

    fn to_ws_message_unchecked(&self) -> Message {
        // Finished becomes JSON {finished: true}
        let json = match self {
            LogMsg::Finished => r#"{"finished":true}"#.to_string(),
            _ => serde_json::to_string(self)
                .unwrap_or_else(|_| r#"{"error":"serialization_failed"}"#.to_string()),
        };

        Message::Text(json.into())
    }
}

fn decode_pointer_segment(segment: &str) -> String {
    segment.replace("~1", "/").replace("~0", "~")
}

fn split_pointer_path(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(decode_pointer_segment)
        .collect()
}

fn invalidation_hints_from_patch(patch: &Patch) -> Option<Value> {
    let mut task_ids: BTreeSet<String> = BTreeSet::new();
    let mut workspace_ids: BTreeSet<String> = BTreeSet::new();
    let mut has_execution_process = false;

    for op in patch.iter() {
        let path = op.path();
        if path.is_empty() {
            continue;
        }

        let segments = split_pointer_path(path);
        if segments.is_empty() {
            continue;
        }

        match segments[0].as_str() {
            "tasks" => {
                if let Some(id) = segments.get(1) {
                    task_ids.insert(id.clone());
                }
            }
            "workspaces" => {
                if let Some(id) = segments.get(1) {
                    workspace_ids.insert(id.clone());
                }

                match op {
                    PatchOperation::Add(add) => {
                        if let Some(task_id) = add.value.get("task_id").and_then(|v| v.as_str()) {
                            task_ids.insert(task_id.to_string());
                        }
                    }
                    PatchOperation::Replace(replace) => {
                        if let Some(task_id) = replace.value.get("task_id").and_then(|v| v.as_str())
                        {
                            task_ids.insert(task_id.to_string());
                        }
                    }
                    _ => {}
                }
            }
            "execution_processes" => {
                has_execution_process = true;
            }
            _ => {}
        }
    }

    if task_ids.is_empty() && workspace_ids.is_empty() && !has_execution_process {
        return None;
    }

    Some(serde_json::json!({
        "taskIds": task_ids.into_iter().collect::<Vec<_>>(),
        "workspaceIds": workspace_ids.into_iter().collect::<Vec<_>>(),
        "hasExecutionProcess": has_execution_process,
    }))
}

pub trait SequencedLogMsgAxumExt {
    fn to_sse_event(&self) -> Event;
    fn to_invalidate_sse_event(&self) -> Option<Event>;
    fn to_ws_message_unchecked(&self) -> Message;
}

impl SequencedLogMsgAxumExt for SequencedLogMsg {
    fn to_sse_event(&self) -> Event {
        self.msg.to_sse_event().id(self.seq.to_string())
    }

    fn to_invalidate_sse_event(&self) -> Option<Event> {
        let LogMsg::JsonPatch(patch) = &self.msg else {
            return None;
        };

        let hints = invalidation_hints_from_patch(patch)?;
        let data = serde_json::to_string(&hints).ok()?;

        Some(
            Event::default()
                .event(EV_INVALIDATE)
                .id(self.seq.to_string())
                .data(data),
        )
    }

    fn to_ws_message_unchecked(&self) -> Message {
        if matches!(self.msg, LogMsg::Finished) {
            return Message::Text(format!(r#"{{"seq":{},"finished":true}}"#, self.seq).into());
        }

        let hints = match &self.msg {
            LogMsg::JsonPatch(patch) => invalidation_hints_from_patch(patch),
            _ => None,
        };

        let value = serde_json::to_value(&self.msg)
            .unwrap_or_else(|_| serde_json::json!({ "error": "serialization_failed" }));

        let value = match value {
            Value::Object(mut map) => {
                if let Some(hints) = hints {
                    map.insert("invalidate".to_string(), hints);
                }
                map.insert("seq".to_string(), Value::from(self.seq));
                Value::Object(map)
            }
            other => serde_json::json!({
                "seq": self.seq,
                "msg": other,
            }),
        };

        let json = serde_json::to_string(&value)
            .unwrap_or_else(|_| r#"{"error":"serialization_failed"}"#.to_string());

        Message::Text(json.into())
    }
}

pub trait MsgStoreAxumExt {
    fn sse_stream(&self) -> futures::stream::BoxStream<'static, Result<Event, std::io::Error>>;
}

impl MsgStoreAxumExt for std::sync::Arc<MsgStore> {
    fn sse_stream(&self) -> futures::stream::BoxStream<'static, Result<Event, std::io::Error>> {
        self.clone()
            .history_plus_stream()
            .map_ok(|m| m.to_sse_event())
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_text_message(message: Message) -> serde_json::Value {
        match message {
            Message::Text(text) => serde_json::from_str(&text).expect("valid json"),
            other => panic!("expected text message, got {other:?}"),
        }
    }

    #[test]
    fn sequenced_ws_json_patch_includes_seq_and_legacy_field() {
        let log_msg: LogMsg = serde_json::from_value(serde_json::json!({ "JsonPatch": [] }))
            .expect("valid JsonPatch log msg");
        let msg = SequencedLogMsg {
            seq: 42,
            msg: log_msg,
        };

        let value = decode_text_message(msg.to_ws_message_unchecked());
        assert_eq!(value["seq"], 42);
        assert!(value.get("JsonPatch").is_some(), "JsonPatch field missing");
    }

    #[test]
    fn sequenced_ws_finished_includes_seq_and_finished_true() {
        let msg = SequencedLogMsg {
            seq: 7,
            msg: LogMsg::Finished,
        };

        let value = decode_text_message(msg.to_ws_message_unchecked());
        assert_eq!(value["seq"], 7);
        assert_eq!(value["finished"], true);
        assert!(
            value.get("Finished").is_none(),
            "must preserve legacy finished shape"
        );
    }

    #[test]
    fn sequenced_ws_json_patch_includes_invalidate_hints_for_tasks() {
        let task_id = "11111111-1111-1111-1111-111111111111";
        let log_msg: LogMsg = serde_json::from_value(serde_json::json!({
            "JsonPatch": [
                { "op": "replace", "path": format!("/tasks/{task_id}"), "value": { "id": task_id } }
            ]
        }))
        .expect("valid JsonPatch log msg");

        let msg = SequencedLogMsg {
            seq: 1,
            msg: log_msg,
        };
        let value = decode_text_message(msg.to_ws_message_unchecked());
        assert_eq!(value["seq"], 1);

        let invalidate = value
            .get("invalidate")
            .and_then(|v| v.as_object())
            .expect("invalidate hints missing");
        assert_eq!(
            invalidate.get("taskIds"),
            Some(&serde_json::json!([task_id]))
        );
        assert_eq!(invalidate.get("workspaceIds"), Some(&serde_json::json!([])));
        assert_eq!(
            invalidate.get("hasExecutionProcess"),
            Some(&serde_json::json!(false))
        );
    }

    #[test]
    fn sequenced_ws_json_patch_includes_invalidate_hints_for_workspaces() {
        let workspace_id = "22222222-2222-2222-2222-222222222222";
        let task_id = "33333333-3333-3333-3333-333333333333";
        let log_msg: LogMsg = serde_json::from_value(serde_json::json!({
            "JsonPatch": [
                { "op": "add", "path": format!("/workspaces/{workspace_id}"), "value": { "task_id": task_id } }
            ]
        }))
        .expect("valid JsonPatch log msg");

        let msg = SequencedLogMsg {
            seq: 1,
            msg: log_msg,
        };
        let value = decode_text_message(msg.to_ws_message_unchecked());
        assert_eq!(value["seq"], 1);

        let invalidate = value
            .get("invalidate")
            .and_then(|v| v.as_object())
            .expect("invalidate hints missing");
        assert_eq!(
            invalidate.get("taskIds"),
            Some(&serde_json::json!([task_id]))
        );
        assert_eq!(
            invalidate.get("workspaceIds"),
            Some(&serde_json::json!([workspace_id]))
        );
        assert_eq!(
            invalidate.get("hasExecutionProcess"),
            Some(&serde_json::json!(false))
        );
    }

    #[test]
    fn sequenced_ws_json_patch_includes_invalidate_hints_for_execution_processes() {
        let process_id = "44444444-4444-4444-4444-444444444444";
        let log_msg: LogMsg = serde_json::from_value(serde_json::json!({
            "JsonPatch": [
                { "op": "remove", "path": format!("/execution_processes/{process_id}") }
            ]
        }))
        .expect("valid JsonPatch log msg");

        let msg = SequencedLogMsg {
            seq: 1,
            msg: log_msg,
        };
        let value = decode_text_message(msg.to_ws_message_unchecked());
        assert_eq!(value["seq"], 1);

        let invalidate = value
            .get("invalidate")
            .and_then(|v| v.as_object())
            .expect("invalidate hints missing");
        assert_eq!(invalidate.get("taskIds"), Some(&serde_json::json!([])));
        assert_eq!(invalidate.get("workspaceIds"), Some(&serde_json::json!([])));
        assert_eq!(
            invalidate.get("hasExecutionProcess"),
            Some(&serde_json::json!(true))
        );
    }

    #[test]
    fn invalidate_hints_decode_json_pointer_segments() {
        let task_id = "foo~1bar";
        let workspace_id = "baz~0qux";
        let log_msg: LogMsg = serde_json::from_value(serde_json::json!({
            "JsonPatch": [
                { "op": "replace", "path": format!("/tasks/{task_id}"), "value": {} },
                { "op": "add", "path": format!("/workspaces/{workspace_id}"), "value": { "task_id": "ignored" } }
            ]
        }))
        .expect("valid JsonPatch log msg");

        let msg = SequencedLogMsg {
            seq: 2,
            msg: log_msg,
        };
        let value = decode_text_message(msg.to_ws_message_unchecked());

        let invalidate = value
            .get("invalidate")
            .and_then(|v| v.as_object())
            .expect("invalidate hints missing");
        assert_eq!(
            invalidate.get("taskIds"),
            Some(&serde_json::json!(["foo/bar", "ignored"]))
        );
        assert_eq!(
            invalidate.get("workspaceIds"),
            Some(&serde_json::json!(["baz~qux"]))
        );
    }
}
