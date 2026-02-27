use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use db::models::scratch::DraftFollowUpData;
use serde::{Deserialize, Serialize};
use thiserror::Error;
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
    idempotency: Arc<DashMap<Uuid, QueueIdempotencyRecord>>,
    ttl: Duration,
}

#[derive(Debug, Clone)]
struct QueueIdempotencyRecord {
    key: String,
    request_hash: String,
}

#[derive(Debug, Error)]
pub enum QueueMessageIdempotencyError {
    #[error("Idempotency key already used with different message payload")]
    Conflict,
}

impl QueuedMessageService {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(DashMap::new()),
            idempotency: Arc::new(DashMap::new()),
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
            self.idempotency.remove(key);
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
                self.idempotency.remove(session_id);
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
        self.idempotency.remove(&session_id);
        let queued = QueuedMessage {
            session_id,
            data,
            queued_at: Utc::now(),
        };
        self.queue.insert(session_id, queued.clone());
        queued
    }

    /// Queue a message for a session, using an idempotency key for safe retries.
    /// If the same idempotency key is reused with the same payload, this returns the existing
    /// queued message without modifying timestamps. If the key is reused with a different payload,
    /// this returns a Conflict error.
    pub fn queue_message_idempotent(
        &self,
        session_id: Uuid,
        idempotency_key: String,
        request_hash: String,
        data: DraftFollowUpData,
    ) -> Result<QueuedMessage, QueueMessageIdempotencyError> {
        self.prune_expired();

        if let Some(entry) = self.idempotency.get(&session_id) {
            // If the same key is reused, ensure the payload matches and return the existing message.
            if entry.key == idempotency_key {
                if entry.request_hash != request_hash {
                    return Err(QueueMessageIdempotencyError::Conflict);
                }
                if let Some(existing) = self.queue.get(&session_id) {
                    return Ok(existing.clone());
                }
            }
        }

        let queued_at = Utc::now();
        let queued = QueuedMessage {
            session_id,
            data,
            queued_at,
        };
        self.queue.insert(session_id, queued.clone());
        self.idempotency.insert(
            session_id,
            QueueIdempotencyRecord {
                key: idempotency_key,
                request_hash,
            },
        );
        Ok(queued)
    }

    /// Cancel/remove a queued message for a session
    pub fn cancel_queued(&self, session_id: Uuid) -> Option<QueuedMessage> {
        self.idempotency.remove(&session_id);
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
        self.idempotency.remove(&session_id);
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

    #[test]
    fn queue_message_idempotent_reuses_existing_message() {
        let service = QueuedMessageService::new();
        let session_id = Uuid::new_v4();
        let data = DraftFollowUpData {
            message: "hello".to_string(),
            variant: None,
        };

        let queued1 = service
            .queue_message_idempotent(
                session_id,
                "req-1".to_string(),
                "hash-1".to_string(),
                data.clone(),
            )
            .unwrap();

        let queued2 = service
            .queue_message_idempotent(
                session_id,
                "req-1".to_string(),
                "hash-1".to_string(),
                data.clone(),
            )
            .unwrap();

        assert_eq!(queued1.queued_at, queued2.queued_at);
        assert_eq!(queued1.data.message, queued2.data.message);
    }

    #[test]
    fn queue_message_idempotent_conflicts_on_payload_change() {
        let service = QueuedMessageService::new();
        let session_id = Uuid::new_v4();
        let data1 = DraftFollowUpData {
            message: "hello".to_string(),
            variant: None,
        };
        let data2 = DraftFollowUpData {
            message: "different".to_string(),
            variant: None,
        };

        let _ = service
            .queue_message_idempotent(session_id, "req-1".to_string(), "hash-1".to_string(), data1)
            .unwrap();

        let err = service
            .queue_message_idempotent(session_id, "req-1".to_string(), "hash-2".to_string(), data2)
            .expect_err("expected conflict");

        assert!(matches!(err, QueueMessageIdempotencyError::Conflict));
    }
}
