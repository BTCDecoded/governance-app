use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub head: CommitRef,
    pub base: CommitRef,
    pub user: User,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitRef {
    pub sha: String,
    pub ref_field: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub login: String,
    pub id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub name: String,
    pub full_name: String,
    pub owner: User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub action: String,
    pub pull_request: Option<PullRequest>,
    pub repository: Option<Repository>,
    pub issue: Option<Issue>,
    pub comment: Option<Comment>,
    pub review: Option<Review>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub user: User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub body: String,
    pub user: User,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub state: String,
    pub user: User,
    pub body: Option<String>,
}
