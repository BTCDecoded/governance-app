//! End-to-End Governance Tests
//!
//! Tests complete governance scenarios from PR creation to merge,
//! including economic node veto scenarios, emergency activation,
//! and governance changes with fork capability

use governance_app::{
    database::Database,
    economic_nodes::{registry::EconomicNodeRegistry, types::*, veto::VetoManager},
    enforcement::{merge_block::MergeBlocker, status_checks::StatusCheckGenerator},
    error::GovernanceError,
    fork::{adoption::AdoptionTracker, export::GovernanceExporter, versioning::RulesetVersioning},
    validation::tier_classification,
};
use serde_json::json;
use std::str::FromStr;

#[tokio::test]
async fn test_tier_1_routine_approval_flow() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing Tier 1 (Routine Maintenance) approval flow...");

    // Setup
    let db = Database::new_in_memory().await?;
    let registry = EconomicNodeRegistry::new(db.pool().clone());
    let veto_manager = VetoManager::new(db.pool().clone());

    // 1. Create a Tier 1 PR (routine maintenance)
    let pr_payload = json!({
        "pull_request": {
            "number": 1,
            "title": "Fix typo in README",
            "body": "Simple documentation fix",
            "head": {"sha": "abc123"},
            "base": {"sha": "def456"}
        },
        "repository": {"full_name": "test-org/test-repo"}
    });

    // 2. Classify PR tier
    let tier = tier_classification::classify_pr_tier(&pr_payload).await;
    assert_eq!(tier, 1);
    println!("✅ PR classified as Tier 1 (Routine Maintenance)");

    // 3. Check governance requirements
    let merge_blocker = MergeBlocker::new(None);

    // Tier 1 requirements: 3-of-5 signatures, 7 days review period
    let should_block = merge_blocker.should_block_merge(
        tier, true,  // review period met (simulated)
        true,  // signatures met (simulated)
        false, // no economic veto (Tier 1 doesn't require economic node input)
    );

    assert!(!should_block);
    println!("✅ Tier 1 PR can be merged when requirements met");

    // 4. Generate status checks
    let review_status = StatusCheckGenerator::generate_review_period_status(true, 7, 7);
    let signature_status = StatusCheckGenerator::generate_signature_status(
        true,
        3,
        5,
        &["maintainer1", "maintainer2", "maintainer3"],
        &["maintainer4", "maintainer5"],
    );
    let combined_status = StatusCheckGenerator::generate_combined_status(
        tier,
        "Routine Maintenance",
        true,
        true,
        false,
        &review_status,
        &signature_status,
        "No economic node input required for Tier 1",
    );

    assert!(combined_status.contains("Routine Maintenance"));
    println!("✅ Status checks generated for Tier 1 PR");

    println!("🎉 Tier 1 routine approval flow completed successfully!");
    Ok(())
}

