use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, OnceLock, RwLock},
};

use axum::response::sse::Event;
use futures::{StreamExt, TryStreamExt, future};
use json_patch::{Patch, PatchOperation};
use serde_json::Value;
use tokio::{sync::broadcast, task::JoinHandle};
use tokio_stream::wrappers::BroadcastStream;

use crate::{log_msg::LogMsg, stream_lines::LinesStreamExt};

const DEFAULT_HISTORY_MAX_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_HISTORY_MAX_ENTRIES: usize = 5000;

struct LogHistoryConfig {
    max_bytes: usize,
    max_entries: usize,
}

static LOG_HISTORY_CONFIG: OnceLock<LogHistoryConfig> = OnceLock::new();

fn log_history_config() -> &'static LogHistoryConfig {
    LOG_HISTORY_CONFIG.get_or_init(|| {
        let max_bytes = read_env_usize("VK_LOG_HISTORY_MAX_BYTES", DEFAULT_HISTORY_MAX_BYTES);
        let max_entries = read_env_usize("VK_LOG_HISTORY_MAX_ENTRIES", DEFAULT_HISTORY_MAX_ENTRIES);

        LogHistoryConfig {
            max_bytes: normalize_limit(max_bytes, "VK_LOG_HISTORY_MAX_BYTES"),
            max_entries: normalize_limit(max_entries, "VK_LOG_HISTORY_MAX_ENTRIES"),
        }
    })
}

fn read_env_usize(name: &str, default: usize) -> usize {
    match std::env::var(name) {
        Ok(value) => match value.parse::<usize>() {
            Ok(parsed) => parsed,
            Err(err) => {
                tracing::warn!("Invalid {name}='{value}': {err}. Using default {default}.");
                default
            }
        },
        Err(_) => default,
    }
}

fn normalize_limit(value: usize, name: &str) -> usize {
    if value == 0 {
        tracing::warn!("{name} set to 0. Using minimum value 1 instead.");
        1
    } else {
        value
    }
}

#[derive(Clone)]
struct StoredMsg {
    msg: LogMsg,
    bytes: usize,
}

#[derive(Clone, Debug)]
pub struct LogEntrySnapshot {
    pub entry_index: usize,
    pub entry_json: Value,
}

#[derive(Clone, Debug)]
pub enum LogEntryEvent {
    Append { entry_index: usize, entry: Value },
    Replace { entry_index: usize, entry: Value },
    Finished,
}

struct StoredEntry {
    entry_index: usize,
    entry_json: Value,
    bytes: usize,
}

struct Inner {
    history: VecDeque<StoredMsg>,
    total_bytes: usize,
    raw_entries: VecDeque<StoredEntry>,
    raw_total_bytes: usize,
    raw_next_index: usize,
    raw_evicted: bool,
    normalized_entries: BTreeMap<usize, StoredEntry>,
    normalized_total_bytes: usize,
    normalized_max_index: usize,
    normalized_evicted: bool,
    finished: bool,
}

pub struct MsgStore {
    inner: RwLock<Inner>,
    sender: broadcast::Sender<LogMsg>,
    raw_sender: broadcast::Sender<LogEntryEvent>,
    normalized_sender: broadcast::Sender<LogEntryEvent>,
}

