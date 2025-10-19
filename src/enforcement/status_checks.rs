use chrono::{DateTime, Utc};
use crate::validation::{ReviewPeriodValidator, ThresholdValidator};
use crate::validation::emergency::{EmergencyTier, ActiveEmergency};

pub struct StatusCheckGenerator;

impl StatusCheckGenerator {
    pub fn generate_review_period_status(
        opened_at: DateTime<Utc>,
        required_days: i64,
        emergency_mode: bool,
    ) -> String {
        let remaining_days = ReviewPeriodValidator::get_remaining_days(
            opened_at,
            required_days,
            emergency_mode,
        );

        if remaining_days > 0 {
            let earliest_merge = ReviewPeriodValidator::get_earliest_merge_date(
                opened_at,
                required_days,
                emergency_mode,
            );

            format!(
                "❌ Governance: Review Period Not Met\nRequired: {} days | Elapsed: {} days\nEarliest merge: {}",
                required_days,
                (Utc::now() - opened_at).num_days(),
                earliest_merge.format("%Y-%m-%d")
            )
        } else {
            "✅ Governance: Review Period Met".to_string()
        }
    }

    pub fn generate_signature_status(
        current_signatures: usize,
        required_signatures: usize,
        total_maintainers: usize,
        signers: &[String],
        pending: &[String],
    ) -> String {
        if current_signatures >= required_signatures {
            "✅ Governance: Signatures Complete".to_string()
        } else {
            ThresholdValidator::format_threshold_status(
                current_signatures,
                required_signatures,
                total_maintainers,
                signers,
                pending,
            )
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
                "❌ Governance: Requirements Not Met\n\n{}\n\n{}",
                review_period_status,
                signature_status
            )
        }
    }

    /// Generate status check message for active emergency tier
    pub fn generate_emergency_status(emergency: &ActiveEmergency) -> String {
        let tier = emergency.tier;
        let emoji = tier.emoji();
        let name = tier.name();
        let (sig_required, sig_total) = tier.signature_threshold();
        let review_days = tier.review_period_days();
        let remaining = emergency.remaining_duration();
        
        let expiration_text = if remaining.num_hours() < 24 {
            format!("⏰ Expires in {} hours", remaining.num_hours())
        } else {
            format!("Expires in {} days", remaining.num_days())
        };

        let extension_text = if emergency.can_extend() {
            let max_ext = tier.max_extensions();
            let used_ext = emergency.extension_count;
            format!(
                "\n📋 Extensions: {} of {} used (can extend by {} days)",
                used_ext,
                max_ext,
                tier.extension_duration_days()
            )
        } else if tier.allows_extensions() && emergency.extension_count >= tier.max_extensions() {
            "\n⚠️ Maximum extensions reached".to_string()
        } else {
            "\n🚫 Extensions not allowed for this tier".to_string()
        };

        format!(
            "{} Emergency Tier Active: {}\n\
            📊 Requirements: {}-of-{} signatures, {} day review period\n\
            {}{}\n\
            \n\
            Reason: {}\n\
            Activated by: {} on {}",
            emoji,
            name,
            sig_required,
            sig_total,
            review_days,
            expiration_text,
            extension_text,
            emergency.reason,
            emergency.activated_by,
            emergency.activated_at.format("%Y-%m-%d %H:%M UTC")
        )
    }

    /// Generate status check for emergency tier expiration warning
    pub fn generate_emergency_expiration_warning(emergency: &ActiveEmergency) -> String {
        let remaining = emergency.remaining_duration();
        let tier = emergency.tier;
        let emoji = tier.emoji();

        if remaining.num_hours() < 24 {
            format!(
                "⚠️ {} Emergency Tier Expiring Soon\n\
                ⏰ Less than 24 hours remaining\n\
                Expires at: {}\n\
                \n\
                {}",
                emoji,
                emergency.expires_at.format("%Y-%m-%d %H:%M UTC"),
                if emergency.can_extend() {
                    format!(
                        "Extension available: requires {}-of-{} signatures",
                        tier.extension_threshold().0,
                        tier.extension_threshold().1
                    )
                } else {
                    "Extensions not available for this tier".to_string()
                }
            )
        } else if remaining.num_days() < 3 {
            format!(
                "⚠️ {} Emergency Tier Expiring Soon\n\
                ⏰ {} days remaining\n\
                Expires at: {}",
                emoji,
                remaining.num_days(),
                emergency.expires_at.format("%Y-%m-%d %H:%M UTC")
            )
        } else {
            String::new()
        }
    }

    /// Generate combined status with emergency tier
    pub fn generate_combined_status_with_emergency(
        review_period_met: bool,
        signatures_met: bool,
        review_period_status: &str,
        signature_status: &str,
        emergency: Option<&ActiveEmergency>,
    ) -> String {
        let base_status = if review_period_met && signatures_met {
            "✅ Governance: All Requirements Met - Ready to Merge".to_string()
        } else {
            format!(
                "❌ Governance: Requirements Not Met\n\n{}\n\n{}",
                review_period_status,
                signature_status
            )
        };

        if let Some(emerg) = emergency {
            let emergency_status = Self::generate_emergency_status(emerg);
            let expiration_warning = Self::generate_emergency_expiration_warning(emerg);
            
            if expiration_warning.is_empty() {
                format!("{}\n\n---\n\n{}", emergency_status, base_status)
            } else {
                format!(
                    "{}\n\n---\n\n{}\n\n---\n\n{}",
                    emergency_status,
                    expiration_warning,
                    base_status
                )
            }
        } else {
            base_status
        }
    }

    /// Generate post-emergency requirements status
    pub fn generate_post_emergency_requirements(
        tier: EmergencyTier,
        post_mortem_published: bool,
        post_mortem_deadline: DateTime<Utc>,
        security_audit_completed: bool,
        security_audit_deadline: Option<DateTime<Utc>>,
    ) -> String {
        let mut status = format!("📋 Post-Emergency Requirements for {}\n\n", tier.name());

        // Post-mortem status
        let pm_status = if post_mortem_published {
            "✅ Post-mortem published"
        } else if Utc::now() > post_mortem_deadline {
            "❌ Post-mortem OVERDUE"
        } else {
            let days_remaining = (post_mortem_deadline - Utc::now()).num_days();
            if days_remaining < 7 {
                "⚠️ Post-mortem due soon"
            } else {
                "⏳ Post-mortem pending"
            }
        };
        
        status.push_str(&format!(
            "{}\nDeadline: {}\n",
            pm_status,
            post_mortem_deadline.format("%Y-%m-%d")
        ));

        // Security audit status (if required)
        if tier.requires_security_audit() {
            if let Some(audit_deadline) = security_audit_deadline {
                let audit_status = if security_audit_completed {
                    "✅ Security audit completed"
                } else if Utc::now() > audit_deadline {
                    "❌ Security audit OVERDUE"
                } else {
                    let days_remaining = (audit_deadline - Utc::now()).num_days();
                    if days_remaining < 14 {
                        "⚠️ Security audit due soon"
                    } else {
                        "⏳ Security audit pending"
                    }
                };
                
                status.push_str(&format!(
                    "\n{}\nDeadline: {}",
                    audit_status,
                    audit_deadline.format("%Y-%m-%d")
                ));
            }
        }

        status
    }
}



