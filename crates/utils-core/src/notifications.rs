use std::sync::Arc;

use async_trait::async_trait;

#[async_trait]
pub trait Notifier: Send + Sync {
    async fn notify(&self, title: &str, message: &str);
}

#[derive(Debug, Default)]
pub struct NoopNotifier;

#[async_trait]
impl Notifier for NoopNotifier {
    async fn notify(&self, _title: &str, _message: &str) {}
}

pub type SharedNotifier = Arc<dyn Notifier>;
