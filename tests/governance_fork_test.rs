//! Governance Fork Capability Tests
//!
//! Tests for governance configuration export, ruleset versioning,
//! adoption tracking, and multiple ruleset support

use governance_app::database::Database;
use governance_app::error::GovernanceError;
use governance_app::fork::{
    adoption::AdoptionTracker, export::GovernanceExporter, types::*, versioning::RulesetVersioning,
};
use serde_json::json;
use std::str::FromStr;

#[tokio::test]
async fn test_governance_config_export() -> Result<(), Box<dyn std::error::Error>> {
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
    assert_eq!(export.version.major, 1);
    assert_eq!(export.version.minor, 0);
    assert_eq!(export.version.patch, 0);
    assert!(!export.config_hash.is_empty());
    assert_eq!(export.metadata.exported_by, "test_exporter");
    assert_eq!(export.metadata.source_repository, "test-repo");
    assert_eq!(export.metadata.commit_hash, "abc123def456");

    println!("✅ Governance config exported successfully");
    println!("   Ruleset ID: {}", export.ruleset_id);
    println!("   Version: {}", export.version);
    println!("   Config Hash: {}", export.config_hash);

    Ok(())
}

#[tokio::test]
async fn test_ruleset_versioning() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::new_in_memory().await?;
    let versioning = RulesetVersioning::new(db.pool().clone());

    // Test initial ruleset creation
    let config_data = json!({
        "tiers": [
            {
                "name": "Routine Maintenance",
                "tier": 1,
                "signatures_required": 3,
                "signatures_total": 5,
                "review_period_days": 7
            }
        ]
    });

    let ruleset = versioning
        .create_ruleset(
            "test-ruleset",
            config_data,
            Some(RulesetVersion::new(1, 0, 0)),
        )
        .await?;

    assert_eq!(ruleset.ruleset_id, "test-ruleset");
    assert_eq!(ruleset.version.major, 1);
    assert_eq!(ruleset.version.minor, 0);
    assert_eq!(ruleset.version.patch, 0);
    assert_eq!(ruleset.status, "pending");
    println!("✅ Initial ruleset created: {}", ruleset.ruleset_id);

    // Test version increment
    let patch_version =
        versioning.version_ruleset(Some(&ruleset.version), VersionChangeType::Patch)?;
    assert_eq!(patch_version.major, 1);
    assert_eq!(patch_version.minor, 0);
    assert_eq!(patch_version.patch, 1);
    println!("✅ Patch version incremented: {}", patch_version);

    let minor_version =
        versioning.version_ruleset(Some(&ruleset.version), VersionChangeType::Minor)?;
    assert_eq!(minor_version.major, 1);
    assert_eq!(minor_version.minor, 1);
    assert_eq!(minor_version.patch, 0);
    println!("✅ Minor version incremented: {}", minor_version);

    let major_version =
        versioning.version_ruleset(Some(&ruleset.version), VersionChangeType::Major)?;
    assert_eq!(major_version.major, 2);
    assert_eq!(major_version.minor, 0);
    assert_eq!(major_version.patch, 0);
    println!("✅ Major version incremented: {}", major_version);

    // Test version comparison
    let v1 = RulesetVersion::new(1, 0, 0);
    let v2 = RulesetVersion::new(1, 1, 0);
    let v3 = RulesetVersion::new(2, 0, 0);

    assert_eq!(
        RulesetVersioning::compare_versions(&v1, &v1),
        VersionComparison::Equal
    );
    assert_eq!(
        RulesetVersioning::compare_versions(&v1, &v2),
        VersionComparison::Older
    );
    assert_eq!(
        RulesetVersioning::compare_versions(&v2, &v1),
        VersionComparison::Newer
    );
    assert_eq!(
        RulesetVersioning::compare_versions(&v1, &v3),
        VersionComparison::Older
    );
    println!("✅ Version comparison working correctly");

    Ok(())
}

