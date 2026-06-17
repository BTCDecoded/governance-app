use crate::validation::review_period::ReviewPeriodValidator;
use crate::validation::threshold::ThresholdValidator;
use chrono::{DateTime, Utc};

pub struct StatusCheckGenerator;

impl StatusCheckGenerator {
    pub fn generate_review_period_status(
        opened_at: DateTime<Utc>,
        required_days: i64,
        emergency_mode: bool,
    ) -> String {
        Self::generate_review_period_status_with_dry_run(
            opened_at,
            required_days,
            emergency_mode,
            false,
        )
    }

    pub fn generate_review_period_status_with_dry_run(
        opened_at: DateTime<Utc>,
        required_days: i64,
        emergency_mode: bool,
        dry_run: bool,
    ) -> String {
        let remaining_days =
            ReviewPeriodValidator::get_remaining_days(opened_at, required_days, emergency_mode);

        let prefix = if dry_run { "[DRY-RUN] " } else { "" };

        if remaining_days > 0 {
            let earliest_merge = ReviewPeriodValidator::get_earliest_merge_date(
                opened_at,
                required_days,
                emergency_mode,
            );

            format!(
                "{}❌ Governance: Review Period Not Met\nRequired: {} days | Elapsed: {} days\nEarliest merge: {}",
                prefix,
                required_days,
                (Utc::now() - opened_at).num_days(),
                earliest_merge.format("%Y-%m-%d")
            )
        } else {
            format!("{prefix}✅ Governance: Review Period Met")
        }
    }

    pub fn generate_signature_status(
        current_signatures: usize,
        required_signatures: usize,
        total_maintainers: usize,
        signers: &[String],
        pending: &[String],
    ) -> String {
        Self::generate_signature_status_with_dry_run(
            current_signatures,
            required_signatures,
            total_maintainers,
            signers,
            pending,
            false,
        )
    }

    pub fn generate_signature_status_with_dry_run(
        current_signatures: usize,
        required_signatures: usize,
        total_maintainers: usize,
        signers: &[String],
        pending: &[String],
        dry_run: bool,
    ) -> String {
        let prefix = if dry_run { "[DRY-RUN] " } else { "" };

        if current_signatures >= required_signatures {
            format!("{prefix}✅ Governance: Signatures Complete")
        } else {
            let base_status = ThresholdValidator::format_threshold_status(
                current_signatures,
                required_signatures,
                total_maintainers,
                signers,
                pending,
            );
            format!("{prefix}{base_status}")
        }
    }

    pub fn generate_combined_status(
        review_period_met: bool,
        signatures_met: bool,
        review_period_status: &str,
        signature_status: &str,
    ) -> String {
        if review_period_met && signatures_met {
            "✅ Governance: All Requirements Met - Ready to Merge".to_string()
        } else {
            format!(
                "❌ Governance: Requirements Not Met\n\n{review_period_status}\n\n{signature_status}"
            )
        }
    }

    /// Generate status check with tier classification
    pub fn generate_tier_status(
        tier: u32,
        tier_name: &str,
        review_period_met: bool,
        signatures_met: bool,
        review_period_status: &str,
        signature_status: &str,
    ) -> String {
        let tier_emoji = match tier {
            1 => "🔧", // Routine
            2 => "✨", // Feature
            3 => "⚡", // Consensus-Adjacent
            4 => "🚨", // Emergency
            5 => "🏛️", // Governance
            _ => "❓",
        };

        let mut status = format!("{tier_emoji} Tier {tier}: {tier_name}\n");

        if review_period_met && signatures_met {
            status.push_str("✅ Governance: All Requirements Met - Ready to Merge");
        } else {
            status.push_str("❌ Governance: Requirements Not Met\n");
            status.push_str(&format!("\n{review_period_status}\n\n{signature_status}"));
        }

        status
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_generate_review_period_status_met() {
        let opened_at = Utc::now() - Duration::days(10);
        let required_days = 7;

        let status =
            StatusCheckGenerator::generate_review_period_status(opened_at, required_days, false);

        assert!(status.contains("✅"), "Should show review period met");
    }

    #[test]
    fn test_generate_review_period_status_not_met() {
        let opened_at = Utc::now() - Duration::days(3);
        let required_days = 7;

        let status =
            StatusCheckGenerator::generate_review_period_status(opened_at, required_days, false);

        assert!(status.contains("❌"), "Should show review period not met");
        assert!(
            status.contains("Required: 7 days"),
            "Should show required days"
        );
    }

    #[test]
    fn test_generate_review_period_status_dry_run() {
        let opened_at = Utc::now() - Duration::days(3);
        let required_days = 7;

        let status = StatusCheckGenerator::generate_review_period_status_with_dry_run(
            opened_at,
            required_days,
            false,
            true, // dry_run
        );

        assert!(
            status.contains("[DRY-RUN]"),
            "Should include dry-run prefix"
        );
    }

    #[test]
    fn test_generate_signature_status_complete() {
        let status = StatusCheckGenerator::generate_signature_status(
            5, // current_signatures
            4, // required_signatures
            5, // total_maintainers
            &["alice".to_string(), "bob".to_string()],
            &[],
        );

        assert!(status.contains("✅"), "Should show signatures complete");
    }

    #[test]
    fn test_generate_signature_status_incomplete() {
        let status = StatusCheckGenerator::generate_signature_status(
            2, // current_signatures
            4, // required_signatures
            5, // total_maintainers
            &["alice".to_string(), "bob".to_string()],
            &["charlie".to_string()],
        );

        assert!(
            !status.contains("✅ Governance: Signatures Complete"),
            "Should not show complete when threshold not met"
        );
    }

    #[test]
    fn test_generate_signature_status_dry_run() {
        let status = StatusCheckGenerator::generate_signature_status_with_dry_run(
            5,
            4,
            5,
            &["alice".to_string()],
            &[],
            true, // dry_run
        );

        assert!(
            status.contains("[DRY-RUN]"),
            "Should include dry-run prefix"
        );
    }

    #[test]
    fn test_generate_combined_status_all_met() {
        let status = StatusCheckGenerator::generate_combined_status(
            true, // review_period_met
            true, // signatures_met
            "Review period met",
            "Signatures complete",
        );

        assert!(status.contains("✅"), "Should show all requirements met");
        assert!(
            status.contains("Ready to Merge"),
            "Should indicate ready to merge"
        );
    }

    #[test]
    fn test_generate_combined_status_not_met() {
        let status = StatusCheckGenerator::generate_combined_status(
            false, // review_period_met
            true,  // signatures_met
            "Review period not met",
            "Signatures complete",
        );

        assert!(status.contains("❌"), "Should show requirements not met");
        assert!(
            status.contains("Review period not met"),
            "Should include review period status"
        );
    }

    #[test]
    fn test_generate_tier_status_routine() {
        let status = StatusCheckGenerator::generate_tier_status(
            1, // tier
            "Routine Maintenance",
            true,
            true,
            "Review period met",
            "Signatures complete",
        );

        assert!(status.contains("🔧"), "Should have routine emoji");
        assert!(status.contains("Tier 1"), "Should show tier number");
    }

    #[test]
    fn test_generate_tier_status_emergency() {
        let status = StatusCheckGenerator::generate_tier_status(
            4, // tier
            "Emergency",
            true,
            true,
            "Review period met",
            "Signatures complete",
        );

        assert!(status.contains("🚨"), "Should have emergency emoji");
        assert!(status.contains("Tier 4"), "Should show tier number");
    }
}
