use governance_app::enforcement::*;
use governance_app::enforcement::status_checks::StatusCheckGenerator;
use governance_app::enforcement::merge_block::MergeBlocker;
use governance_app::validation::emergency::*;
use chrono::{DateTime, Utc, Duration};

mod common;
use common::*;

#[tokio::test]
async fn test_status_check_generation() {
    let opened_at = Utc::now() - Duration::days(100);
    let required_days = 90;
    let emergency_mode = false;
    
    let status = StatusCheckGenerator::generate_review_period_status(
        opened_at,
        required_days,
        emergency_mode,
    );
    
    assert!(status.contains("✅ Governance: Review Period Met"));
}

#[tokio::test]
async fn test_status_check_insufficient_review_period() {
    let opened_at = Utc::now() - Duration::days(10);
    let required_days = 90;
    let emergency_mode = false;
    
    let status = StatusCheckGenerator::generate_review_period_status(
        opened_at,
        required_days,
        emergency_mode,
    );
    
    assert!(status.contains("❌ Governance: Review Period Not Met"));
    assert!(status.contains("Required: 90 days"));
    assert!(status.contains("Elapsed: 10 days"));
}

#[tokio::test]
async fn test_status_check_emergency_mode() {
    let opened_at = Utc::now() - Duration::days(35);
    let required_days = 90;
    let emergency_mode = true;
    
    let status = StatusCheckGenerator::generate_review_period_status(
        opened_at,
        required_days,
        emergency_mode,
    );
    
    assert!(status.contains("✅ Governance: Review Period Met"));
}

#[tokio::test]
async fn test_signature_status_complete() {
    let current_signatures = 5;
    let required_signatures = 4;
    let total_maintainers = 7;
    let signers = vec!["alice".to_string(), "bob".to_string(), "charlie".to_string(), "dave".to_string(), "eve".to_string()];
    let pending = vec!["frank".to_string(), "grace".to_string()];
    
    let status = StatusCheckGenerator::generate_signature_status(
        current_signatures,
        required_signatures,
        total_maintainers,
        &signers,
        &pending,
    );
    
    assert!(status.contains("✅ Governance: Signatures Complete"));
}

#[tokio::test]
async fn test_signature_status_incomplete() {
    let current_signatures = 2;
    let required_signatures = 4;
    let total_maintainers = 7;
    let signers = vec!["alice".to_string(), "bob".to_string()];
    let pending = vec!["charlie".to_string(), "dave".to_string(), "eve".to_string(), "frank".to_string(), "grace".to_string()];
    
    let status = StatusCheckGenerator::generate_signature_status(
        current_signatures,
        required_signatures,
        total_maintainers,
        &signers,
        &pending,
    );
    
    assert!(status.contains("❌ Governance: Signatures Missing"));
    assert!(status.contains("Required: 4-of-7"));
    assert!(status.contains("Current: 2/7"));
    assert!(status.contains("alice, bob"));
    assert!(status.contains("charlie, dave, eve, frank, grace"));
}

#[tokio::test]
async fn test_combined_status_all_met() {
    let review_period_met = true;
    let signatures_met = true;
    let review_period_status = "✅ Governance: Review Period Met";
    let signature_status = "✅ Governance: Signatures Complete";
    
    let status = StatusCheckGenerator::generate_combined_status(
        review_period_met,
        signatures_met,
        review_period_status,
        signature_status,
    );
    
    assert!(status.contains("✅ Governance: All Requirements Met - Ready to Merge"));
}

#[tokio::test]
async fn test_combined_status_not_met() {
    let review_period_met = false;
    let signatures_met = false;
    let review_period_status = "❌ Governance: Review Period Not Met";
    let signature_status = "❌ Governance: Signatures Missing";
    
    let status = StatusCheckGenerator::generate_combined_status(
        review_period_met,
        signatures_met,
        review_period_status,
        signature_status,
    );
    
    assert!(status.contains("❌ Governance: Requirements Not Met"));
    assert!(status.contains("Review Period Not Met"));
    assert!(status.contains("Signatures Missing"));
}

