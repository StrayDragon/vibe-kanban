use regex::Regex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitHubRepoInfoError {
    #[error("Repository error: {0}")]
    Repository(String),
}

#[derive(Debug, Clone)]
pub struct GitHubRepoInfo {
    pub owner: String,
    pub repo_name: String,
}

impl GitHubRepoInfo {
    pub fn from_remote_url(remote_url: &str) -> Result<Self, GitHubRepoInfoError> {
        let re = Regex::new(r"github\.com[:/](?P<owner>[^/]+)/(?P<repo>[^/]+?)(?:\.git)?(?:/|$)")
            .map_err(|error| {
            GitHubRepoInfoError::Repository(format!("Failed to compile regex: {error}"))
        })?;

        let caps = re.captures(remote_url).ok_or_else(|| {
            GitHubRepoInfoError::Repository(format!("Invalid GitHub URL format: {remote_url}"))
        })?;

        let owner = caps
            .name("owner")
            .ok_or_else(|| {
                GitHubRepoInfoError::Repository("Missing owner in GitHub URL".to_string())
            })?
            .as_str()
            .to_string();
        let repo_name = caps
            .name("repo")
            .ok_or_else(|| {
                GitHubRepoInfoError::Repository("Missing repo name in GitHub URL".to_string())
            })?
            .as_str()
            .to_string();

        Ok(Self { owner, repo_name })
    }
}
