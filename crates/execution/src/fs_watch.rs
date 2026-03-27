use std::path::{Path, PathBuf};

use notify::Watcher;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct FsWatchEvent {
    pub paths: Vec<PathBuf>,
    pub is_access: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum FsWatchError {
    #[error("watch init failed: {0}")]
    Init(String),
    #[error("watch event error: {0}")]
    Event(String),
}

pub struct FsWatcher {
    _watcher: notify::RecommendedWatcher,
}

pub fn recommended_recursive_watcher(
    dir: &Path,
) -> Result<
    (
        FsWatcher,
        mpsc::UnboundedReceiver<Result<FsWatchEvent, FsWatchError>>,
    ),
    FsWatchError,
> {
    let (tx, rx) = mpsc::unbounded_channel();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let msg = match res {
            Ok(event) => Ok(FsWatchEvent {
                paths: event.paths,
                is_access: event.kind.is_access(),
            }),
            Err(err) => Err(FsWatchError::Event(err.to_string())),
        };
        let _ = tx.send(msg);
    })
    .map_err(|err| FsWatchError::Init(err.to_string()))?;

    // Watch the directory instead of individual files to handle atomic writes via rename.
    // Use recursive mode so newly-created subdirectories are picked up without a restart.
    watcher
        .watch(dir, notify::RecursiveMode::Recursive)
        .map_err(|err| FsWatchError::Init(err.to_string()))?;

    Ok((FsWatcher { _watcher: watcher }, rx))
}