impl Default for MsgStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MsgStore {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(10000);
        let (raw_sender, _) = broadcast::channel(10000);
        let (normalized_sender, _) = broadcast::channel(10000);
        Self {
            inner: RwLock::new(Inner {
                history: VecDeque::with_capacity(32),
                total_bytes: 0,
                raw_entries: VecDeque::with_capacity(64),
                raw_total_bytes: 0,
                raw_next_index: 0,
                raw_evicted: false,
                normalized_entries: BTreeMap::new(),
                normalized_total_bytes: 0,
                normalized_max_index: 0,
                normalized_evicted: false,
                finished: false,
            }),
            sender,
            raw_sender,
            normalized_sender,
        }
    }

    pub fn push(&self, msg: LogMsg) {
        let _ = self.sender.send(msg.clone());
        let bytes = msg.approx_bytes();

        let mut raw_events: Vec<LogEntryEvent> = Vec::new();
        let mut normalized_events: Vec<LogEntryEvent> = Vec::new();

        {
            let mut inner = self.inner.write().unwrap();
            inner.push_msg(msg.clone(), bytes);

            match msg {
                LogMsg::Stdout(content) => {
                    if let Some(event) = inner.push_raw_entry(content, true) {
                        raw_events.push(event);
                    }
                }
                LogMsg::Stderr(content) => {
                    if let Some(event) = inner.push_raw_entry(content, false) {
                        raw_events.push(event);
                    }
                }
                LogMsg::JsonPatch(patch) => {
                    let updates = extract_normalized_updates(&patch);
                    for update in updates {
                        if let Some(event) = inner.upsert_normalized_entry(update) {
                            normalized_events.push(event);
                        }
                    }
                }
                LogMsg::Finished => {
                    inner.finished = true;
                    raw_events.push(LogEntryEvent::Finished);
                    normalized_events.push(LogEntryEvent::Finished);
                }
                _ => {}
            }
        }

        for event in raw_events {
            let _ = self.raw_sender.send(event);
        }
        for event in normalized_events {
            let _ = self.normalized_sender.send(event);
        }
    }

    // Convenience
    pub fn push_stdout<S: Into<String>>(&self, s: S) {
        self.push(LogMsg::Stdout(s.into()));
    }

    pub fn push_stderr<S: Into<String>>(&self, s: S) {
        self.push(LogMsg::Stderr(s.into()));
    }
    pub fn push_patch(&self, patch: Patch) {
        self.push(LogMsg::JsonPatch(patch));
    }

    pub fn push_session_id(&self, session_id: String) {
        self.push(LogMsg::SessionId(session_id));
    }

    pub fn push_finished(&self) {
        self.push(LogMsg::Finished);
    }

    pub fn get_receiver(&self) -> broadcast::Receiver<LogMsg> {
        self.sender.subscribe()
    }

    pub fn get_history(&self) -> Vec<LogMsg> {
        self.inner
            .read()
            .unwrap()
            .history
            .iter()
            .map(|s| s.msg.clone())
            .collect()
    }

    pub fn raw_history_page(
        &self,
        limit: usize,
        cursor: Option<usize>,
    ) -> (Vec<LogEntrySnapshot>, bool) {
        let inner = self.inner.read().unwrap();
        let mut entries: Vec<LogEntrySnapshot> = Vec::new();

        for entry in inner.raw_entries.iter().rev() {
            if cursor.is_some_and(|cursor| entry.entry_index >= cursor) {
                continue;
            }
            entries.push(LogEntrySnapshot {
                entry_index: entry.entry_index,
                entry_json: entry.entry_json.clone(),
            });
            if entries.len() >= limit {
                break;
            }
        }

        entries.reverse();
        let has_more = entries.first().map_or(inner.raw_evicted, |first| {
            inner
                .raw_entries
                .iter()
                .any(|entry| entry.entry_index < first.entry_index)
                || inner.raw_evicted
        });

        (entries, has_more)
    }

    pub fn normalized_history_page(
        &self,
        limit: usize,
        cursor: Option<usize>,
    ) -> (Vec<LogEntrySnapshot>, bool) {
        let inner = self.inner.read().unwrap();
        let mut entries: Vec<LogEntrySnapshot> = Vec::new();

        for (index, entry) in inner.normalized_entries.iter().rev() {
            if cursor.is_some_and(|cursor| *index >= cursor) {
                continue;
            }
            entries.push(LogEntrySnapshot {
                entry_index: *index,
                entry_json: entry.entry_json.clone(),
            });
            if entries.len() >= limit {
                break;
            }
        }

        entries.reverse();
        let has_more = entries.first().map_or(inner.normalized_evicted, |first| {
            inner
                .normalized_entries
                .range(..first.entry_index)
                .next()
                .is_some()
                || inner.normalized_evicted
        });

        (entries, has_more)
    }

    pub fn raw_history_plus_stream(
        &self,
    ) -> futures::stream::BoxStream<'static, Result<LogEntryEvent, std::io::Error>> {
        let finished = self.inner.read().unwrap().finished;
        let history = self.raw_history_page(usize::MAX, None).0;

        let hist = futures::stream::iter(
            history
                .into_iter()
                .map(|entry| Ok::<_, std::io::Error>(LogEntryEvent::Append {
                    entry_index: entry.entry_index,
                    entry: entry.entry_json,
                })),
        );

        if finished {
            Box::pin(hist.chain(futures::stream::once(async {
                Ok::<_, std::io::Error>(LogEntryEvent::Finished)
            })))
        } else {
            let live = BroadcastStream::new(self.raw_sender.subscribe())
                .filter_map(|res| async move { res.ok().map(Ok::<_, std::io::Error>) });
            Box::pin(hist.chain(live))
        }
    }

    pub fn normalized_history_plus_stream(
        &self,
    ) -> futures::stream::BoxStream<'static, Result<LogEntryEvent, std::io::Error>> {
        let finished = self.inner.read().unwrap().finished;
        let history = self.normalized_history_page(usize::MAX, None).0;

        let hist = futures::stream::iter(
            history
                .into_iter()
                .map(|entry| Ok::<_, std::io::Error>(LogEntryEvent::Append {
                    entry_index: entry.entry_index,
                    entry: entry.entry_json,
                })),
        );

        if finished {
            Box::pin(hist.chain(futures::stream::once(async {
                Ok::<_, std::io::Error>(LogEntryEvent::Finished)
            })))
        } else {
            let live = BroadcastStream::new(self.normalized_sender.subscribe())
                .filter_map(|res| async move { res.ok().map(Ok::<_, std::io::Error>) });
            Box::pin(hist.chain(live))
        }
    }

    /// History then live, as `LogMsg`.
    pub fn history_plus_stream(
        &self,
    ) -> futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>> {
        let (history, rx) = (self.get_history(), self.get_receiver());

        let hist = futures::stream::iter(history.into_iter().map(Ok::<_, std::io::Error>));
        let live = BroadcastStream::new(rx)
            .filter_map(|res| async move { res.ok().map(Ok::<_, std::io::Error>) });

        Box::pin(hist.chain(live))
    }

    pub fn stdout_chunked_stream(
        &self,
    ) -> futures::stream::BoxStream<'static, Result<String, std::io::Error>> {
        self.history_plus_stream()
            .take_while(|res| future::ready(!matches!(res, Ok(LogMsg::Finished))))
            .filter_map(|res| async move {
                match res {
                    Ok(LogMsg::Stdout(s)) => Some(Ok(s)),
                    _ => None,
                }
            })
            .boxed()
    }

    pub fn stdout_lines_stream(
        &self,
    ) -> futures::stream::BoxStream<'static, std::io::Result<String>> {
        self.stdout_chunked_stream().lines()
    }

    pub fn stderr_chunked_stream(
        &self,
    ) -> futures::stream::BoxStream<'static, Result<String, std::io::Error>> {
        self.history_plus_stream()
            .take_while(|res| future::ready(!matches!(res, Ok(LogMsg::Finished))))
            .filter_map(|res| async move {
                match res {
                    Ok(LogMsg::Stderr(s)) => Some(Ok(s)),
                    _ => None,
                }
            })
            .boxed()
    }

    pub fn stderr_lines_stream(
        &self,
    ) -> futures::stream::BoxStream<'static, std::io::Result<String>> {
        self.stderr_chunked_stream().lines()
    }

    /// Same stream but mapped to `Event` for SSE handlers.
    pub fn sse_stream(&self) -> futures::stream::BoxStream<'static, Result<Event, std::io::Error>> {
        self.history_plus_stream()
            .map_ok(|m| m.to_sse_event())
            .boxed()
    }

    /// Forward a stream of typed log messages into this store.
    pub fn spawn_forwarder<S, E>(self: Arc<Self>, stream: S) -> JoinHandle<()>
    where
        S: futures::Stream<Item = Result<LogMsg, E>> + Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        tokio::spawn(async move {
            tokio::pin!(stream);

            while let Some(next) = stream.next().await {
                match next {
                    Ok(msg) => self.push(msg),
                    Err(e) => self.push(LogMsg::Stderr(format!("stream error: {e}"))),
                }
            }
        })
    }
}

