//! Simple Governance System Tests
//!
//! Basic tests to verify the governance system components work correctly

use governance_app::database::Database;
use governance_app::validation::tier_classification;
use serde_json::json;

#[tokio::test]
async fn test_database_creation() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing database creation...");

    // Test in-memory database creation
    let _db = Database::new_in_memory().await?;
    println!("✅ In-memory database created successfully");

    // Test database operations
    let pool = _db.pool();
    assert!(!pool.is_closed());
    println!("✅ Database pool is accessible");

    Ok(())
}

#[tokio::test]
async fn test_tier_classification() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing tier classification...");

    // Test routine maintenance PR
    let routine_pr = json!({
        "pull_request": {
            "title": "Fix typo in README",
            "body": "Simple documentation fix"
        }
    });

    let tier = tier_classification::classify_pr_tier(&routine_pr).await;
    assert_eq!(tier, 1);
    println!("✅ Routine PR classified as Tier 1");

    // Test feature PR
    let feature_pr = json!({
        "pull_request": {
            "title": "[FEATURE] Add new RPC method",
            "body": "This PR adds a new RPC method",
            "files": [
                {"filename": "src/rpc/new_method.rs"},
                {"filename": "src/rpc/mod.rs"}
            ]
        }
    });

    let tier = tier_classification::classify_pr_tier(&feature_pr).await;
    assert_eq!(tier, 2);
    println!("✅ Feature PR classified as Tier 2");

    // Test consensus-adjacent PR
    let consensus_pr = json!({
        "pull_request": {
            "title": "[CONSENSUS-ADJACENT] Update validation logic",
            "body": "This PR updates consensus validation code"
        }
    });

    let tier = tier_classification::classify_pr_tier(&consensus_pr).await;
    assert_eq!(tier, 3);
    println!("✅ Consensus-adjacent PR classified as Tier 3");

    // Test emergency PR
    let emergency_pr = json!({
        "pull_request": {
            "title": "[EMERGENCY] Critical security fix",
            "body": "This PR fixes a critical security vulnerability"
        }
    });

    let tier = tier_classification::classify_pr_tier(&emergency_pr).await;
    assert_eq!(tier, 4);
    println!("✅ Emergency PR classified as Tier 4");

    // Test governance PR
    let governance_pr = json!({
        "pull_request": {
            "title": "[GOVERNANCE] Update governance rules",
            "body": "This PR updates the governance configuration"
        }
    });

    let tier = tier_classification::classify_pr_tier(&governance_pr).await;
    assert_eq!(tier, 5);
    println!("✅ Governance PR classified as Tier 5");

    Ok(())
}

#[tokio::test]
async fn test_status_check_generation() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing status check generation...");

    use governance_app::enforcement::status_checks::StatusCheckGenerator;

    // Test review period status
    let opened_at = chrono::Utc::now() - chrono::Duration::days(10);
    let review_status = StatusCheckGenerator::generate_review_period_status(
        opened_at, 7,     // required days
        false, // emergency mode
    );
    assert!(review_status.contains("Governance: Review Period Met"));
    println!("✅ Review period status generated: {}", review_status);

    // Test signature status
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
    assert!(signature_status.contains("Governance: Signatures Complete"));
    println!("✅ Signature status generated: {}", signature_status);

    // Test combined status
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
    println!("🧪 Testing merge blocking logic...");

    use governance_app::enforcement::merge_block::MergeBlocker;

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
async fn test_threshold_validation() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing threshold validation...");

    use governance_app::validation::threshold::ThresholdValidator;

    // Test tier-specific thresholds
    let (required, total) = ThresholdValidator::get_tier_threshold(1);
    assert_eq!(required, 3);
    assert_eq!(total, 5);
    println!("✅ Tier 1 threshold: {}/{}", required, total);

    let (required, total) = ThresholdValidator::get_tier_threshold(2);
    assert_eq!(required, 4);
    assert_eq!(total, 5);
    println!("✅ Tier 2 threshold: {}/{}", required, total);

    let (required, total) = ThresholdValidator::get_tier_threshold(3);
    assert_eq!(required, 5);
    assert_eq!(total, 5);
    println!("✅ Tier 3 threshold: {}/{}", required, total);

    // Test review periods
    let review_period = ThresholdValidator::get_tier_review_period(1);
    assert_eq!(review_period, 7);
    println!("✅ Tier 1 review period: {} days", review_period);

    let review_period = ThresholdValidator::get_tier_review_period(3);
    assert_eq!(review_period, 90);
    println!("✅ Tier 3 review period: {} days", review_period);

    Ok(())
}