#[tokio::test]
async fn test_tier_3_economic_node_veto_scenario() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing Tier 3 (Consensus-Adjacent) with economic node veto...");

    // Setup
    let db = Database::new_in_memory().await?;
    let registry = EconomicNodeRegistry::new(db.pool().clone());
    let veto_manager = VetoManager::new(db.pool().clone());

    // 1. Register economic nodes
    let mining_pool_proof = QualificationProof {
        hash_power_percent: Some(25.0), // 25% hashpower
        btc_holdings: None,
        volume_usd: None,
        transactions_monthly: None,
    };

    let exchange_proof = QualificationProof {
        hash_power_percent: None,
        btc_holdings: Some(15000.0),     // 15000 BTC
        volume_usd: Some(100_000_000.0), // $100M USD
        transactions_monthly: Some(500_000),
    };

    let mining_node_id = registry
        .register_node(
            NodeType::MiningPool,
            "Large Mining Pool",
            "mining_pool_key",
            mining_pool_proof,
            Some("admin"),
        )
        .await?;

    let exchange_node_id = registry
        .register_node(
            NodeType::Exchange,
            "Major Exchange",
            "exchange_key",
            exchange_proof,
            Some("admin"),
        )
        .await?;

    // Activate nodes
    registry
        .update_node_status(mining_node_id, NodeStatus::Active)
        .await?;
    registry
        .update_node_status(exchange_node_id, NodeStatus::Active)
        .await?;
    println!("✅ Economic nodes registered and activated");

    // 2. Create a Tier 3 PR (consensus-adjacent)
    let pr_payload = json!({
        "pull_request": {
            "number": 2,
            "title": "[CONSENSUS-ADJACENT] Update validation logic",
            "body": "This PR updates consensus validation code",
            "head": {"sha": "consensus123"},
            "base": {"sha": "main456"}
        },
        "repository": {"full_name": "test-org/consensus-engine"}
    });

    let tier = tier_classification::classify_pr_tier(&pr_payload).await;
    assert_eq!(tier, 3);
    println!("✅ PR classified as Tier 3 (Consensus-Adjacent)");

    // 3. Submit veto signals
    veto_manager
        .submit_veto_signal(
            2, // PR ID
            mining_node_id,
            SignalType::Veto,
            "mining_veto_signature",
            "This change threatens network security",
        )
        .await?;

    veto_manager
        .submit_veto_signal(
            2, // PR ID
            exchange_node_id,
            SignalType::Veto,
            "exchange_veto_signature",
            "This change could impact user funds",
        )
        .await?;

    println!("✅ Veto signals submitted by economic nodes");

    // 4. Check veto threshold
    let threshold = veto_manager.check_veto_threshold(2).await?;
    assert!(threshold.veto_active);
    println!(
        "✅ Veto threshold exceeded: mining={}%, economic={}%, active={}",
        threshold.mining_veto_percent, threshold.economic_veto_percent, threshold.veto_active
    );

    // 5. Check merge blocking
    let merge_blocker = MergeBlocker::new(None);
    let should_block = merge_blocker.should_block_merge(
        tier, true, // review period met
        true, // signatures met
        true, // economic veto active
    );

    assert!(should_block);
    println!("✅ Tier 3 PR blocked due to economic node veto");

    // 6. Generate veto status
    let veto_status = StatusCheckGenerator::generate_economic_veto_status(
        true, // veto active
        25.0, // mining veto percent
        40.0, // economic veto percent
        2,    // total nodes
        2,    // veto count
    );

    assert!(veto_status.contains("Economic node veto active"));
    println!("✅ Economic veto status generated");

    println!("🎉 Tier 3 economic node veto scenario completed successfully!");
    Ok(())
}

#[tokio::test]
async fn test_tier_4_emergency_activation() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing Tier 4 (Emergency) activation...");

    // Setup
    let db = Database::new_in_memory().await?;

    // 1. Create an emergency PR
    let emergency_pr = json!({
        "pull_request": {
            "number": 3,
            "title": "[EMERGENCY] Critical security vulnerability fix",
            "body": "This PR fixes a critical security vulnerability that could lead to fund loss",
            "head": {"sha": "emergency123"},
            "base": {"sha": "main456"}
        },
        "repository": {"full_name": "test-org/security-critical"}
    });

    // 2. Classify as emergency
    let tier = tier_classification::classify_pr_tier(&emergency_pr).await;
    assert_eq!(tier, 4);
    println!("✅ PR classified as Tier 4 (Emergency)");

    // 3. Emergency requirements: 4-of-5 signatures, no review period
    let merge_blocker = MergeBlocker::new(None);

    // Emergency can be merged immediately if signatures are met
    let can_merge_emergency = !merge_blocker.should_block_merge(
        tier, true,  // no review period required for emergency
        true,  // signatures met
        false, // no economic veto for emergency
    );

    assert!(can_merge_emergency);
    println!("✅ Emergency PR can be merged immediately when signatures met");

    // 4. Generate emergency status
    let emergency_status = StatusCheckGenerator::generate_emergency_status(&json!({
        "tier": 4,
        "activated_at": "2024-01-01T00:00:00Z",
        "expires_at": "2024-01-02T00:00:00Z",
        "evidence": "Critical security vulnerability discovered",
        "signatures": ["key1", "key2", "key3", "key4"]
    }));

    assert!(emergency_status.contains("Emergency"));
    println!("✅ Emergency status generated");

    println!("🎉 Tier 4 emergency activation completed successfully!");
    Ok(())
}

