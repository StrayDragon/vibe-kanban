use axum::{
    Json,
    extract::multipart::MultipartError,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use db::{
    DbErr,
    models::{
        execution_process::ExecutionProcessError, project::ProjectError,
        project_repo::ProjectRepoError, repo::RepoError, scratch::ScratchError,
        session::SessionError, workspace::WorkspaceError,
    },
};
use deployment::DeploymentError;
use executors::executors::ExecutorError;
use git2::Error as Git2Error;
use services::services::{
    config::{ConfigError, EditorOpenError},
    container::ContainerError,
    git::GitServiceError,
    github::GitHubServiceError,
    image::ImageError,
    project::ProjectServiceError,
    repo::RepoError as RepoServiceError,
    worktree_manager::WorktreeError,
};
use thiserror::Error;
use utils::response::ApiResponse;

#[derive(Debug, Error, ts_rs::TS)]
#[ts(type = "string")]
pub enum ApiError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Repo(#[from] RepoError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    ScratchError(#[from] ScratchError),
    #[error(transparent)]
    ExecutionProcess(#[from] ExecutionProcessError),
    #[error(transparent)]
    GitService(#[from] GitServiceError),
    #[error(transparent)]
    GitHubService(#[from] GitHubServiceError),
    #[error(transparent)]
    Deployment(#[from] DeploymentError),
    #[error(transparent)]
    Container(#[from] ContainerError),
    #[error(transparent)]
    Executor(#[from] ExecutorError),
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error(transparent)]
    Worktree(#[from] WorktreeError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Image(#[from] ImageError),
    #[error("Multipart error: {0}")]
    Multipart(#[from] MultipartError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    EditorOpen(#[from] EditorOpenError),
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Internal server error: {0}")]
    Internal(String),
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
}

impl From<&'static str> for ApiError {
    fn from(msg: &'static str) -> Self {
        ApiError::BadRequest(msg.to_string())
    }
}

impl From<Git2Error> for ApiError {
    fn from(err: Git2Error) -> Self {
        ApiError::GitService(GitServiceError::from(err))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status_code, error_type) = match &self {
            ApiError::Project(err) => match err {
                ProjectError::ProjectNotFound => (StatusCode::NOT_FOUND, "ProjectError"),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "ProjectError"),
            },
            ApiError::Repo(err) => match err {
                RepoError::NotFound => (StatusCode::NOT_FOUND, "RepoError"),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "RepoError"),
            },
            ApiError::Workspace(err) => match err {
                WorkspaceError::TaskNotFound | WorkspaceError::ProjectNotFound => {
                    (StatusCode::NOT_FOUND, "WorkspaceError")
                }
                WorkspaceError::BranchNotFound(_) => (StatusCode::NOT_FOUND, "WorkspaceError"),
                WorkspaceError::ValidationError(_) => (StatusCode::BAD_REQUEST, "WorkspaceError"),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "WorkspaceError"),
            },
            ApiError::Session(err) => match err {
                SessionError::NotFound | SessionError::WorkspaceNotFound => {
                    (StatusCode::NOT_FOUND, "SessionError")
                }
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "SessionError"),
            },
            ApiError::ScratchError(err) => match err {
                ScratchError::TypeMismatch { .. } => (StatusCode::BAD_REQUEST, "ScratchError"),
                ScratchError::Serde(_) => (StatusCode::BAD_REQUEST, "ScratchError"),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "ScratchError"),
            },
            ApiError::ExecutionProcess(err) => match err {
                ExecutionProcessError::ExecutionProcessNotFound => {
                    (StatusCode::NOT_FOUND, "ExecutionProcessError")
                }
                ExecutionProcessError::InvalidExecutorAction => {
                    (StatusCode::BAD_REQUEST, "ExecutionProcessError")
                }
                ExecutionProcessError::ValidationError(_) => {
                    (StatusCode::BAD_REQUEST, "ExecutionProcessError")
                }
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "ExecutionProcessError"),
            },
            // Promote certain GitService errors to conflict status with concise messages
            ApiError::GitService(git_err) => match git_err {
                services::services::git::GitServiceError::MergeConflicts(_) => {
                    (StatusCode::CONFLICT, "GitServiceError")
                }
                services::services::git::GitServiceError::RebaseInProgress => {
                    (StatusCode::CONFLICT, "GitServiceError")
                }
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "GitServiceError"),
            },
            ApiError::GitHubService(_) => (StatusCode::INTERNAL_SERVER_ERROR, "GitHubServiceError"),
            ApiError::Deployment(_) => (StatusCode::INTERNAL_SERVER_ERROR, "DeploymentError"),
            ApiError::Container(_) => (StatusCode::INTERNAL_SERVER_ERROR, "ContainerError"),
            ApiError::Executor(_) => (StatusCode::INTERNAL_SERVER_ERROR, "ExecutorError"),
            ApiError::Database(db_err) => match db_err {
                DbErr::RecordNotFound(_) => (StatusCode::NOT_FOUND, "DatabaseError"),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "DatabaseError"),
            },
            ApiError::Worktree(_) => (StatusCode::INTERNAL_SERVER_ERROR, "WorktreeError"),
            ApiError::Config(err) => match err {
                ConfigError::ValidationError(_) => (StatusCode::BAD_REQUEST, "ConfigError"),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "ConfigError"),
            },
            ApiError::Image(img_err) => match img_err {
                ImageError::InvalidFormat => (StatusCode::BAD_REQUEST, "InvalidImageFormat"),
                ImageError::TooLarge(_, _) => (StatusCode::PAYLOAD_TOO_LARGE, "ImageTooLarge"),
                ImageError::NotFound => (StatusCode::NOT_FOUND, "ImageNotFound"),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "ImageError"),
            },
            ApiError::Io(_) => (StatusCode::INTERNAL_SERVER_ERROR, "IoError"),
            ApiError::EditorOpen(err) => match err {
                EditorOpenError::LaunchFailed { .. } => {
                    (StatusCode::INTERNAL_SERVER_ERROR, "EditorLaunchError")
                }
                _ => (StatusCode::BAD_REQUEST, "EditorOpenError"),
            },
            ApiError::Multipart(_) => (StatusCode::BAD_REQUEST, "MultipartError"),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, "NotFound"),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "InternalError"),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "BadRequest"),
            ApiError::Conflict(_) => (StatusCode::CONFLICT, "ConflictError"),
            ApiError::Forbidden(_) => (StatusCode::FORBIDDEN, "ForbiddenError"),
        };

        let error_message = match &self {
            ApiError::Image(img_err) => match img_err {
                ImageError::InvalidFormat => "This file type is not supported. Please upload an image file (PNG, JPG, GIF, WebP, or BMP).".to_string(),
                ImageError::TooLarge(size, max) => format!(
                    "This image is too large ({:.1} MB). Maximum file size is {:.1} MB.",
                    *size as f64 / 1_048_576.0,
                    *max as f64 / 1_048_576.0
                ),
                ImageError::NotFound => "Image not found.".to_string(),
                _ => {
                    "Failed to process image. Please try again.".to_string()
                }
            },
            ApiError::GitService(git_err) => match git_err {
                services::services::git::GitServiceError::MergeConflicts(msg) => msg.clone(),
                services::services::git::GitServiceError::RebaseInProgress => {
                    "A rebase is already in progress. Resolve conflicts or abort the rebase, then retry.".to_string()
                }
                _ => format!("{}: {}", error_type, self),
            },
            ApiError::Multipart(_) => "Failed to upload file. Please ensure the file is valid and try again.".to_string(),
            ApiError::Unauthorized => "Unauthorized. Please sign in again.".to_string(),
            ApiError::NotFound(msg) => msg.clone(),
            ApiError::Internal(msg) => msg.clone(),
            ApiError::BadRequest(msg) => msg.clone(),
            ApiError::Conflict(msg) => msg.clone(),
            ApiError::Forbidden(msg) => msg.clone(),
            _ => format!("{}: {}", error_type, self),
        };

        if status_code.is_server_error() {
            tracing::error!(
                status = %status_code,
                error_type,
                error = %self,
                "API request failed"
            );
        }
        let response = ApiResponse::<()>::error(&error_message);
        (status_code, Json(response)).into_response()
    }
}