#[tokio::test]
async fn test_adoption_tracking() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::new_in_memory().await?;
    let tracker = AdoptionTracker::new(db.pool().clone());

    // Record fork decisions
    tracker
        .record_fork_decision(
            1, // node_id
            "ruleset-v1.0.0",
            "adopt",
            "test_signature_1",
            Some("This ruleset is better"),
        )
        .await?;

    tracker
        .record_fork_decision(
            2, // node_id
            "ruleset-v1.0.0",
            "support",
            "test_signature_2",
            Some("Supporting this ruleset"),
        )
        .await?;

    tracker
        .record_fork_decision(
            3, // node_id
            "ruleset-v1.1.0",
            "adopt",
            "test_signature_3",
            Some("Newer version is better"),
        )
        .await?;

    println!("✅ Fork decisions recorded");

    // Calculate adoption metrics
    let metrics = tracker.calculate_adoption_metrics("ruleset-v1.0.0").await?;
    assert_eq!(metrics.ruleset_id, "ruleset-v1.0.0");
    assert!(metrics.total_nodes > 0);
    println!(
        "✅ Adoption metrics calculated for ruleset-v1.0.0: {} nodes",
        metrics.total_nodes
    );

    let metrics_v2 = tracker.calculate_adoption_metrics("ruleset-v1.1.0").await?;
    assert_eq!(metrics_v2.ruleset_id, "ruleset-v1.1.0");
    assert!(metrics_v2.total_nodes > 0);
    println!(
        "✅ Adoption metrics calculated for ruleset-v1.1.0: {} nodes",
        metrics_v2.total_nodes
    );

    // Get adoption statistics
    let stats = tracker.get_adoption_statistics().await?;
    assert!(stats.total_nodes > 0);
    assert!(stats.rulesets.len() > 0);
    println!(
        "✅ Adoption statistics retrieved: {} total nodes, {} rulesets",
        stats.total_nodes,
        stats.rulesets.len()
    );

    Ok(())
}

#[tokio::test]
async fn test_ruleset_retrieval() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::new_in_memory().await?;
    let versioning = RulesetVersioning::new(db.pool().clone());

    // Create a ruleset
    let config_data = json!({
        "tiers": [
            {
                "name": "Routine Maintenance",
                "tier": 1,
                "signatures_required": 3,
                "signatures_total": 5,
                "review_period_days": 7
            }
        ]
    });

    let ruleset = versioning
        .create_ruleset(
            "test-ruleset-retrieval",
            config_data,
            Some(RulesetVersion::new(1, 0, 0)),
        )
        .await?;

    // Retrieve the ruleset
    let retrieved = versioning
        .get_ruleset_by_id("test-ruleset-retrieval")
        .await?;
    assert!(retrieved.is_some());

    let ruleset = retrieved.unwrap();
    assert_eq!(ruleset.ruleset_id, "test-ruleset-retrieval");
    assert_eq!(ruleset.version.major, 1);
    assert_eq!(ruleset.version.minor, 0);
    assert_eq!(ruleset.version.patch, 0);
    println!("✅ Ruleset retrieved successfully: {}", ruleset.ruleset_id);

    // Test non-existent ruleset
    let non_existent = versioning.get_ruleset_by_id("non-existent-ruleset").await?;
    assert!(non_existent.is_none());
    println!("✅ Non-existent ruleset correctly returns None");

    Ok(())
}

#[tokio::test]
async fn test_ruleset_status_update() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::new_in_memory().await?;
    let versioning = RulesetVersioning::new(db.pool().clone());

    // Create a ruleset
    let config_data = json!({
        "tiers": [
            {
                "name": "Routine Maintenance",
                "tier": 1,
                "signatures_required": 3,
                "signatures_total": 5,
                "review_period_days": 7
            }
        ]
    });

    let ruleset = versioning
        .create_ruleset(
            "test-ruleset-status",
            config_data,
            Some(RulesetVersion::new(1, 0, 0)),
        )
        .await?;

    assert_eq!(ruleset.status, "pending");
    println!("✅ Ruleset created with pending status");

    // Update status to active
    versioning
        .update_ruleset_status("test-ruleset-status", "active")
        .await?;
    println!("✅ Ruleset status updated to active");

    // Verify status update
    let updated = versioning.get_ruleset_by_id("test-ruleset-status").await?;
    assert!(updated.is_some());
    let ruleset = updated.unwrap();
    assert_eq!(ruleset.status, "active");
    assert!(ruleset.activated_at.is_some());
    println!("✅ Ruleset status verified as active");

    Ok(())
}