#[tokio::test]
async fn test_emergency_status_generation() {
    let emergency = ActiveEmergency {
        id: 1,
        tier: EmergencyTier::Critical,
        activated_by: "emergency_alice".to_string(),
        reason: "Critical security vulnerability".to_string(),
        activated_at: Utc::now() - Duration::days(2),
        expires_at: Utc::now() + Duration::days(5),
        extended: false,
        extension_count: 0,
    };
    
    let status = StatusCheckGenerator::generate_emergency_status(&emergency);
    
    assert!(status.contains("🚨 Emergency Tier Active: Critical Emergency"));
    assert!(status.contains("4-of-7 signatures"));
    assert!(status.contains("0 day review period"));
    assert!(status.contains("Critical security vulnerability"));
    assert!(status.contains("emergency_alice"));
}

#[tokio::test]
async fn test_emergency_expiration_warning() {
    let emergency = ActiveEmergency {
        id: 1,
        tier: EmergencyTier::Urgent,
        activated_by: "emergency_alice".to_string(),
        reason: "Urgent security issue".to_string(),
        activated_at: Utc::now() - Duration::days(20),
        expires_at: Utc::now() + Duration::hours(12), // Less than 24 hours
        extended: false,
        extension_count: 0,
    };
    
    let warning = StatusCheckGenerator::generate_emergency_expiration_warning(&emergency);
    
    assert!(warning.contains("⚠️ ⚠️ Emergency Tier Expiring Soon"));
    assert!(warning.contains("Less than 24 hours remaining"));
}

#[tokio::test]
async fn test_emergency_extension_info() {
    let emergency = ActiveEmergency {
        id: 1,
        tier: EmergencyTier::Elevated,
        activated_by: "emergency_alice".to_string(),
        reason: "Elevated priority issue".to_string(),
        activated_at: Utc::now() - Duration::days(10),
        expires_at: Utc::now() + Duration::days(20),
        extended: false,
        extension_count: 0,
    };
    
    let status = StatusCheckGenerator::generate_emergency_status(&emergency);
    
    assert!(status.contains("📢 Emergency Tier Active: Elevated Priority"));
    assert!(status.contains("Extensions: 0 of 2 used"));
    assert!(status.contains("can extend by 30 days"));
}

#[tokio::test]
async fn test_combined_status_with_emergency() {
    let emergency = ActiveEmergency {
        id: 1,
        tier: EmergencyTier::Urgent,
        activated_by: "emergency_alice".to_string(),
        reason: "Urgent security issue".to_string(),
        activated_at: Utc::now() - Duration::days(5),
        expires_at: Utc::now() + Duration::days(25),
        extended: false,
        extension_count: 0,
    };
    
    let status = StatusCheckGenerator::generate_combined_status_with_emergency(
        true, // review_period_met
        true, // signatures_met
        "✅ Governance: Review Period Met",
        "✅ Governance: Signatures Complete",
        Some(&emergency),
    );
    
    assert!(status.contains("⚠️ Emergency Tier Active: Urgent Security Issue"));
    assert!(status.contains("✅ Governance: All Requirements Met - Ready to Merge"));
}

#[tokio::test]
async fn test_post_emergency_requirements() {
    let tier = EmergencyTier::Critical;
    let post_mortem_published = false;
    let post_mortem_deadline = Utc::now() + Duration::days(25);
    let security_audit_completed = false;
    let security_audit_deadline = Some(Utc::now() + Duration::days(55));
    
    let status = StatusCheckGenerator::generate_post_emergency_requirements(
        tier,
        post_mortem_published,
        post_mortem_deadline,
        security_audit_completed,
        security_audit_deadline,
    );
    
    assert!(status.contains("📋 Post-Emergency Requirements for Critical Emergency"));
    assert!(status.contains("⏳ Post-mortem pending"));
    assert!(status.contains("⏳ Security audit pending"));
}

#[tokio::test]
async fn test_merge_blocker_normal_mode() {
    // Test normal mode - both requirements must be met
    let result = MergeBlocker::should_block_merge(true, true, false);
    assert!(result.is_ok());
    assert!(!result.unwrap()); // Should not block
    
    let result = MergeBlocker::should_block_merge(false, true, false);
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should block
    
    let result = MergeBlocker::should_block_merge(true, false, false);
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should block
    
    let result = MergeBlocker::should_block_merge(false, false, false);
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should block
}

