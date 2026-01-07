use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use db::models::scratch::DraftFollowUpData;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::services::cache_budget::{cache_budgets, should_warn};

/// Represents a queued follow-up message for a session
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct QueuedMessage {
    /// The session this message is queued for
    pub session_id: Uuid,
    /// The follow-up data (message + variant)
    pub data: DraftFollowUpData,
    /// Timestamp when the message was queued
    pub queued_at: DateTime<Utc>,
}

/// Status of the queue for a session (for frontend display)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
#[ts(export)]
pub enum QueueStatus {
    /// No message queued
    Empty,
    /// Message is queued and waiting for execution to complete
    Queued { message: QueuedMessage },
}

/// In-memory service for managing queued follow-up messages.
/// One queued message per session.
#[derive(Clone)]
pub struct QueuedMessageService {
    queue: Arc<DashMap<Uuid, QueuedMessage>>,
    ttl: Duration,
}

impl QueuedMessageService {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(DashMap::new()),
            ttl: cache_budgets().queued_messages_ttl,
        }
    }

    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    fn is_expired(&self, queued_at: DateTime<Utc>) -> bool {
        if self.ttl.is_zero() {
            return false;
        }

        let ttl = chrono::Duration::from_std(self.ttl).unwrap_or_else(|_| chrono::Duration::zero());
        Utc::now() - queued_at > ttl
    }

    fn prune_expired(&self) -> usize {
        if self.ttl.is_zero() {
            return 0;
        }

        let mut expired = Vec::new();
        for entry in self.queue.iter() {
            if self.is_expired(entry.value().queued_at) {
                expired.push(*entry.key());
            }
        }

        for key in &expired {
            self.queue.remove(key);
        }

        if !expired.is_empty() && should_warn("queued_messages") {
            tracing::warn!(
                "Removed {} expired queued messages (ttl={}s)",
                expired.len(),
                self.ttl.as_secs()
            );
        }

        expired.len()
    }

    fn prune_if_expired(&self, session_id: &Uuid) -> bool {
        if let Some(entry) = self.queue.get(session_id) {
            let expired = self.is_expired(entry.queued_at);
            drop(entry);
            if expired {
                self.queue.remove(session_id);
                if should_warn("queued_messages") {
                    tracing::warn!(
                        "Queued message expired for session {session_id} (ttl={}s)",
                        self.ttl.as_secs()
                    );
                }
                return true;
            }
        }
        false
    }

    /// Queue a message for a session. Replaces any existing queued message.
    pub fn queue_message(&self, session_id: Uuid, data: DraftFollowUpData) -> QueuedMessage {
        self.prune_expired();
        let queued = QueuedMessage {
            session_id,
            data,
            queued_at: Utc::now(),
        };
        self.queue.insert(session_id, queued.clone());
        queued
    }

    /// Cancel/remove a queued message for a session
    pub fn cancel_queued(&self, session_id: Uuid) -> Option<QueuedMessage> {
        self.queue.remove(&session_id).map(|(_, v)| v)
    }

    /// Get the queued message for a session (if any)
    pub fn get_queued(&self, session_id: Uuid) -> Option<QueuedMessage> {
        if self.prune_if_expired(&session_id) {
            return None;
        }
        self.queue.get(&session_id).map(|r| r.clone())
    }

    /// Take (remove and return) the queued message for a session.
    /// Used by finalization flow to consume the queued message.
    pub fn take_queued(&self, session_id: Uuid) -> Option<QueuedMessage> {
        if self.prune_if_expired(&session_id) {
            return None;
        }
        self.queue.remove(&session_id).map(|(_, v)| v)
    }

    /// Check if a session has a queued message
    pub fn has_queued(&self, session_id: Uuid) -> bool {
        if self.prune_if_expired(&session_id) {
            return false;
        }
        self.queue.contains_key(&session_id)
    }

    /// Get queue status for frontend display
    pub fn get_status(&self, session_id: Uuid) -> QueueStatus {
        if self.prune_if_expired(&session_id) {
            return QueueStatus::Empty;
        }
        match self.get_queued(session_id) {
            Some(msg) => QueueStatus::Queued { message: msg },
            None => QueueStatus::Empty,
        }
    }
}

impl Default for QueuedMessageService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration as ChronoDuration, Utc};
    use uuid::Uuid;

    use super::*;

    #[test]
    fn queued_message_expires_on_access() {
        let service = QueuedMessageService::new();
        if service.ttl.is_zero() {
            return;
        }

        let session_id = Uuid::new_v4();
        let data = DraftFollowUpData {
            message: "hello".to_string(),
            variant: None,
        };
        service.queue_message(session_id, data);

        if let Some(mut entry) = service.queue.get_mut(&session_id) {
            entry.queued_at =
                Utc::now() - ChronoDuration::seconds((service.ttl.as_secs() + 1) as i64);
        }

        assert!(service.get_queued(session_id).is_none());
        assert!(!service.queue.contains_key(&session_id));
    }
}