#[tokio::test]
async fn test_adoption_history() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::new_in_memory().await?;
    let tracker = AdoptionTracker::new(db.pool().clone());

    // Record multiple fork decisions over time
    tracker
        .record_fork_decision(
            1,
            "ruleset-v1.0.0",
            "adopt",
            "test_signature_1",
            Some("Initial adoption"),
        )
        .await?;

    tracker
        .record_fork_decision(
            2,
            "ruleset-v1.0.0",
            "support",
            "test_signature_2",
            Some("Supporting adoption"),
        )
        .await?;

    // Get adoption history
    let history = tracker.get_adoption_history("ruleset-v1.0.0", 10).await?;
    assert!(history.len() > 0);
    println!("✅ Adoption history retrieved: {} entries", history.len());

    // Test with limit
    let limited_history = tracker.get_adoption_history("ruleset-v1.0.0", 1).await?;
    assert!(limited_history.len() <= 1);
    println!(
        "✅ Limited adoption history retrieved: {} entries",
        limited_history.len()
    );

    Ok(())
}

#[tokio::test]
async fn test_version_parsing() -> Result<(), Box<dyn std::error::Error>> {
    // Test valid version strings
    let v1 = RulesetVersion::from_str("1.0.0")?;
    assert_eq!(v1.major, 1);
    assert_eq!(v1.minor, 0);
    assert_eq!(v1.patch, 0);

    let v2 = RulesetVersion::from_str("v2.1.3")?;
    assert_eq!(v2.major, 2);
    assert_eq!(v2.minor, 1);
    assert_eq!(v2.patch, 3);

    println!("✅ Version parsing working correctly");

    // Test invalid version strings
    assert!(RulesetVersion::from_str("invalid").is_err());
    assert!(RulesetVersion::from_str("1.0").is_err());
    assert!(RulesetVersion::from_str("1.0.0.0").is_err());

    println!("✅ Invalid version strings correctly rejected");

    Ok(())
}

#[tokio::test]
async fn test_config_hash_calculation() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::new_in_memory().await?;
    let versioning = RulesetVersioning::new(db.pool().clone());

    let config1 = json!({
        "tiers": [
            {
                "name": "Routine Maintenance",
                "tier": 1,
                "signatures_required": 3,
                "signatures_total": 5,
                "review_period_days": 7
            }
        ]
    });

    let config2 = json!({
        "tiers": [
            {
                "name": "Routine Maintenance",
                "tier": 1,
                "signatures_required": 3,
                "signatures_total": 5,
                "review_period_days": 7
            }
        ]
    });

    let config3 = json!({
        "tiers": [
            {
                "name": "Routine Maintenance",
                "tier": 1,
                "signatures_required": 4, // Different value
                "signatures_total": 5,
                "review_period_days": 7
            }
        ]
    });

    let hash1 = versioning.calculate_config_hash(&config1)?;
    let hash2 = versioning.calculate_config_hash(&config2)?;
    let hash3 = versioning.calculate_config_hash(&config3)?;

    // Identical configs should have same hash
    assert_eq!(hash1, hash2);
    println!("✅ Identical configs produce same hash");

    // Different configs should have different hashes
    assert_ne!(hash1, hash3);
    println!("✅ Different configs produce different hashes");

    // Hashes should be valid hex strings
    assert!(hash1.len() == 64); // SHA256 produces 64-character hex string
    assert!(hash1.chars().all(|c| c.is_ascii_hexdigit()));
    println!("✅ Config hash is valid SHA256 hex string");

    Ok(())
}