#[tokio::test]
async fn test_merge_blocker_emergency_mode() {
    // Test emergency mode - only signatures matter
    let result = MergeBlocker::should_block_merge(false, true, true);
    assert!(result.is_ok());
    assert!(!result.unwrap()); // Should not block (signatures met)
    
    let result = MergeBlocker::should_block_merge(true, false, true);
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should block (signatures not met)
    
    let result = MergeBlocker::should_block_merge(false, false, true);
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should block (signatures not met)
}

#[tokio::test]
async fn test_merge_blocker_reasons() {
    // Test normal mode reasons
    let reason = MergeBlocker::get_block_reason(false, false, false);
    assert!(reason.contains("Both review period and signature requirements not met"));
    
    let reason = MergeBlocker::get_block_reason(false, true, false);
    assert!(reason.contains("Review period requirement not met"));
    
    let reason = MergeBlocker::get_block_reason(true, false, false);
    assert!(reason.contains("Signature threshold requirement not met"));
    
    let reason = MergeBlocker::get_block_reason(true, true, false);
    assert!(reason.contains("All governance requirements met"));
    
    // Test emergency mode reasons
    let reason = MergeBlocker::get_block_reason(false, false, true);
    assert!(reason.contains("Emergency mode: Signature threshold not met"));
    
    let reason = MergeBlocker::get_block_reason(false, true, true);
    assert!(reason.contains("Emergency mode: All requirements met"));
}

#[tokio::test]
async fn test_status_check_edge_cases() {
    // Test exactly at boundary
    let opened_at = Utc::now() - Duration::days(90);
    let required_days = 90;
    let emergency_mode = false;
    
    let status = StatusCheckGenerator::generate_review_period_status(
        opened_at,
        required_days,
        emergency_mode,
    );
    
    assert!(status.contains("✅ Governance: Review Period Met"));
    
    // Test one day before boundary
    let opened_at = Utc::now() - Duration::days(89);
    let status = StatusCheckGenerator::generate_review_period_status(
        opened_at,
        required_days,
        emergency_mode,
    );
    
    assert!(status.contains("❌ Governance: Review Period Not Met"));
}

#[tokio::test]
async fn test_emergency_tier_display_properties() {
    // Test Critical tier
    assert_eq!(EmergencyTier::Critical.emoji(), "🚨");
    assert_eq!(EmergencyTier::Critical.name(), "Critical Emergency");
    assert!(EmergencyTier::Critical.description().contains("Network-threatening"));
    
    // Test Urgent tier
    assert_eq!(EmergencyTier::Urgent.emoji(), "⚠️");
    assert_eq!(EmergencyTier::Urgent.name(), "Urgent Security Issue");
    assert!(EmergencyTier::Urgent.description().contains("Serious security"));
    
    // Test Elevated tier
    assert_eq!(EmergencyTier::Elevated.emoji(), "📢");
    assert_eq!(EmergencyTier::Elevated.name(), "Elevated Priority");
    assert!(EmergencyTier::Elevated.description().contains("Important priority"));
}

#[tokio::test]
async fn test_emergency_extension_calculations() {
    let emergency = ActiveEmergency {
        id: 1,
        tier: EmergencyTier::Urgent,
        activated_by: "emergency_alice".to_string(),
        reason: "Test".to_string(),
        activated_at: Utc::now() - Duration::days(5),
        expires_at: Utc::now() + Duration::days(25),
        extended: false,
        extension_count: 0,
    };
    
    // Test extension calculation
    let new_expiration = emergency.calculate_extension_expiration();
    assert!(new_expiration.is_some());
    
    let expected_expiration = emergency.expires_at + Duration::days(30);
    assert_eq!(new_expiration.unwrap(), expected_expiration);
    
    // Test max extensions reached
    let maxed_emergency = ActiveEmergency {
        extension_count: 1, // Urgent allows only 1 extension
        ..emergency
    };
    
    assert!(!maxed_emergency.can_extend());
    assert!(maxed_emergency.calculate_extension_expiration().is_none());
}