impl From<ProjectServiceError> for ApiError {
    fn from(err: ProjectServiceError) -> Self {
        match err {
            ProjectServiceError::Database(db_err) => ApiError::Database(db_err),
            ProjectServiceError::Io(io_err) => ApiError::Io(io_err),
            ProjectServiceError::Project(proj_err) => ApiError::Project(proj_err),
            ProjectServiceError::PathNotFound(path) => {
                ApiError::BadRequest(format!("Path does not exist: {}", path.display()))
            }
            ProjectServiceError::PathNotDirectory(path) => {
                ApiError::BadRequest(format!("Path is not a directory: {}", path.display()))
            }
            ProjectServiceError::NotGitRepository(path) => {
                ApiError::BadRequest(format!("Path is not a git repository: {}", path.display()))
            }
            ProjectServiceError::DuplicateGitRepoPath => ApiError::Conflict(
                "A project with this git repository path already exists".to_string(),
            ),
            ProjectServiceError::DuplicateRepositoryName => ApiError::Conflict(
                "A repository with this name already exists in the project".to_string(),
            ),
            ProjectServiceError::RepositoryNotFound => {
                ApiError::BadRequest("Repository not found".to_string())
            }
            ProjectServiceError::GitError(msg) => {
                ApiError::BadRequest(format!("Git operation failed: {}", msg))
            }
        }
    }
}