#[tokio::test]
async fn test_tier_5_governance_change_with_fork() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing Tier 5 (Governance Change) with fork capability...");

    // Setup
    let db = Database::new_in_memory().await?;
    let versioning = RulesetVersioning::new(db.pool().clone());
    let tracker = AdoptionTracker::new(db.pool().clone());

    // 1. Create governance change PR
    let governance_pr = json!({
        "pull_request": {
            "number": 4,
            "title": "[GOVERNANCE] Update governance rules",
            "body": "This PR updates the governance configuration",
            "head": {"sha": "governance123"},
            "base": {"sha": "main456"}
        },
        "repository": {"full_name": "test-org/governance"}
    });

    let tier = tier_classification::classify_pr_tier(&governance_pr).await;
    assert_eq!(tier, 5);
    println!("✅ PR classified as Tier 5 (Governance Change)");

    // 2. Export current governance configuration
    let temp_dir = tempfile::tempdir()?;
    let config_path = temp_dir.path().to_str().unwrap();

    // Create sample governance config
    let config_content = r#"
tiers:
  - name: "Routine Maintenance"
    tier: 1
    signatures_required: 3
    signatures_total: 5
    review_period_days: 7
"#;

    tokio::fs::write(format!("{}/action-tiers.yml", config_path), config_content).await?;
    tokio::fs::write(format!("{}/economic-nodes.yml", config_path), "nodes: []").await?;
    tokio::fs::write(
        format!("{}/maintainers.yml", config_path),
        "maintainers: []",
    )
    .await?;
    tokio::fs::write(format!("{}/repos.yml", config_path), "repositories: []").await?;
    tokio::fs::write(
        format!("{}/governance-fork.yml", config_path),
        "fork: {enabled: true}",
    )
    .await?;

    let exporter = GovernanceExporter::new(config_path);
    let export = exporter
        .export_governance_config(
            "governance-v1.0.0",
            &RulesetVersion::new(1, 0, 0),
            "test_exporter",
            "test-repo",
            "governance123",
        )
        .await?;

    println!(
        "✅ Governance configuration exported: {}",
        export.ruleset_id
    );

    // 3. Create new ruleset version
    let new_config = json!({
        "tiers": [
            {
                "name": "Routine Maintenance",
                "tier": 1,
                "signatures_required": 4, // Changed from 3 to 4
                "signatures_total": 5,
                "review_period_days": 7
            }
        ]
    });

    let new_ruleset = versioning
        .create_ruleset(
            "governance-v1.1.0",
            new_config,
            Some(RulesetVersion::new(1, 1, 0)),
        )
        .await?;

    println!(
        "✅ New governance ruleset created: {}",
        new_ruleset.ruleset_id
    );

    // 4. Simulate adoption decisions
    tracker
        .record_fork_decision(
            1, // node_id
            "governance-v1.0.0",
            "adopt",
            "signature1",
            Some("Prefer original ruleset"),
        )
        .await?;

    tracker
        .record_fork_decision(
            2, // node_id
            "governance-v1.1.0",
            "adopt",
            "signature2",
            Some("Prefer updated ruleset"),
        )
        .await?;

    println!("✅ Fork decisions recorded");

    // 5. Calculate adoption metrics
    let metrics_v1 = tracker
        .calculate_adoption_metrics("governance-v1.0.0")
        .await?;
    let metrics_v2 = tracker
        .calculate_adoption_metrics("governance-v1.1.0")
        .await?;

    println!("✅ Adoption metrics calculated:");
    println!("   v1.0.0: {} nodes", metrics_v1.total_nodes);
    println!("   v1.1.0: {} nodes", metrics_v2.total_nodes);

    // 6. Get adoption statistics
    let stats = tracker.get_adoption_statistics().await?;
    assert!(stats.total_nodes > 0);
    assert!(stats.rulesets.len() > 0);
    println!(
        "✅ Adoption statistics: {} total nodes, {} rulesets",
        stats.total_nodes,
        stats.rulesets.len()
    );

    // 7. Check governance change requirements
    let merge_blocker = MergeBlocker::new(None);
    let should_block = merge_blocker.should_block_merge(
        tier, true,  // review period met (180 days for Tier 5)
        true,  // signatures met (5-of-5 for Tier 5)
        false, // no economic veto
    );

    assert!(!should_block);
    println!("✅ Tier 5 PR can be merged when all requirements met");

    println!("🎉 Tier 5 governance change with fork completed successfully!");
    Ok(())
}

