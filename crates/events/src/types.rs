use anyhow::Error as AnyhowError;
use db::{DbErr, models::scratch::ScratchError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EventError {
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error(transparent)]
    Parse(#[from] serde_json::Error),
    #[error(transparent)]
    Scratch(#[from] ScratchError),
    #[error(transparent)]
    Other(#[from] AnyhowError),
}
