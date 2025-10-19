use crate::error::GovernanceError;

pub struct MergeBlocker;

impl MergeBlocker {
    pub fn should_block_merge(
        review_period_met: bool,
        signatures_met: bool,
        emergency_mode: bool,
    ) -> Result<bool, GovernanceError> {
        // In emergency mode, only signature threshold matters
        if emergency_mode {
            Ok(!signatures_met)
        } else {
            // Normal mode: both review period and signatures must be met
            Ok(!(review_period_met && signatures_met))
        }
    }

    pub fn get_block_reason(
        review_period_met: bool,
        signatures_met: bool,
        emergency_mode: bool,
    ) -> String {
        if emergency_mode {
            if !signatures_met {
                "Emergency mode: Signature threshold not met".to_string()
            } else {
                "Emergency mode: All requirements met".to_string()
            }
        } else {
            if !review_period_met && !signatures_met {
                "Both review period and signature requirements not met".to_string()
            } else if !review_period_met {
                "Review period requirement not met".to_string()
            } else if !signatures_met {
                "Signature threshold requirement not met".to_string()
            } else {
                "All governance requirements met".to_string()
            }
        }
    }
}




