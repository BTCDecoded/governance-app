//! Tests for governance review system

use blvm_commons::governance_review::{
    AppealManager, GovernanceReviewCaseManager, MediationManager, RemovalManager, SanctionManager,
    TimeLimitManager, get_database_url, get_github_token, get_governance_repo, is_github_actions,
};
use chrono::{Duration, Utc};
use sqlx::SqlitePool;
use std::env;

#[tokio::test]
async fn test_env_variables() {
    // Test get_database_url with default
    env::remove_var("DATABASE_URL");
    let url = get_database_url();
    assert_eq!(url, "sqlite://governance.db");

    // Test get_database_url with env var
    env::set_var("DATABASE_URL", "sqlite://test.db");
    let url = get_database_url();
    assert_eq!(url, "sqlite://test.db");
    env::remove_var("DATABASE_URL");

    // Test get_github_token
    env::remove_var("GITHUB_TOKEN");
    env::remove_var("GITHUB_INSTALLATION_TOKEN");
    assert!(get_github_token().is_none());

    env::set_var("GITHUB_TOKEN", "test_token");
    assert_eq!(get_github_token(), Some("test_token".to_string()));
    env::remove_var("GITHUB_TOKEN");

    env::set_var("GITHUB_INSTALLATION_TOKEN", "install_token");
    assert_eq!(get_github_token(), Some("install_token".to_string()));
    env::remove_var("GITHUB_INSTALLATION_TOKEN");

    // Test get_governance_repo
    env::remove_var("GOVERNANCE_REPO");
    env::remove_var("GOVERNANCE_REPO_OWNER");
    env::remove_var("GOVERNANCE_REPO_NAME");
    assert!(get_governance_repo().is_none());

    env::set_var("GOVERNANCE_REPO", "owner/repo");
    let repo = get_governance_repo();
    assert_eq!(repo, Some(("owner".to_string(), "repo".to_string())));
    env::remove_var("GOVERNANCE_REPO");

    // Test is_github_actions
    env::remove_var("GITHUB_ACTIONS");
    assert!(!is_github_actions());

    env::set_var("GITHUB_ACTIONS", "true");
    assert!(is_github_actions());
    env::remove_var("GITHUB_ACTIONS");
}

