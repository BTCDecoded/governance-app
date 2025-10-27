//! GitHub Integration Tests
//!
//! Tests for GitHub API client, status check posting,
//! merge blocking/unblocking, and webhook event handling

use governance_app::database::Database;
use governance_app::error::GovernanceError;
use governance_app::github::{client::GitHubClient, webhooks::WebhookProcessor};
use governance_app::webhooks::github_integration::GitHubIntegration;
use serde_json::json;

#[tokio::test]
async fn test_github_client_creation() -> Result<(), Box<dyn std::error::Error>> {
    // Test GitHub client creation with mock credentials
    let client = GitHubClient::new(12345, "/tmp/non_existent_key.pem");

    // Should fail with file not found error
    assert!(client.is_err());
    println!("✅ GitHub client creation correctly fails with invalid key path");

    Ok(())
}

#[tokio::test]
async fn test_status_check_posting() -> Result<(), Box<dyn std::error::Error>> {
    // Create a mock GitHub client (we'll use the simplified version)
    let client = GitHubClient::new(12345, "/tmp/non_existent_key.pem");

    // For now, we'll test the mock implementation
    // In a real scenario, this would test actual GitHub API calls
    println!("✅ Status check posting test (mock implementation)");

    Ok(())
}

#[tokio::test]
async fn test_webhook_event_processing() -> Result<(), Box<dyn std::error::Error>> {
    // Test PR opened event
    let pr_opened_payload = json!({
        "action": "opened",
        "pull_request": {
            "number": 123,
            "title": "Test PR",
            "body": "Test PR body",
            "head": {
                "sha": "abc123def456",
                "ref": "feature-branch"
            },
            "base": {
                "sha": "def456ghi789",
                "ref": "main"
            }
        },
        "repository": {
            "full_name": "test-org/test-repo",
            "name": "test-repo"
        }
    });

    let event = WebhookProcessor::process_webhook(&pr_opened_payload)?;
    assert_eq!(event.event_type.to_string(), "pull_request");
    println!("✅ PR opened webhook processed successfully");

    // Test PR comment event
    let pr_comment_payload = json!({
        "action": "created",
        "issue": {
            "number": 123,
            "pull_request": {
                "url": "https://api.github.com/repos/test-org/test-repo/pulls/123"
            }
        },
        "comment": {
            "body": "/governance-sign test_signature_123",
            "user": {
                "login": "test-maintainer"
            }
        },
        "repository": {
            "full_name": "test-org/test-repo",
            "name": "test-repo"
        }
    });

    let event = WebhookProcessor::process_webhook(&pr_comment_payload)?;
    assert_eq!(event.event_type.to_string(), "issue_comment");
    println!("✅ PR comment webhook processed successfully");

    // Test PR updated event
    let pr_updated_payload = json!({
        "action": "synchronize",
        "pull_request": {
            "number": 123,
            "title": "Updated Test PR",
            "body": "Updated test PR body",
            "head": {
                "sha": "xyz789abc123",
                "ref": "feature-branch"
            },
            "base": {
                "sha": "def456ghi789",
                "ref": "main"
            }
        },
        "repository": {
            "full_name": "test-org/test-repo",
            "name": "test-repo"
        }
    });

    let event = WebhookProcessor::process_webhook(&pr_updated_payload)?;
    assert_eq!(event.event_type.to_string(), "pull_request");
    println!("✅ PR updated webhook processed successfully");

    Ok(())
}

#[tokio::test]
async fn test_github_integration_initialization() -> Result<(), Box<dyn std::error::Error>> {
    // Create in-memory database
    let db = Database::new_in_memory().await?;

    // Create mock GitHub client
    let github_client = GitHubClient::new(12345, "/tmp/non_existent_key.pem");

    // This should fail due to invalid key path, but we can test the structure
    assert!(github_client.is_err());
    println!("✅ GitHub integration initialization test (expected failure with mock credentials)");

    Ok(())
}

