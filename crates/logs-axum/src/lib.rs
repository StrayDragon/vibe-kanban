use std::collections::BTreeSet;

use axum::{extract::ws::Message, response::sse::Event};
use futures::{StreamExt, TryStreamExt};
use json_patch::{Patch, PatchOperation};
use logs_protocol::{
    LogMsg,
    log_msg::{EV_FINISHED, EV_INVALIDATE, EV_JSON_PATCH, EV_SESSION_ID, EV_STDERR, EV_STDOUT},
};
use logs_store::{MsgStore, SequencedLogMsg};
use serde::Serialize;
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
    if !segment.contains('~') {
        return segment.to_string();
    }

    let bytes = segment.as_bytes();
    let mut out = String::with_capacity(segment.len());
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == b'~' && idx + 1 < bytes.len() {
            match bytes[idx + 1] {
                b'0' => {
                    out.push('~');
                    idx += 2;
                    continue;
                }
                b'1' => {
                    out.push('/');
                    idx += 2;
                    continue;
                }
                _ => {}
            }
        }

        out.push(bytes[idx] as char);
        idx += 1;
    }
    out
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

        let Some(stripped) = path.strip_prefix('/') else {
            continue;
        };
        let (root, rest) = stripped.split_once('/').unwrap_or((stripped, ""));

        match root {
            "tasks" => {
                let id = rest.split('/').next().unwrap_or(rest);
                if !id.is_empty() {
                    task_ids.insert(decode_pointer_segment(id));
                }
            }
            "workspaces" => {
                let id = rest.split('/').next().unwrap_or(rest);
                if !id.is_empty() {
                    workspace_ids.insert(decode_pointer_segment(id));
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
        let LogMsg::JsonPatch(patch) = self.msg.as_ref() else {
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
        #[derive(Serialize)]
        #[serde(untagged)]
        enum WsMsg<'a> {
            Finished {
                seq: u64,
                finished: bool,
            },
            Stdout {
                seq: u64,
                #[serde(rename = "Stdout")]
                stdout: &'a str,
            },
            Stderr {
                seq: u64,
                #[serde(rename = "Stderr")]
                stderr: &'a str,
            },
            SessionId {
                seq: u64,
                #[serde(rename = "SessionId")]
                session_id: &'a str,
            },
            JsonPatch {
                seq: u64,
                #[serde(rename = "JsonPatch")]
                json_patch: &'a Patch,
                #[serde(skip_serializing_if = "Option::is_none")]
                invalidate: Option<Value>,
            },
        }

        let msg = match self.msg.as_ref() {
            LogMsg::Finished => WsMsg::Finished {
                seq: self.seq,
                finished: true,
            },
            LogMsg::Stdout(s) => WsMsg::Stdout {
                seq: self.seq,
                stdout: s,
            },
            LogMsg::Stderr(s) => WsMsg::Stderr {
                seq: self.seq,
                stderr: s,
            },
            LogMsg::SessionId(s) => WsMsg::SessionId {
                seq: self.seq,
                session_id: s,
            },
            LogMsg::JsonPatch(patch) => WsMsg::JsonPatch {
                seq: self.seq,
                json_patch: patch,
                invalidate: invalidation_hints_from_patch(patch),
            },
        };

        let json = serde_json::to_string(&msg)
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
            msg: log_msg.into(),
        };

        let value = decode_text_message(msg.to_ws_message_unchecked());
        assert_eq!(value["seq"], 42);
        assert!(value.get("JsonPatch").is_some(), "JsonPatch field missing");
    }

    #[test]
    fn sequenced_ws_finished_includes_seq_and_finished_true() {
        let msg = SequencedLogMsg {
            seq: 7,
            msg: LogMsg::Finished.into(),
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
            msg: log_msg.into(),
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
            msg: log_msg.into(),
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
            msg: log_msg.into(),
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
            msg: log_msg.into(),
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

    #[test]
    fn invalidation_hints_from_patch_table() {
        struct Case {
            name: &'static str,
            patch: serde_json::Value,
            expected: Option<serde_json::Value>,
        }

        let cases = [
            Case {
                name: "tasks add uses id from path",
                patch: serde_json::json!([
                    { "op": "add", "path": "/tasks/task-1", "value": {} }
                ]),
                expected: Some(serde_json::json!({
                    "taskIds": ["task-1"],
                    "workspaceIds": [],
                    "hasExecutionProcess": false,
                })),
            },
            Case {
                name: "tasks replace uses id from path",
                patch: serde_json::json!([
                    { "op": "replace", "path": "/tasks/task-2", "value": {} }
                ]),
                expected: Some(serde_json::json!({
                    "taskIds": ["task-2"],
                    "workspaceIds": [],
                    "hasExecutionProcess": false,
                })),
            },
            Case {
                name: "tasks remove uses id from path",
                patch: serde_json::json!([{ "op": "remove", "path": "/tasks/task-3" }]),
                expected: Some(serde_json::json!({
                    "taskIds": ["task-3"],
                    "workspaceIds": [],
                    "hasExecutionProcess": false,
                })),
            },
            Case {
                name: "workspaces add includes workspace id and task_id injection",
                patch: serde_json::json!([
                    {
                        "op": "add",
                        "path": "/workspaces/workspace-1",
                        "value": { "task_id": "task-1" }
                    }
                ]),
                expected: Some(serde_json::json!({
                    "taskIds": ["task-1"],
                    "workspaceIds": ["workspace-1"],
                    "hasExecutionProcess": false,
                })),
            },
            Case {
                name: "workspaces replace includes workspace id and task_id injection",
                patch: serde_json::json!([
                    {
                        "op": "replace",
                        "path": "/workspaces/workspace-2",
                        "value": { "task_id": "task-2" }
                    }
                ]),
                expected: Some(serde_json::json!({
                    "taskIds": ["task-2"],
                    "workspaceIds": ["workspace-2"],
                    "hasExecutionProcess": false,
                })),
            },
            Case {
                name: "workspaces remove includes workspace id only",
                patch: serde_json::json!([{ "op": "remove", "path": "/workspaces/workspace-3" }]),
                expected: Some(serde_json::json!({
                    "taskIds": [],
                    "workspaceIds": ["workspace-3"],
                    "hasExecutionProcess": false,
                })),
            },
            Case {
                name: "execution_processes add sets hasExecutionProcess",
                patch: serde_json::json!([
                    {
                        "op": "add",
                        "path": "/execution_processes/process-1",
                        "value": { "id": "process-1" }
                    }
                ]),
                expected: Some(serde_json::json!({
                    "taskIds": [],
                    "workspaceIds": [],
                    "hasExecutionProcess": true,
                })),
            },
            Case {
                name: "execution_processes replace sets hasExecutionProcess",
                patch: serde_json::json!([
                    {
                        "op": "replace",
                        "path": "/execution_processes/process-2",
                        "value": { "id": "process-2" }
                    }
                ]),
                expected: Some(serde_json::json!({
                    "taskIds": [],
                    "workspaceIds": [],
                    "hasExecutionProcess": true,
                })),
            },
            Case {
                name: "execution_processes remove sets hasExecutionProcess",
                patch: serde_json::json!([
                    { "op": "remove", "path": "/execution_processes/process-3" }
                ]),
                expected: Some(serde_json::json!({
                    "taskIds": [],
                    "workspaceIds": [],
                    "hasExecutionProcess": true,
                })),
            },
            Case {
                name: "json pointer segments are decoded",
                patch: serde_json::json!([
                    { "op": "replace", "path": "/tasks/foo~1bar", "value": {} },
                    { "op": "add", "path": "/workspaces/baz~0qux", "value": { "task_id": "ignored" } }
                ]),
                expected: Some(serde_json::json!({
                    "taskIds": ["foo/bar", "ignored"],
                    "workspaceIds": ["baz~qux"],
                    "hasExecutionProcess": false,
                })),
            },
            Case {
                name: "unrelated patch returns None",
                patch: serde_json::json!([{ "op": "replace", "path": "/unrelated", "value": {} }]),
                expected: None,
            },
        ];

        for case in cases {
            let patch: Patch = serde_json::from_value(case.patch).expect("valid patch");
            let hints = invalidation_hints_from_patch(&patch);
            assert_eq!(hints, case.expected, "case: {}", case.name);
        }
    }
}
