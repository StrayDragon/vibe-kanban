//! Minimal JSON-RPC helper tailored for the Codex executor.
//!
//! We keep this bespoke layer because the codex-app-server client must handle server-initiated
//! requests as well as client-initiated requests. When a bidirectional client that
//! supports this pattern is available, this module should be straightforward to
//! replace.

use std::{
    collections::HashMap,
    fmt::Debug,
    io,
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
};

use async_trait::async_trait;
use codex_app_server_protocol::{
    JSONRPCError, JSONRPCMessage, JSONRPCNotification, JSONRPCRequest, JSONRPCResponse, RequestId,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{ChildStdin, ChildStdout},
    sync::{Mutex, oneshot},
};

use crate::executors::{ExecutorError, ExecutorExitResult};

#[derive(Debug)]
pub enum PendingResponse {
    Result(Value),
    Error(JSONRPCError),
    Shutdown,
}

#[derive(Clone)]
pub struct ExitSignalSender {
    inner: Arc<Mutex<Option<oneshot::Sender<ExecutorExitResult>>>>,
}

impl ExitSignalSender {
    pub fn new(sender: oneshot::Sender<ExecutorExitResult>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Some(sender))),
        }
    }

    pub async fn send_exit_signal(&self, result: ExecutorExitResult) {
        if let Some(sender) = self.inner.lock().await.take() {
            let _ = sender.send(result);
        }
    }
}

#[derive(Clone)]
pub struct JsonRpcPeer {
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Arc<Mutex<HashMap<RequestId, oneshot::Sender<PendingResponse>>>>,
    id_counter: Arc<AtomicI64>,
}

impl JsonRpcPeer {
    pub fn spawn(
        stdin: ChildStdin,
        stdout: ChildStdout,
        callbacks: Arc<dyn JsonRpcCallbacks>,
        exit_tx: ExitSignalSender,
    ) -> Self {
        let peer = Self {
            stdin: Arc::new(Mutex::new(stdin)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            id_counter: Arc::new(AtomicI64::new(1)),
        };

        let reader_peer = peer.clone();
        let callbacks = callbacks.clone();

        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut buffer = String::new();

            loop {
                buffer.clear();
                match reader.read_line(&mut buffer).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let line = buffer.trim_end_matches(['\n', '\r']);
                        if line.is_empty() {
                            continue;
                        }

                        match serde_json::from_str::<JSONRPCMessage>(line) {
                            Ok(JSONRPCMessage::Response(response)) => {
                                let request_id = response.id.clone();
                                let result = response.result.clone();
                                if callbacks
                                    .on_response(&reader_peer, line, &response)
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                                reader_peer
                                    .resolve(request_id, PendingResponse::Result(result))
                                    .await;
                            }
                            Ok(JSONRPCMessage::Error(error)) => {
                                let request_id = error.id.clone();
                                if callbacks
                                    .on_error(&reader_peer, line, &error)
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                                reader_peer
                                    .resolve(request_id, PendingResponse::Error(error))
                                    .await;
                            }
                            Ok(JSONRPCMessage::Request(request)) => {
                                if callbacks
                                    .on_request(&reader_peer, line, request)
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Ok(JSONRPCMessage::Notification(notification)) => {
                                match callbacks
                                    .on_notification(&reader_peer, line, notification)
                                    .await
                                {
                                    // finished
                                    Ok(true) => break,
                                    Ok(false) => {}
                                    Err(_) => {
                                        break;
                                    }
                                }
                            }
                            Err(_) => {
                                if callbacks.on_non_json(line).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(err) => {
                        tracing::warn!("Error reading Codex output: {err}");
                        break;
                    }
                }
            }

            exit_tx.send_exit_signal(ExecutorExitResult::Success).await;
            let _ = reader_peer.shutdown().await;
        });

        peer
    }

    pub fn next_request_id(&self) -> RequestId {
        RequestId::Integer(self.id_counter.fetch_add(1, Ordering::Relaxed))
    }

    pub async fn register(&self, request_id: RequestId) -> PendingReceiver {
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(request_id, sender);
        receiver
    }

    pub async fn resolve(&self, request_id: RequestId, response: PendingResponse) {
        if let Some(sender) = self.pending.lock().await.remove(&request_id) {
            let _ = sender.send(response);
        }
    }

    pub async fn shutdown(&self) -> Result<(), ExecutorError> {
        let mut pending = self.pending.lock().await;
        for (_, sender) in pending.drain() {
            let _ = sender.send(PendingResponse::Shutdown);
        }
        Ok(())
    }

    pub async fn send<T>(&self, message: &T) -> Result<(), ExecutorError>
    where
        T: Serialize + Sync,
    {
        let raw = serde_json::to_string(message)
            .map_err(|err| ExecutorError::Io(io::Error::other(err.to_string())))?;
        self.send_raw(&raw).await
    }

    pub async fn request<R, T>(
        &self,
        request_id: RequestId,
        message: &T,
        label: &str,
    ) -> Result<R, ExecutorError>
    where
        R: DeserializeOwned + Debug,
        T: Serialize + Sync,
    {
        let receiver = self.register(request_id).await;
        self.send(message).await?;
        await_response(receiver, label).await
    }

    async fn send_raw(&self, payload: &str) -> Result<(), ExecutorError> {
        let mut guard = self.stdin.lock().await;
        guard
            .write_all(payload.as_bytes())
            .await
            .map_err(ExecutorError::Io)?;
        guard.write_all(b"\n").await.map_err(ExecutorError::Io)?;
        guard.flush().await.map_err(ExecutorError::Io)?;
        Ok(())
    }
}

pub type PendingReceiver = oneshot::Receiver<PendingResponse>;

pub async fn await_response<R>(receiver: PendingReceiver, label: &str) -> Result<R, ExecutorError>
where
    R: DeserializeOwned + Debug,
{
    match receiver.await {
        Ok(PendingResponse::Result(value)) => serde_json::from_value(value).map_err(|err| {
            ExecutorError::Io(io::Error::other(format!(
                "failed to decode {label} response: {err}",
            )))
        }),
        Ok(PendingResponse::Error(error)) => Err(ExecutorError::Io(io::Error::other(format!(
            "{label} request failed: {}",
            error.error.message
        )))),
        Ok(PendingResponse::Shutdown) => Err(ExecutorError::Io(io::Error::other(format!(
            "server was shutdown while waiting for {label} response",
        )))),
        Err(_) => Err(ExecutorError::Io(io::Error::other(format!(
            "{label} request was dropped",
        )))),
    }
}

#[async_trait]
pub trait JsonRpcCallbacks: Send + Sync {
    async fn on_request(
        &self,
        peer: &JsonRpcPeer,
        raw: &str,
        request: JSONRPCRequest,
    ) -> Result<(), ExecutorError>;