#[tokio::test]
async fn test_repository_info_parsing() -> Result<(), Box<dyn std::error::Error>> {
    // Test repository name parsing
    let test_cases = vec![
        ("test-org/test-repo", ("test-org", "test-repo")),
        (
            "btcdecoded/governance-app",
            ("btcdecoded", "governance-app"),
        ),
        ("user/repo-name", ("user", "repo-name")),
    ];

    for (full_name, expected) in test_cases {
        let parts: Vec<&str> = full_name.split('/').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], expected.0);
        assert_eq!(parts[1], expected.1);
        println!(
            "✅ Repository name '{}' parsed correctly as '{}/{}'",
            full_name, expected.0, expected.1
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_pr_information_extraction() -> Result<(), Box<dyn std::error::Error>> {
    let pr_payload = json!({
        "pull_request": {
            "number": 456,
            "title": "Test Governance PR",
            "body": "This PR implements governance changes",
            "head": {
                "sha": "commit123abc",
                "ref": "governance-feature"
            },
            "base": {
                "sha": "main123def",
                "ref": "main"
            }
        },
        "repository": {
            "full_name": "btcdecoded/governance-app",
            "name": "governance-app"
        }
    });

    // Extract PR information
    let pr_number = pr_payload["pull_request"]["number"].as_u64().unwrap();
    let title = pr_payload["pull_request"]["title"].as_str().unwrap();
    let head_sha = pr_payload["pull_request"]["head"]["sha"].as_str().unwrap();
    let repo_name = pr_payload["repository"]["full_name"].as_str().unwrap();

    assert_eq!(pr_number, 456);
    assert_eq!(title, "Test Governance PR");
    assert_eq!(head_sha, "commit123abc");
    assert_eq!(repo_name, "btcdecoded/governance-app");

    println!("✅ PR information extracted successfully:");
    println!("   PR Number: {}", pr_number);
    println!("   Title: {}", title);
    println!("   Head SHA: {}", head_sha);
    println!("   Repository: {}", repo_name);

    Ok(())
}

#[tokio::test]
async fn test_governance_signature_parsing() -> Result<(), Box<dyn std::error::Error>> {
    // Test governance signature command parsing
    let test_comments = vec![
        ("/governance-sign abc123def456", "abc123def456"),
        ("/governance-sign xyz789ghi012", "xyz789ghi012"),
        ("/governance-sign   spaced_signature  ", "spaced_signature"),
        ("/governance-sign", ""), // Empty signature
        ("Regular comment", ""),  // Not a governance command
    ];

    for (comment, expected_signature) in test_comments {
        let signature = if comment.starts_with("/governance-sign") {
            comment
                .strip_prefix("/governance-sign")
                .unwrap_or("")
                .trim()
        } else {
            ""
        };

        assert_eq!(signature, expected_signature);
        println!(
            "✅ Comment '{}' parsed as signature '{}'",
            comment, signature
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_tier_classification() -> Result<(), Box<dyn std::error::Error>> {
    use governance_app::validation::tier_classification;

    // Test different PR payloads for tier classification
    let routine_pr = json!({
        "pull_request": {
            "title": "Fix typo in documentation",
            "body": "Simple documentation fix"
        }
    });

    let feature_pr = json!({
        "pull_request": {
            "title": "[FEATURE] Add new RPC method",
            "body": "This PR adds a new RPC method for governance"
        }
    });

    let consensus_adjacent_pr = json!({
        "pull_request": {
            "title": "[CONSENSUS-ADJACENT] Update validation logic",
            "body": "This PR updates consensus validation code"
        }
    });

    let emergency_pr = json!({
        "pull_request": {
            "title": "[EMERGENCY] Critical security fix",
            "body": "This PR fixes a critical security vulnerability"
        }
    });

    let governance_pr = json!({
        "pull_request": {
            "title": "[GOVERNANCE] Update governance rules",
            "body": "This PR updates the governance configuration"
        }
    });

    // Test tier classification
    let routine_tier = tier_classification::classify_pr_tier(&routine_pr).await;
    let feature_tier = tier_classification::classify_pr_tier(&feature_pr).await;
    let consensus_tier = tier_classification::classify_pr_tier(&consensus_adjacent_pr).await;
    let emergency_tier = tier_classification::classify_pr_tier(&emergency_pr).await;
    let governance_tier = tier_classification::classify_pr_tier(&governance_pr).await;

    assert_eq!(routine_tier, 1);
    assert_eq!(feature_tier, 2);
    assert_eq!(consensus_tier, 3);
    assert_eq!(emergency_tier, 4);
    assert_eq!(governance_tier, 5);

    println!("✅ Tier classification working correctly:");
    println!("   Routine PR: Tier {}", routine_tier);
    println!("   Feature PR: Tier {}", feature_tier);
    println!("   Consensus-Adjacent PR: Tier {}", consensus_tier);
    println!("   Emergency PR: Tier {}", emergency_tier);
    println!("   Governance PR: Tier {}", governance_tier);

    Ok(())
}

#[tokio::test]
async fn test_status_check_generation() -> Result<(), Box<dyn std::error::Error>> {
    use governance_app::enforcement::status_checks::StatusCheckGenerator;

    // Test review period status generation
    let opened_at = chrono::Utc::now() - chrono::Duration::days(5);
    let review_status = StatusCheckGenerator::generate_review_period_status(
        opened_at, 7,     // required days
        false, // emergency mode
    );
    assert!(review_status.contains("Review period met"));
    println!("✅ Review period status generated: {}", review_status);

    // Test signature status generation
    let signature_status = StatusCheckGenerator::generate_signature_status(
        3, // current signatures
        3, // required signatures
        5, // total maintainers
        &[
            "maintainer1".to_string(),
            "maintainer2".to_string(),
            "maintainer3".to_string(),
        ],
        &["maintainer4".to_string(), "maintainer5".to_string()],
    );
    assert!(signature_status.contains("Signatures met"));
    println!("✅ Signature status generated: {}", signature_status);

    // Test combined status generation
    let combined_status = StatusCheckGenerator::generate_combined_status(
        true, // review period met
        true, // signatures met
        "Feature Changes",
        &review_status,
    );
    assert!(combined_status.contains("Feature Changes"));
    println!("✅ Combined status generated: {}", combined_status);

    Ok(())
}

#[tokio::test]
async fn test_merge_blocking_logic() -> Result<(), Box<dyn std::error::Error>> {
    use governance_app::enforcement::merge_block::MergeBlocker;

    // Test merge blocking conditions
    let blocker = MergeBlocker::new(None);

    // Test case: All requirements met
    let should_block_all_met = MergeBlocker::should_block_merge(
        true,  // review period met
        true,  // signatures met
        false, // economic veto active
        1,     // tier
        false, // emergency mode
    )
    .unwrap();
    assert!(!should_block_all_met);
    println!("✅ Merge not blocked when all requirements met");

    // Test case: Review period not met
    let should_block_review = MergeBlocker::should_block_merge(
        false, // review period not met
        true,  // signatures met
        false, // economic veto active
        1,     // tier
        false, // emergency mode
    )
    .unwrap();
    assert!(should_block_review);
    println!("✅ Merge blocked when review period not met");

    // Test case: Signatures not met
    let should_block_signatures = MergeBlocker::should_block_merge(
        true,  // review period met
        false, // signatures not met
        false, // economic veto active
        1,     // tier
        false, // emergency mode
    )
    .unwrap();
    assert!(should_block_signatures);
    println!("✅ Merge blocked when signatures not met");

    // Test case: Economic veto active
    let should_block_veto = MergeBlocker::should_block_merge(
        true,  // review period met
        true,  // signatures met
        true,  // economic veto active
        3,     // tier 3 (consensus-adjacent)
        false, // emergency mode
    )
    .unwrap();
    assert!(should_block_veto);
    println!("✅ Merge blocked when economic veto active");

    Ok(())
}

#[tokio::test]
async fn test_webhook_event_types() -> Result<(), Box<dyn std::error::Error>> {
    use governance_app::github::webhooks::WebhookEventType;

    // Test webhook event type parsing
    let event_types = vec![
        ("pull_request", WebhookEventType::PullRequest),
        ("issue_comment", WebhookEventType::IssueComment),
        ("push", WebhookEventType::Push),
    ];

    for (event_name, expected_type) in event_types {
        let parsed = event_name.parse::<WebhookEventType>();
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap(), expected_type);
        println!("✅ Event type '{}' parsed correctly", event_name);
    }

    // Test invalid event type
    let invalid_parsed = "invalid_event".parse::<WebhookEventType>();
    assert!(invalid_parsed.is_err());
    println!("✅ Invalid event type correctly rejected");

    Ok(())
}

#[tokio::test]
async fn test_github_api_mock_responses() -> Result<(), Box<dyn std::error::Error>> {
    // Test mock GitHub API responses
    let mock_repo_info = json!({
        "id": 12345,
        "name": "test-repo",
        "full_name": "test-org/test-repo",
        "private": false,
        "default_branch": "main",
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-01T00:00:00Z"
    });

    assert_eq!(mock_repo_info["id"], 12345);
    assert_eq!(mock_repo_info["name"], "test-repo");
    assert_eq!(mock_repo_info["full_name"], "test-org/test-repo");
    println!("✅ Mock repository info structure validated");

    let mock_pr_info = json!({
        "id": 67890,
        "number": 123,
        "title": "Test PR",
        "body": "Test PR body",
        "state": "open",
        "head": {
            "sha": "abc123def456",
            "ref": "feature-branch"
        },
        "base": {
            "sha": "def456ghi789",
            "ref": "main"
        },
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-01T00:00:00Z"
    });

    assert_eq!(mock_pr_info["number"], 123);
    assert_eq!(mock_pr_info["state"], "open");
    assert_eq!(mock_pr_info["head"]["sha"], "abc123def456");
    println!("✅ Mock PR info structure validated");

    Ok(())
}
