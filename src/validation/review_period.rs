use chrono::{DateTime, Utc, Duration};
use crate::error::GovernanceError;

pub struct ReviewPeriodValidator;

impl ReviewPeriodValidator {
    pub fn validate_review_period(
        opened_at: DateTime<Utc>,
        required_days: i64,
        emergency_mode: bool,
    ) -> Result<bool, GovernanceError> {
        let now = Utc::now();
        let elapsed = now - opened_at;
        
        // Emergency mode reduces review period to 30 days
        let required_duration = if emergency_mode {
            Duration::days(30)
        } else {
            Duration::days(required_days)
        };

        if elapsed >= required_duration {
            Ok(true)
        } else {
            let remaining = required_duration - elapsed;
            Err(GovernanceError::ReviewPeriodError(format!(
                "Review period not met. Required: {} days, Elapsed: {} days, Remaining: {} days",
                required_days,
                elapsed.num_days(),
                remaining.num_days()
            )))
        }
    }

    pub fn get_earliest_merge_date(
        opened_at: DateTime<Utc>,
        required_days: i64,
        emergency_mode: bool,
    ) -> DateTime<Utc> {
        let required_duration = if emergency_mode {
            Duration::days(30)
        } else {
            Duration::days(required_days)
        };
        
        opened_at + required_duration
    }

    pub fn get_remaining_days(
        opened_at: DateTime<Utc>,
        required_days: i64,
        emergency_mode: bool,
    ) -> i64 {
        let now = Utc::now();
        let elapsed = now - opened_at;
        
        let required_duration = if emergency_mode {
            Duration::days(30)
        } else {
            Duration::days(required_days)
        };
        
        let remaining = required_duration - elapsed;
        remaining.num_days().max(0)
    }
}




