//! GitHub File Operations
//!
//! This module provides utilities for fetching file content and directory structures
//! from GitHub repositories via the GitHub API.

use crate::error::GovernanceError;
use base64::{Engine as _, engine::general_purpose};
use octocrab::Octocrab;
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

/// Represents a file in a GitHub repository
#[derive(Debug, Clone)]
pub struct GitHubFile {
    pub path: String,
    pub content: Vec<u8>,
    pub sha: String,
    pub size: u64,
    pub download_url: Option<String>,
}

/// Represents a directory tree in a GitHub repository
#[derive(Debug, Clone)]
pub struct GitHubDirectory {
    pub path: String,
    pub files: Vec<GitHubFile>,
    pub subdirectories: Vec<GitHubDirectory>,
    pub total_size: u64,
}

/// GitHub repository information
#[derive(Debug, Clone)]
pub struct GitHubRepo {
    pub owner: String,
    pub name: String,
    pub default_branch: String,
    pub last_commit_sha: String,
}

/// File comparison result
#[derive(Debug, Clone)]
pub struct FileComparison {
    pub file_path: String,
    pub source_sha: String,
    pub target_sha: Option<String>,
    pub is_same: bool,
    pub size_diff: Option<i64>,
    pub content_diff: Option<String>,
}

pub struct GitHubFileOperations {
    client: Octocrab,
}

impl GitHubFileOperations {
    /// Create a new GitHub file operations client
    pub fn new(token: String) -> Result<Self, GovernanceError> {
        let client = Octocrab::builder()
            .personal_token(token)
            .build()
            .map_err(|e| {
                GovernanceError::GitHubError(format!("Failed to create GitHub client: {e}"))
            })?;

        Ok(Self { client })
    }