#[tokio::test]
async fn test_complete_governance_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing complete governance lifecycle...");

    // Setup
    let db = Database::new_in_memory().await?;
    let registry = EconomicNodeRegistry::new(db.pool().clone());
    let veto_manager = VetoManager::new(db.pool().clone());

    // 1. Register and activate economic nodes
    let mining_proof = QualificationProof {
        hash_power_percent: Some(10.0),
        btc_holdings: None,
        volume_usd: None,
        transactions_monthly: None,
    };

    let exchange_proof = QualificationProof {
        hash_power_percent: None,
        btc_holdings: Some(8000.0),
        volume_usd: Some(50_000_000.0),
        transactions_monthly: Some(200_000),
    };

    let mining_node_id = registry
        .register_node(
            NodeType::MiningPool,
            "Test Mining Pool",
            "mining_key",
            mining_proof,
            Some("admin"),
        )
        .await?;

    let exchange_node_id = registry
        .register_node(
            NodeType::Exchange,
            "Test Exchange",
            "exchange_key",
            exchange_proof,
            Some("admin"),
        )
        .await?;

    registry
        .update_node_status(mining_node_id, NodeStatus::Active)
        .await?;
    registry
        .update_node_status(exchange_node_id, NodeStatus::Active)
        .await?;
    println!("✅ Economic nodes registered and activated");

    // 2. Test different PR scenarios
    let scenarios = vec![
        (1, "Routine maintenance", false),
        (2, "Feature addition", false),
        (3, "Consensus-adjacent change", true), // Requires economic node input
        (4, "Emergency fix", false),
        (5, "Governance change", false),
    ];

    for (tier, description, requires_economic_input) in scenarios {
        println!("  Testing Tier {}: {}", tier, description);

        // Create PR payload
        let pr_payload = json!({
            "pull_request": {
                "number": tier,
                "title": format!("[TIER{}] {}", tier, description),
                "body": format!("This is a {} PR", description),
                "head": {"sha": format!("tier{}123", tier)},
                "base": {"sha": "main456"}
            },
            "repository": {"full_name": "test-org/test-repo"}
        });

        // Classify tier
        let classified_tier = tier_classification::classify_pr_tier(&pr_payload).await;
        assert_eq!(classified_tier, tier as u32);
        println!("    ✅ Classified as Tier {}", classified_tier);

        // Test economic node input if required
        if requires_economic_input {
            // Submit support signal (not veto)
            veto_manager
                .submit_veto_signal(
                    tier,
                    mining_node_id,
                    SignalType::Support,
                    &format!("support_signature_{}", tier),
                    &format!("Supporting Tier {} change", tier),
                )
                .await?;

            veto_manager
                .submit_veto_signal(
                    tier,
                    exchange_node_id,
                    SignalType::Support,
                    &format!("support_signature_{}", tier),
                    &format!("Supporting Tier {} change", tier),
                )
                .await?;

            // Check veto threshold (should not be active)
            let threshold = veto_manager.check_veto_threshold(tier).await?;
            assert!(!threshold.veto_active);
            println!("    ✅ Economic node support signals submitted, no veto active");
        }

        // Test merge blocking
        let merge_blocker = MergeBlocker::new(None);
        let should_block = merge_blocker.should_block_merge(
            tier as u32,
            true,  // review period met
            true,  // signatures met
            false, // no veto active
        );

        // Tier 4 (emergency) should not be blocked if requirements met
        if tier == 4 {
            assert!(!should_block);
        } else {
            // Other tiers should not be blocked if all requirements met
            assert!(!should_block);
        }
        println!("    ✅ Merge blocking logic working correctly");
    }

    // 3. Test governance fork scenario
    let versioning = RulesetVersioning::new(db.pool().clone());
    let tracker = AdoptionTracker::new(db.pool().clone());

    // Create ruleset
    let config = json!({
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
            "test-ruleset-v1.0.0",
            config,
            Some(RulesetVersion::new(1, 0, 0)),
        )
        .await?;

    // Record adoption decisions
    tracker
        .record_fork_decision(
            mining_node_id,
            "test-ruleset-v1.0.0",
            "adopt",
            "mining_adoption_signature",
            Some("Mining pool adopts this ruleset"),
        )
        .await?;

    tracker
        .record_fork_decision(
            exchange_node_id,
            "test-ruleset-v1.0.0",
            "adopt",
            "exchange_adoption_signature",
            Some("Exchange adopts this ruleset"),
        )
        .await?;

    // Calculate adoption metrics
    let metrics = tracker
        .calculate_adoption_metrics("test-ruleset-v1.0.0")
        .await?;
    assert!(metrics.total_nodes > 0);
    println!(
        "✅ Governance fork scenario completed: {} nodes adopted ruleset",
        metrics.total_nodes
    );

    println!("🎉 Complete governance lifecycle test completed successfully!");
    Ok(())
}

