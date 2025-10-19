use governance_app::validation::*;

#[tokio::test]
async fn test_review_period_validation() {
    use chrono::{DateTime, Utc, Duration};
    
    let now = Utc::now();
    let opened_at = now - Duration::days(100); // 100 days ago
    
    // Test normal mode
    let result = ReviewPeriodValidator::validate_review_period(opened_at, 90, false);
    assert!(result.is_ok());
    
    // Test emergency mode
    let result = ReviewPeriodValidator::validate_review_period(opened_at, 90, true);
    assert!(result.is_ok());
    
    // Test insufficient time
    let opened_recently = now - Duration::days(10);
    let result = ReviewPeriodValidator::validate_review_period(opened_recently, 90, false);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_threshold_validation() {
    // Test valid threshold
    let result = ThresholdValidator::validate_threshold(5, 4, 7);
    assert!(result.is_ok());
    
    // Test invalid threshold
    let result = ThresholdValidator::validate_threshold(3, 4, 7);
    assert!(result.is_err());
    
    // Test layer-specific thresholds
    let (required, total) = ThresholdValidator::get_threshold_for_layer(1);
    assert_eq!((required, total), (6, 7));
    
    let (required, total) = ThresholdValidator::get_threshold_for_layer(3);
    assert_eq!((required, total), (4, 5));
}