    /// Fetch file content from GitHub repository
    pub async fn fetch_file_content(
        &self,
        owner: &str,
        repo: &str,
        file_path: &str,
        branch: Option<&str>,
    ) -> Result<GitHubFile, GovernanceError> {
        info!("Fetching file content: {}/{}:{}", owner, repo, file_path);

        let branch = branch.unwrap_or("main");

        let response = self
            .client
            .repos(owner, repo)
            .get_content()
            .path(file_path)
            .r#ref(branch)
            .send()
            .await
            .map_err(|e| GovernanceError::GitHubError(format!("Failed to fetch file: {e}")))?;

        // Handle the response - octocrab 0.38 returns ContentItems with items: Vec<Content>
        // For a single file, items should contain one Content with type "file"
        let items = response.items;
        if items.len() != 1 {
            return Err(GovernanceError::GitHubError(format!(
                "Expected single file, got {} items",
                items.len()
            )));
        }

        let content = &items[0];
        match content.r#type.as_str() {
            "file" => {
                // Decode base64 content
                let content_bytes = match &content.content {
                    Some(encoded) => general_purpose::STANDARD
                        .decode(encoded.trim_end_matches('\n'))
                        .map_err(|e| {
                            GovernanceError::GitHubError(format!(
                                "Failed to decode base64 content: {e}"
                            ))
                        })?,
                    None => {
                        return Err(GovernanceError::GitHubError(
                            "File content is empty".to_string(),
                        ));
                    }
                };

                Ok(GitHubFile {
                    path: content.path.clone(),
                    content: content_bytes,
                    sha: content.sha.clone(),
                    size: content.size as u64,
                    download_url: content.download_url.as_ref().map(|u| u.to_string()),
                })
            }
            "dir" => Err(GovernanceError::GitHubError(format!(
                "Path '{file_path}' is a directory, not a file"
            ))),
            "symlink" => Err(GovernanceError::GitHubError(format!(
                "Path '{file_path}' is a symlink, not a file"
            ))),
            "submodule" => Err(GovernanceError::GitHubError(format!(
                "Path '{file_path}' is a submodule, not a file"
            ))),
            _ => Err(GovernanceError::GitHubError(format!(
                "Unknown content type: {}",
                content.r#type
            ))),
        }
    }

    /// Fetch directory tree from GitHub repository
    pub async fn fetch_directory_tree(
        &self,
        owner: &str,
        repo: &str,
        directory_path: &str,
        branch: Option<&str>,
    ) -> Result<GitHubDirectory, GovernanceError> {
        info!(
            "Fetching directory tree: {}/{}:{}",
            owner, repo, directory_path
        );

        let branch = branch.unwrap_or("main");

        let response = self
            .client
            .repos(owner, repo)
            .get_content()
            .path(directory_path)
            .r#ref(branch)
            .send()
            .await
            .map_err(|e| GovernanceError::GitHubError(format!("Failed to fetch directory: {e}")))?;

        // Handle the response - octocrab 0.38 returns ContentItems with items: Vec<Content>
        // For a directory, items contains multiple Content items
        let items = response.items;
        let mut files = Vec::new();
        let subdirectories = Vec::new();
        let mut total_size = 0u64;

        // Process each item in the directory
        for item in items {
            match item.r#type.as_str() {
                "file" => {
                    // For files, create GitHubFile with metadata
                    // Content can be fetched later if needed via fetch_file_content()
                    let size = item.size as u64;
                    total_size += size;

                    files.push(GitHubFile {
                        path: item.path.clone(),
                        content: Vec::new(), // Content not loaded by default (can fetch later)
                        sha: item.sha.clone(),
                        size,
                        download_url: item.download_url.as_ref().map(|u| u.to_string()),
                    });
                }
                "dir" => {
                    // For subdirectories, we can recursively fetch them if needed
                    // For now, skip nested directories - they can be fetched separately if needed
                    debug!(
                        "Skipping nested directory: {} - fetch separately if needed",
                        item.path
                    );
                }
                "symlink" | "submodule" => {
                    // Skip symlinks and submodules
                    debug!("Skipping symlink/submodule in directory: {}", item.path);
                }
                _ => {
                    warn!("Unknown content type in directory: {}", item.r#type);
                }
            }
        }

        Ok(GitHubDirectory {
            path: directory_path.to_string(),
            files,
            subdirectories,
            total_size,
        })
    }

    /// Compute hash of entire repository state
    /// Returns the SHA of the latest commit on the specified branch
    pub async fn compute_repo_hash(
        &self,
        owner: &str,
        repo: &str,
        branch: Option<&str>,
    ) -> Result<String, GovernanceError> {
        info!("Computing repository hash: {}/{}", owner, repo);

        let branch = branch.unwrap_or("main");

        // Get the branch reference to get the latest commit SHA
        // Try using repos().get_branch() - if it doesn't exist, use commits API
        // For now, use a workaround: get the latest commit from the default branch
        let commits = self
            .client
            .repos(owner, repo)
            .list_commits()
            .branch(branch)
            .per_page(1)
            .send()
            .await
            .map_err(|e| GovernanceError::GitHubError(format!("Failed to get branch: {e}")))?;

        // Extract commit SHA from first commit
        let commit_sha = commits
            .items
            .first()
            .ok_or_else(|| GovernanceError::GitHubError("No commits found".to_string()))?
            .sha
            .clone();

        info!(
            "Repository hash for {}/{}:{} = {}",
            owner, repo, branch, commit_sha
        );

        Ok(commit_sha)
    }

    /// Compare file versions across repositories
    pub async fn compare_file_versions(
        &self,
        source_owner: &str,
        source_repo: &str,
        source_file: &str,
        target_owner: &str,
        target_repo: &str,
        target_file: &str,
        branch: Option<&str>,
    ) -> Result<FileComparison, GovernanceError> {
        info!(
            "Comparing files: {}/{}:{} vs {}/{}:{}",
            source_owner, source_repo, source_file, target_owner, target_repo, target_file
        );

        let branch = branch.unwrap_or("main");

        // Fetch source file
        let source_file_data = self
            .fetch_file_content(source_owner, source_repo, source_file, Some(branch))
            .await?;

        // Try to fetch target file
        let target_file_data = match self
            .fetch_file_content(target_owner, target_repo, target_file, Some(branch))
            .await
        {
            Ok(file) => Some(file),
            Err(e) => {
                warn!("Target file not found: {}", e);
                None
            }
        };

        let is_same = if let Some(ref target) = target_file_data {
            source_file_data.sha == target.sha
        } else {
            false
        };

        let size_diff = target_file_data
            .as_ref()
            .map(|target| source_file_data.size as i64 - target.size as i64);

        let content_diff = if let Some(ref target) = target_file_data {
            if source_file_data.content != target.content {
                Some(format!(
                    "Content differs: {} bytes vs {} bytes",
                    source_file_data.content.len(),
                    target.content.len()
                ))
            } else {
                None
            }
        } else {
            Some("Target file does not exist".to_string())
        };

        Ok(FileComparison {
            file_path: source_file.to_string(),
            source_sha: source_file_data.sha,
            target_sha: target_file_data.map(|f| f.sha),
            is_same,
            size_diff,
            content_diff,
        })
    }

    /// Get repository information
    pub async fn get_repo_info(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<GitHubRepo, GovernanceError> {
        info!("Getting repository info: {}/{}", owner, repo);

        // Get repository information using octocrab API
        let repository = self.client.repos(owner, repo).get().await.map_err(|e| {
            GovernanceError::GitHubError(format!("Failed to get repository info: {e}"))
        })?;

        // Get the default branch's latest commit SHA
        let default_branch = repository.default_branch.as_deref().unwrap_or("main");
        let last_commit_sha = self
            .compute_repo_hash(owner, repo, Some(default_branch))
            .await?;

        // In octocrab 0.38, owner structure - extract login
        // Owner is Option<Author>, need to unwrap first
        let owner_name = repository
            .owner
            .as_ref()
            .map(|author| author.login.clone())
            .unwrap_or_else(|| "unknown".to_string());

        Ok(GitHubRepo {
            owner: owner_name,
            name: repository.name.clone(),
            default_branch: default_branch.to_string(),
            last_commit_sha,
        })
    }

    /// Fetch multiple files in parallel
    pub async fn fetch_multiple_files(
        &self,
        owner: &str,
        repo: &str,
        file_paths: &[String],
        branch: Option<&str>,
    ) -> Result<HashMap<String, GitHubFile>, GovernanceError> {
        info!("Fetching {} files in parallel", file_paths.len());

        let mut results = HashMap::new();
        let mut tasks = Vec::new();

        for file_path in file_paths {
            let client = self.client.clone();
            let owner = owner.to_string();
            let repo = repo.to_string();
            let file_path = file_path.clone();
            let branch = branch.map(|s| s.to_string());

            let task = tokio::spawn(async move {
                match Self::fetch_file_content_static(
                    &client,
                    &owner,
                    &repo,
                    &file_path,
                    branch.as_deref(),
                )
                .await
                {
                    Ok(file) => Some((file_path, file)),
                    Err(e) => {
                        error!("Failed to fetch file {}: {}", file_path, e);
                        None
                    }
                }
            });

            tasks.push(task);
        }

        // Wait for all tasks to complete
        for task in tasks {
            if let Ok(Some((path, file))) = task.await {
                results.insert(path, file);
            }
        }

        Ok(results)
    }

    /// Static method for fetching file content (used in async tasks)
    async fn fetch_file_content_static(
        client: &Octocrab,
        owner: &str,
        repo: &str,
        file_path: &str,
        branch: Option<&str>,
    ) -> Result<GitHubFile, GovernanceError> {
        let branch = branch.unwrap_or("main");

        let response = client
            .repos(owner, repo)
            .get_content()
            .path(file_path)
            .r#ref(branch)
            .send()
            .await
            .map_err(|e| GovernanceError::GitHubError(format!("Failed to fetch file: {e}")))?;

        // Handle the response - octocrab 0.38 returns ContentItems with items: Vec<Content>
        // For a single file, items should contain one Content with type "file"
        let items = response.items;
        if items.len() != 1 {
            return Err(GovernanceError::GitHubError(format!(
                "Expected single file, got {} items",
                items.len()
            )));
        }

        let content = &items[0];
        match content.r#type.as_str() {
            "file" => {
                // Decode base64 content
                let content_bytes = match &content.content {
                    Some(encoded) => general_purpose::STANDARD
                        .decode(encoded.trim_end_matches('\n'))
                        .map_err(|e| {
                            GovernanceError::GitHubError(format!(
                                "Failed to decode base64 content: {e}"
                            ))
                        })?,
                    None => {
                        return Err(GovernanceError::GitHubError(
                            "File content is empty".to_string(),
                        ));
                    }
                };

                Ok(GitHubFile {
                    path: content.path.clone(),
                    content: content_bytes,
                    sha: content.sha.clone(),
                    size: content.size as u64,
                    download_url: content.download_url.as_ref().map(|u| u.to_string()),
                })
            }
            "dir" => Err(GovernanceError::GitHubError(format!(
                "Path '{file_path}' is a directory, not a file"
            ))),
            "symlink" => Err(GovernanceError::GitHubError(format!(
                "Path '{file_path}' is a symlink, not a file"
            ))),
            "submodule" => Err(GovernanceError::GitHubError(format!(
                "Path '{file_path}' is a submodule, not a file"
            ))),
            _ => Err(GovernanceError::GitHubError(format!(
                "Unknown content type: {}",
                content.r#type
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_github_file_operations_creation() {
        // This test requires a valid GitHub token
        // In a real test environment, you would use a test token
        let result = GitHubFileOperations::new("test_token".to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn test_file_comparison_creation() {
        let comparison = FileComparison {
            file_path: "test.txt".to_string(),
            source_sha: "abc123".to_string(),
            target_sha: Some("def456".to_string()),
            is_same: false,
            size_diff: Some(100),
            content_diff: Some("Content differs".to_string()),
        };

        assert_eq!(comparison.file_path, "test.txt");
        assert_eq!(comparison.source_sha, "abc123");
        assert_eq!(comparison.target_sha, Some("def456".to_string()));
        assert!(!comparison.is_same);
        assert_eq!(comparison.size_diff, Some(100));
        assert_eq!(comparison.content_diff, Some("Content differs".to_string()));
    }

    #[test]
    fn test_github_repo_creation() {
        let repo = GitHubRepo {
            owner: "test-owner".to_string(),
            name: "test-repo".to_string(),
            default_branch: "main".to_string(),
            last_commit_sha: "abc123def456".to_string(),
        };

        assert_eq!(repo.owner, "test-owner");
        assert_eq!(repo.name, "test-repo");
        assert_eq!(repo.default_branch, "main");
        assert_eq!(repo.last_commit_sha, "abc123def456");
    }
}