#[tokio::test]
async fn test_governance_fork_export() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing governance fork export...");

    use governance_app::fork::{export::GovernanceExporter, types::*};

    // Create a temporary config directory for testing
    let temp_dir = tempfile::tempdir()?;
    let config_path = temp_dir.path().to_str().unwrap();

    // Create sample config files
    let action_tiers_content = r#"
tiers:
  - name: "Routine Maintenance"
    tier: 1
    signatures_required: 3
    signatures_total: 5
    review_period_days: 7
"#;

    let economic_nodes_content = r#"
nodes:
  - type: "mining_pool"
    name: "Test Pool"
    hashpower_percent: 5.0
"#;

    let maintainers_content = r#"
maintainers:
  - name: "Test Maintainer"
    public_key: "test_key"
    layer: 1
"#;

    let repos_content = r#"
repositories:
  - name: "test-repo"
    layer: 1
    governance_enabled: true
"#;

    let governance_fork_content = r#"
fork:
  enabled: true
  export_format: "yaml"
  versioning: "semantic"
"#;

    // Write config files
    tokio::fs::write(
        format!("{}/action-tiers.yml", config_path),
        action_tiers_content,
    )
    .await?;
    tokio::fs::write(
        format!("{}/economic-nodes.yml", config_path),
        economic_nodes_content,
    )
    .await?;
    tokio::fs::write(
        format!("{}/maintainers.yml", config_path),
        maintainers_content,
    )
    .await?;
    tokio::fs::write(format!("{}/repos.yml", config_path), repos_content).await?;
    tokio::fs::write(
        format!("{}/governance-fork.yml", config_path),
        governance_fork_content,
    )
    .await?;

    // Test export
    let exporter = GovernanceExporter::new(config_path);
    let export = exporter
        .export_governance_config(
            "test-ruleset-v1.0.0",
            &RulesetVersion::new(1, 0, 0),
            "test_exporter",
            "test-repo",
            "abc123def456",
        )
        .await?;

    assert_eq!(export.ruleset_id, "test-ruleset-v1.0.0");
    assert_eq!(export.ruleset_version.major, 1);
    assert_eq!(export.ruleset_version.minor, 0);
    assert_eq!(export.ruleset_version.patch, 0);
    assert_eq!(export.metadata.exported_by, "test_exporter");
    assert_eq!(export.metadata.source_repository, "test-repo");
    assert_eq!(export.metadata.commit_hash, "abc123def456");

    println!("✅ Governance config exported successfully");
    println!("   Ruleset ID: {}", export.ruleset_id);
    println!("   Version: {}", export.ruleset_version.to_string());
    println!("   Created at: {}", export.created_at);

    Ok(())
}

#[tokio::test]
async fn test_complete_governance_workflow() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing complete governance workflow...");

    // 1. Create database
    let _db = Database::new_in_memory().await?;
    println!("✅ Database created");

    // 2. Test PR classification
    let pr_payload = json!({
        "pull_request": {
            "number": 123,
            "title": "[FEATURE] Add new governance feature",
            "body": "This PR adds a new governance feature",
            "head": {"sha": "abc123"},
            "base": {"sha": "def456"}
        },
        "repository": {"full_name": "test-org/test-repo"}
    });

    let tier = tier_classification::classify_pr_tier(&pr_payload).await;
    assert_eq!(tier, 2);
    println!("✅ PR classified as Tier 2 (Feature)");

    // 3. Test status check generation
    use governance_app::enforcement::status_checks::StatusCheckGenerator;

    let opened_at = chrono::Utc::now() - chrono::Duration::days(5);
    let review_status = StatusCheckGenerator::generate_review_period_status(opened_at, 30, false);
    let signature_status = StatusCheckGenerator::generate_signature_status(
        4,
        4,
        5,
        &[
            "maintainer1".to_string(),
            "maintainer2".to_string(),
            "maintainer3".to_string(),
            "maintainer4".to_string(),
        ],
        &["maintainer5".to_string()],
    );
    let combined_status = StatusCheckGenerator::generate_combined_status(
        true,
        true,
        "Feature Changes",
        &review_status,
    );

    assert!(combined_status.contains("Feature Changes"));
    println!("✅ Status checks generated");

    // 4. Test merge blocking
    use governance_app::enforcement::merge_block::MergeBlocker;

    let should_block = MergeBlocker::should_block_merge(
        true,  // review period met
        true,  // signatures met
        false, // no economic veto
        tier,  // tier
        false, // emergency mode
    )
    .unwrap();

    assert!(!should_block);
    println!("✅ PR can be merged when all requirements met");

    println!("🎉 Complete governance workflow test passed!");
    Ok(())
}
