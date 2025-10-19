use crate::error::GovernanceError;

pub struct ThresholdValidator;

impl ThresholdValidator {
    pub fn validate_threshold(
        current_signatures: usize,
        required_signatures: usize,
        total_maintainers: usize,
    ) -> Result<bool, GovernanceError> {
        if current_signatures >= required_signatures {
            Ok(true)
        } else {
            Err(GovernanceError::ThresholdError(format!(
                "Signature threshold not met. Required: {}/{} signatures, Current: {}/{}",
                required_signatures,
                total_maintainers,
                current_signatures,
                total_maintainers
            )))
        }
    }

    pub fn get_threshold_for_layer(layer: i32) -> (usize, usize) {
        match layer {
            1 | 2 => (6, 7),  // Constitutional layers: 6-of-7
            3 => (4, 5),      // Implementation layer: 4-of-5
            4 => (3, 5),      // Application layer: 3-of-5
            5 => (2, 3),      // Extension layer: 2-of-3
            _ => (1, 1),      // Default fallback
        }
    }

    pub fn get_review_period_for_layer(layer: i32, emergency_mode: bool) -> i64 {
        if emergency_mode {
            30  // Emergency mode: 30 days for all layers
        } else {
            match layer {
                1 | 2 => 180,  // Constitutional layers: 180 days
                3 => 90,       // Implementation layer: 90 days
                4 => 60,       // Application layer: 60 days
                5 => 14,       // Extension layer: 14 days
                _ => 30,       // Default fallback
            }
        }
    }

    pub fn format_threshold_status(
        current: usize,
        required: usize,
        total: usize,
        signers: &[String],
        pending: &[String],
    ) -> String {
        format!(
            "❌ Governance: Signatures Missing\nRequired: {}-of-{} | Current: {}/{}\nSigned by: {}\nPending: {}",
            required,
            total,
            current,
            total,
            signers.join(", "),
            pending.join(", ")
        )
    }
}