#[tokio::test]
async fn test_case_creation_off_platform_rejection() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

    // Run migrations
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS governance_review_cases (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            case_number TEXT UNIQUE NOT NULL,
            subject_maintainer_id INTEGER NOT NULL,
            reporter_maintainer_id INTEGER NOT NULL,
            case_type TEXT NOT NULL,
            severity TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'open',
            description TEXT NOT NULL,
            evidence TEXT NOT NULL DEFAULT '{}',
            on_platform BOOLEAN NOT NULL DEFAULT true,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            response_deadline TEXT,
            resolution_deadline TEXT,
            resolved_at TEXT,
            resolution_reason TEXT,
            github_issue_number INTEGER
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let case_manager = GovernanceReviewCaseManager::new(pool);

    // Try to create off-platform case - should fail
    let result = case_manager
        .create_case(
            1,
            2,
            "abuse",
            "minor",
            "Test case",
            serde_json::json!({}),
            false, // off-platform
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_sanction_thresholds() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

    // Run migrations
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS governance_review_cases (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            case_number TEXT UNIQUE NOT NULL,
            subject_maintainer_id INTEGER NOT NULL,
            reporter_maintainer_id INTEGER NOT NULL,
            case_type TEXT NOT NULL,
            severity TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'open',
            description TEXT NOT NULL,
            evidence TEXT NOT NULL DEFAULT '{}',
            on_platform BOOLEAN NOT NULL DEFAULT true,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            response_deadline TEXT,
            resolution_deadline TEXT,
            resolved_at TEXT,
            resolution_reason TEXT,
            github_issue_number INTEGER
        );
        CREATE TABLE IF NOT EXISTS governance_review_warnings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            case_id INTEGER NOT NULL,
            maintainer_id INTEGER NOT NULL,
            warning_level INTEGER NOT NULL,
            warning_type TEXT NOT NULL,
            issued_by_team_approval INTEGER NOT NULL,
            issued_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            improvement_deadline TEXT,
            improvement_extended BOOLEAN DEFAULT false,
            improvement_extended_until TEXT,
            resolved BOOLEAN DEFAULT false,
            resolved_at TEXT,
            warning_file_path TEXT
        );
        CREATE TABLE IF NOT EXISTS governance_review_sanction_approvals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            case_id INTEGER NOT NULL,
            maintainer_id INTEGER NOT NULL,
            sanction_type TEXT NOT NULL,
            approved_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            signature TEXT
        );
        CREATE TABLE IF NOT EXISTS governance_review_time_limits (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            case_id INTEGER NOT NULL,
            limit_type TEXT NOT NULL,
            deadline TEXT NOT NULL,
            extended BOOLEAN DEFAULT false,
            extension_approved_by INTEGER,
            extension_reason TEXT,
            extension_until TEXT
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let sanction_manager = SanctionManager::new(pool.clone());
    let case_manager = GovernanceReviewCaseManager::new(pool.clone());

    // Create a case first
    let case = case_manager
        .create_case(
            1,
            2,
            "abuse",
            "minor",
            "Test case",
            serde_json::json!({}),
            true,
        )
        .await
        .unwrap();

    // Try private warning with insufficient approvals (need 4, only 3)
    let result = sanction_manager
        .issue_private_warning(case.id, 1, vec![10, 11, 12]) // Only 3 approvals
        .await;

    assert!(result.is_err());

    // Try private warning with sufficient approvals (4)
    let result = sanction_manager
        .issue_private_warning(case.id, 1, vec![10, 11, 12, 13]) // 4 approvals
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_time_limits() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS governance_review_time_limits (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            case_id INTEGER NOT NULL,
            limit_type TEXT NOT NULL,
            deadline TEXT NOT NULL,
            extended BOOLEAN DEFAULT false,
            extension_approved_by INTEGER,
            extension_reason TEXT,
            extension_until TEXT
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let time_limit_manager = TimeLimitManager::new(pool);

    let response_deadline = Utc::now() + Duration::days(30);
    let resolution_deadline = Utc::now() + Duration::days(180);

    // Create time limits
    let result = time_limit_manager
        .create_time_limits(1, response_deadline, resolution_deadline)
        .await;

    assert!(result.is_ok());

    // Check for expired limits (should be none)
    let expired = time_limit_manager.check_expired_limits().await.unwrap();
    assert_eq!(expired.len(), 0);
}

#[tokio::test]
async fn test_removal_deactivation() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS maintainers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            github_username TEXT NOT NULL UNIQUE,
            public_key TEXT NOT NULL,
            layer INTEGER NOT NULL,
            active BOOLEAN DEFAULT true,
            last_updated TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS governance_review_cases (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            case_number TEXT UNIQUE NOT NULL,
            subject_maintainer_id INTEGER NOT NULL,
            reporter_maintainer_id INTEGER NOT NULL,
            case_type TEXT NOT NULL,
            severity TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'open',
            description TEXT NOT NULL,
            evidence TEXT NOT NULL DEFAULT '{}',
            on_platform BOOLEAN NOT NULL DEFAULT true,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            response_deadline TEXT,
            resolution_deadline TEXT,
            resolved_at TEXT,
            resolution_reason TEXT,
            github_issue_number INTEGER
        );
        CREATE TABLE IF NOT EXISTS governance_review_sanction_approvals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            case_id INTEGER NOT NULL,
            maintainer_id INTEGER NOT NULL,
            sanction_type TEXT NOT NULL,
            approved_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            signature TEXT
        );
        CREATE TABLE IF NOT EXISTS governance_review_time_limits (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            case_id INTEGER NOT NULL,
            limit_type TEXT NOT NULL,
            deadline TEXT NOT NULL,
            extended BOOLEAN DEFAULT false,
            extension_approved_by INTEGER,
            extension_reason TEXT,
            extension_until TEXT
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Create a maintainer
    sqlx::query(
        "INSERT INTO maintainers (github_username, public_key, layer, active) VALUES (?, ?, ?, ?)",
    )
    .bind("test_maintainer")
    .bind("test_key")
    .bind(1)
    .bind(true)
    .execute(&pool)
    .await
    .unwrap();

    let removal_manager = RemovalManager::new(pool.clone());

    // Create a case
    let case_manager = GovernanceReviewCaseManager::new(pool.clone());
    let case = case_manager
        .create_case(
            1,
            2,
            "abuse",
            "serious",
            "Test removal case",
            serde_json::json!({}),
            true,
        )
        .await
        .unwrap();

    // Check maintainer is active
    assert!(removal_manager.is_maintainer_active(1).await.unwrap());

    // Remove maintainer (6-of-7 team + 4-of-7 teams)
    let result = removal_manager
        .remove_maintainer(
            case.id,
            1,
            vec![10, 11, 12, 13, 14, 15], // 6 approvals
            4,                            // 4 teams
        )
        .await;

    assert!(result.is_ok());

    // Check maintainer is now inactive
    assert!(!removal_manager.is_maintainer_active(1).await.unwrap());
}
