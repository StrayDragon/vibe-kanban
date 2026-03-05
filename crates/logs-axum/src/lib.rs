use axum::{extract::ws::Message, response::sse::Event};
use futures::{StreamExt, TryStreamExt};
use logs_protocol::{
    LogMsg,
    log_msg::{EV_FINISHED, EV_JSON_PATCH, EV_SESSION_ID, EV_STDERR, EV_STDOUT},
};
use logs_store::MsgStore;

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

pub trait MsgStoreAxumExt {
    fn sse_stream(&self) -> futures::stream::BoxStream<'static, Result<Event, std::io::Error>>;
}

impl MsgStoreAxumExt for MsgStore {
    fn sse_stream(&self) -> futures::stream::BoxStream<'static, Result<Event, std::io::Error>> {
        self.history_plus_stream()
            .map_ok(|m| m.to_sse_event())
            .boxed()
    }
}