    async fn on_response(
        &self,
        peer: &JsonRpcPeer,
        raw: &str,
        response: &JSONRPCResponse,
    ) -> Result<(), ExecutorError>;

    async fn on_error(
        &self,
        peer: &JsonRpcPeer,
        raw: &str,
        error: &JSONRPCError,
    ) -> Result<(), ExecutorError>;

    async fn on_notification(
        &self,
        peer: &JsonRpcPeer,
        raw: &str,
        notification: JSONRPCNotification,
    ) -> Result<bool, ExecutorError>;

    async fn on_non_json(&self, _raw: &str) -> Result<(), ExecutorError>;
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::{process::Stdio, sync::Arc};

    use tokio::{
        process::Command,
        sync::{Mutex, oneshot},
        time::{Duration, timeout},
    };

    use super::*;

    #[derive(Default)]
    struct CallbackState {
        non_json: Vec<String>,
        notifications: Vec<String>,
    }

    #[derive(Clone)]
    struct RecordingCallbacks {
        state: Arc<Mutex<CallbackState>>,
        stop_method: Option<String>,
    }

    impl RecordingCallbacks {
        fn new(stop_method: Option<&str>) -> (Self, Arc<Mutex<CallbackState>>) {
            let state = Arc::new(Mutex::new(CallbackState::default()));
            (
                Self {
                    state: state.clone(),
                    stop_method: stop_method.map(str::to_string),
                },
                state,
            )
        }
    }

    #[async_trait]
    impl JsonRpcCallbacks for RecordingCallbacks {
        async fn on_request(
            &self,
            _peer: &JsonRpcPeer,
            _raw: &str,
            _request: JSONRPCRequest,
        ) -> Result<(), ExecutorError> {
            Ok(())
        }