impl Inner {
    fn push_msg(&mut self, msg: LogMsg, bytes: usize) {
        let limits = log_history_config();

        while self.history.len() >= limits.max_entries
            || self.total_bytes.saturating_add(bytes) > limits.max_bytes
        {
            if let Some(front) = self.history.pop_front() {
                self.total_bytes = self.total_bytes.saturating_sub(front.bytes);
            } else {
                break;
            }
        }
        self.history.push_back(StoredMsg { msg, bytes });
        self.total_bytes = self.total_bytes.saturating_add(bytes);
    }

    fn push_raw_entry(&mut self, content: String, stdout: bool) -> Option<LogEntryEvent> {
        let entry_index = self.raw_next_index;
        self.raw_next_index = self.raw_next_index.saturating_add(1);

        let entry_json = if stdout {
            serde_json::json!({ "type": "STDOUT", "content": content })
        } else {
            serde_json::json!({ "type": "STDERR", "content": content })
        };

        let bytes = approx_json_bytes(&entry_json);
        let stored = StoredEntry {
            entry_index,
            entry_json: entry_json.clone(),
            bytes,
        };

        self.raw_entries.push_back(stored);
        self.raw_total_bytes = self.raw_total_bytes.saturating_add(bytes);
        self.trim_raw_entries();

        Some(LogEntryEvent::Append {
            entry_index,
            entry: entry_json,
        })
    }

    fn upsert_normalized_entry(&mut self, update: NormalizedUpdate) -> Option<LogEntryEvent> {
        let bytes = approx_json_bytes(&update.entry_json);
        let stored = StoredEntry {
            entry_index: update.entry_index,
            entry_json: update.entry_json.clone(),
            bytes,
        };

        self.normalized_max_index = self.normalized_max_index.max(update.entry_index);

        match self.normalized_entries.insert(update.entry_index, stored) {
            Some(prev) => {
                self.normalized_total_bytes = self
                    .normalized_total_bytes
                    .saturating_sub(prev.bytes)
                    .saturating_add(bytes);
            }
            None => {
                self.normalized_total_bytes = self.normalized_total_bytes.saturating_add(bytes);
            }
        }

        self.trim_normalized_entries();

        Some(match update.op {
            UpdateOp::Append => LogEntryEvent::Append {
                entry_index: update.entry_index,
                entry: update.entry_json,
            },
            UpdateOp::Replace => LogEntryEvent::Replace {
                entry_index: update.entry_index,
                entry: update.entry_json,
            },
        })
    }