impl From<RepoServiceError> for ApiError {
    fn from(err: RepoServiceError) -> Self {
        match err {
            RepoServiceError::Database(db_err) => ApiError::Database(db_err),
            RepoServiceError::Io(io_err) => ApiError::Io(io_err),
            RepoServiceError::PathNotFound(path) => {
                ApiError::BadRequest(format!("Path does not exist: {}", path.display()))
            }
            RepoServiceError::PathNotDirectory(path) => {
                ApiError::BadRequest(format!("Path is not a directory: {}", path.display()))
            }
            RepoServiceError::NotGitRepository(path) => {
                ApiError::BadRequest(format!("Path is not a git repository: {}", path.display()))
            }
            RepoServiceError::NotFound => ApiError::BadRequest("Repository not found".to_string()),
            RepoServiceError::DirectoryAlreadyExists(path) => {
                ApiError::BadRequest(format!("Directory already exists: {}", path.display()))
            }
            RepoServiceError::Git(git_err) => {
                ApiError::BadRequest(format!("Git error: {}", git_err))
            }
            RepoServiceError::InvalidFolderName(name) => {
                ApiError::BadRequest(format!("Invalid folder name: {}", name))
            }
        }
    }
}

impl From<ProjectRepoError> for ApiError {
    fn from(err: ProjectRepoError) -> Self {
        match err {
            ProjectRepoError::Database(db_err) => ApiError::Database(db_err),
            ProjectRepoError::NotFound => {
                ApiError::BadRequest("Repository not found in project".to_string())
            }
            ProjectRepoError::AlreadyExists => {
                ApiError::Conflict("Repository already exists in project".to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_error_maps_to_expected_http_statuses() {
        assert_eq!(
            ApiError::BadRequest("bad".to_string())
                .into_response()
                .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ApiError::Unauthorized.into_response().status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            ApiError::Forbidden("nope".to_string())
                .into_response()
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            ApiError::NotFound("missing".to_string())
                .into_response()
                .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ApiError::Conflict("conflict".to_string())
                .into_response()
                .status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            ApiError::Internal("boom".to_string())
                .into_response()
                .status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn domain_errors_map_to_expected_http_statuses() {
        assert_eq!(
            ApiError::from(ProjectError::ProjectNotFound)
                .into_response()
                .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ApiError::from(RepoError::NotFound).into_response().status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ApiError::from(WorkspaceError::ValidationError("bad".to_string()))
                .into_response()
                .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ApiError::from(SessionError::NotFound)
                .into_response()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
}