        async fn on_response(
            &self,
            _peer: &JsonRpcPeer,
            _raw: &str,
            _response: &JSONRPCResponse,
        ) -> Result<(), ExecutorError> {
            Ok(())
        }

        async fn on_error(
            &self,
            _peer: &JsonRpcPeer,
            _raw: &str,
            _error: &JSONRPCError,
        ) -> Result<(), ExecutorError> {
            Ok(())
        }

        async fn on_notification(
            &self,
            _peer: &JsonRpcPeer,
            _raw: &str,
            notification: JSONRPCNotification,
        ) -> Result<bool, ExecutorError> {
            let method = notification.method.clone();
            let stop = self.stop_method.as_deref() == Some(method.as_str());
            self.state.lock().await.notifications.push(method);
            Ok(stop)
        }

        async fn on_non_json(&self, raw: &str) -> Result<(), ExecutorError> {
            self.state.lock().await.non_json.push(raw.to_string());
            Ok(())
        }
    }

    async fn run_script(script: &str, callbacks: Arc<dyn JsonRpcCallbacks>) -> ExecutorExitResult {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn fake server");
        let stdout = child.stdout.take().expect("stdout");
        let stdin = child.stdin.take().expect("stdin");
        let (exit_tx, exit_rx) = oneshot::channel();

        let _peer = JsonRpcPeer::spawn(stdin, stdout, callbacks, ExitSignalSender::new(exit_tx));
        tokio::spawn(async move {
            let _ = child.wait().await;
        });

        timeout(Duration::from_secs(2), exit_rx)
            .await
            .expect("exit timeout")
            .expect("exit result")
    }

    #[tokio::test]
    async fn jsonrpc_peer_reports_non_json_and_continues() {
        let (callbacks, state) = RecordingCallbacks::new(None);
        let script = r#"printf '%s\n' 'not json' '{"jsonrpc":"2.0","method":"codex/event/unknown","params":{}}'"#;
        let exit = run_script(script, Arc::new(callbacks)).await;

        assert!(matches!(exit, ExecutorExitResult::Success));

        let state = state.lock().await;
        assert_eq!(state.non_json, vec!["not json"]);
        assert_eq!(state.notifications, vec!["codex/event/unknown"]);
    }

    #[tokio::test]
    async fn jsonrpc_peer_continues_after_unknown_notification() {
        let (callbacks, state) = RecordingCallbacks::new(Some("stop"));
        let script = r#"printf '%s\n' '{"jsonrpc":"2.0","method":"codex/event/unknown","params":{}}' '{"jsonrpc":"2.0","method":"stop","params":{}}' '{"jsonrpc":"2.0","method":"codex/event/after","params":{}}'"#;
        run_script(script, Arc::new(callbacks)).await;

        let state = state.lock().await;
        assert!(state.non_json.is_empty());
        assert_eq!(state.notifications, vec!["codex/event/unknown", "stop"]);
    }
}

#[cfg(test)]
mod await_response_tests {
    use codex_app_server_protocol::JSONRPCErrorError;
    use tokio::sync::oneshot;

    use super::*;

    #[tokio::test]
    async fn await_response_returns_error_for_error_response() {
        let (sender, receiver) = oneshot::channel();
        let error = JSONRPCError {
            id: RequestId::Integer(1),
            error: JSONRPCErrorError {
                code: -1,
                data: None,
                message: "nope".to_string(),
            },
        };
        sender.send(PendingResponse::Error(error)).expect("send");

        let err = await_response::<serde_json::Value>(receiver, "ping")
            .await
            .expect_err("error response");
        assert!(err.to_string().contains("ping request failed: nope"));
    }

    #[tokio::test]
    async fn await_response_returns_error_on_shutdown() {
        let (sender, receiver) = oneshot::channel();
        sender
            .send(PendingResponse::Shutdown)
            .expect("send shutdown");

        let err = await_response::<serde_json::Value>(receiver, "ping")
            .await
            .expect_err("shutdown");
        assert!(
            err.to_string()
                .contains("server was shutdown while waiting for ping response")
        );
    }

    #[tokio::test]
    async fn await_response_returns_error_when_channel_dropped() {
        let (sender, receiver) = oneshot::channel::<PendingResponse>();
        drop(sender);

        let err = await_response::<serde_json::Value>(receiver, "ping")
            .await
            .expect_err("dropped");
        assert!(err.to_string().contains("ping request was dropped"));
    }
}