    fn trim_raw_entries(&mut self) {
        let limits = log_history_config();

        while self.raw_entries.len() > limits.max_entries
            || self.raw_total_bytes > limits.max_bytes
        {
            if let Some(front) = self.raw_entries.pop_front() {
                self.raw_total_bytes = self.raw_total_bytes.saturating_sub(front.bytes);
                self.raw_evicted = true;
            } else {
                break;
            }
        }
    }

    fn trim_normalized_entries(&mut self) {
        let limits = log_history_config();

        while self.normalized_entries.len() > limits.max_entries
            || self.normalized_total_bytes > limits.max_bytes
        {
            if let Some((&key, _)) = self.normalized_entries.iter().next() {
                if let Some(removed) = self.normalized_entries.remove(&key) {
                    self.normalized_total_bytes = self
                        .normalized_total_bytes
                        .saturating_sub(removed.bytes);
                    self.normalized_evicted = true;
                }
            } else {
                break;
            }
        }
    }
}

#[derive(Clone)]
struct NormalizedUpdate {
    entry_index: usize,
    entry_json: Value,
    op: UpdateOp,
}

#[derive(Clone, Copy)]
enum UpdateOp {
    Append,
    Replace,
}

fn extract_normalized_updates(patch: &Patch) -> Vec<NormalizedUpdate> {
    patch
        .iter()
        .filter_map(|op| match op {
            PatchOperation::Add(add) => {
                normalize_patch_entry(&add.path, &add.value).map(|entry_json| {
                    NormalizedUpdate {
                        entry_index: entry_json.entry_index,
                        entry_json: entry_json.entry_json,
                        op: UpdateOp::Append,
                    }
                })
            }
            PatchOperation::Replace(replace) => {
                normalize_patch_entry(&replace.path, &replace.value).map(|entry_json| {
                    NormalizedUpdate {
                        entry_index: entry_json.entry_index,
                        entry_json: entry_json.entry_json,
                        op: UpdateOp::Replace,
                    }
                })
            }
            _ => None,
        })
        .collect()
}

fn normalize_patch_entry(path: &str, value: &Value) -> Option<NormalizedEntryJson> {
    let index = path.strip_prefix("/entries/")?.parse::<usize>().ok()?;

    let entry_type = value.get("type")?.as_str()?;
    if entry_type != "NORMALIZED_ENTRY" {
        return None;
    }

    Some(NormalizedEntryJson {
        entry_index: index,
        entry_json: value.clone(),
    })
}

fn approx_json_bytes(value: &Value) -> usize {
    serde_json::to_string(value).map(|s| s.len()).unwrap_or(2)
}

struct NormalizedEntryJson {
    entry_index: usize,
    entry_json: Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_history_assigns_entry_indexes() {
        let store = MsgStore::new();
        store.push_stdout("hello");
        store.push_stderr("oops");

        let (entries, has_more) = store.raw_history_page(10, None);
        assert!(!has_more);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].entry_index, 0);
        assert_eq!(entries[1].entry_index, 1);
        assert_eq!(entries[0].entry_json["type"], "STDOUT");
        assert_eq!(entries[1].entry_json["type"], "STDERR");
    }

    #[test]
    fn normalized_replace_updates_entry() {
        let store = MsgStore::new();

        let add_patch: Patch = serde_json::from_value(serde_json::json!([{
            "op": "add",
            "path": "/entries/0",
            "value": {
                "type": "NORMALIZED_ENTRY",
                "content": {
                    "entry_type": { "type": "assistant_message" },
                    "content": "initial",
                    "metadata": null,
                    "timestamp": null
                }
            }
        }]))
        .expect("valid add patch");

        let replace_patch: Patch = serde_json::from_value(serde_json::json!([{
            "op": "replace",
            "path": "/entries/0",
            "value": {
                "type": "NORMALIZED_ENTRY",
                "content": {
                    "entry_type": { "type": "assistant_message" },
                    "content": "updated",
                    "metadata": null,
                    "timestamp": null
                }
            }
        }]))
        .expect("valid replace patch");

        store.push_patch(add_patch);
        store.push_patch(replace_patch);

        let (entries, _) = store.normalized_history_page(10, None);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].entry_index, 0);
        assert_eq!(entries[0].entry_json["content"]["content"], "updated");
    }
}