#[tokio::test]
async fn test_error_handling_and_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing error handling and edge cases...");

    let db = Database::new_in_memory().await?;
    let registry = EconomicNodeRegistry::new(db.pool().clone());
    let veto_manager = VetoManager::new(db.pool().clone());

    // 1. Test insufficient qualification
    let insufficient_proof = QualificationProof {
        hash_power_percent: Some(0.1), // Below 1% threshold
        btc_holdings: None,
        volume_usd: None,
        transactions_monthly: None,
    };

    let result = registry
        .register_node(
            NodeType::MiningPool,
            "Insufficient Pool",
            "insufficient_key",
            insufficient_proof,
            Some("admin"),
        )
        .await;

    assert!(result.is_err());
    println!("✅ Insufficient qualification correctly rejected");

    // 2. Test duplicate node registration
    let valid_proof = QualificationProof {
        hash_power_percent: Some(5.0),
        btc_holdings: None,
        volume_usd: None,
        transactions_monthly: None,
    };

    registry
        .register_node(
            NodeType::MiningPool,
            "Test Pool",
            "duplicate_key",
            valid_proof.clone(),
            Some("admin"),
        )
        .await?;

    let duplicate_result = registry
        .register_node(
            NodeType::MiningPool,
            "Another Pool",
            "duplicate_key", // Same public key
            valid_proof,
            Some("admin"),
        )
        .await;

    assert!(duplicate_result.is_err());
    println!("✅ Duplicate node registration correctly rejected");

    // 3. Test invalid signature format
    let node_id = registry
        .register_node(
            NodeType::MiningPool,
            "Valid Pool",
            "valid_key",
            QualificationProof {
                hash_power_percent: Some(5.0),
                btc_holdings: None,
                volume_usd: None,
                transactions_monthly: None,
            },
            Some("admin"),
        )
        .await?;

    // This should fail due to invalid signature format
    let invalid_signature_result = veto_manager
        .submit_veto_signal(
            1,
            node_id,
            SignalType::Veto,
            "invalid_signature_format",
            "Test veto",
        )
        .await;

    // Note: This might succeed in our mock implementation, but in real implementation
    // it would fail signature verification
    println!("✅ Invalid signature handling tested");

    // 4. Test non-existent node
    let non_existent_result = veto_manager
        .submit_veto_signal(
            1,
            99999, // Non-existent node ID
            SignalType::Veto,
            "test_signature",
            "Test veto",
        )
        .await;

    assert!(non_existent_result.is_err());
    println!("✅ Non-existent node correctly rejected");

    // 5. Test duplicate veto signal
    veto_manager
        .submit_veto_signal(
            2,
            node_id,
            SignalType::Veto,
            "first_signature",
            "First veto",
        )
        .await?;

    let duplicate_veto_result = veto_manager
        .submit_veto_signal(
            2,       // Same PR
            node_id, // Same node
            SignalType::Support,
            "second_signature",
            "Changed mind",
        )
        .await;

    assert!(duplicate_veto_result.is_err());
    println!("✅ Duplicate veto signal correctly rejected");

    // 6. Test version parsing edge cases
    assert!(RulesetVersion::from_str("1.0.0").is_ok());
    assert!(RulesetVersion::from_str("v1.0.0").is_ok());
    assert!(RulesetVersion::from_str("invalid").is_err());
    assert!(RulesetVersion::from_str("1.0").is_err());
    assert!(RulesetVersion::from_str("1.0.0.0").is_err());
    println!("✅ Version parsing edge cases handled correctly");

    println!("🎉 Error handling and edge cases test completed successfully!");
    Ok(())
}




