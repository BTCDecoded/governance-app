//! Governance Fork Detection
//! 
//! Detects governance fork conditions and triggers appropriate responses.

use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, debug};

use crate::error::GovernanceError;
use super::types::*;
use super::adoption::AdoptionTracker;

/// Detects governance fork conditions and manages fork triggers
pub struct ForkDetector {
    adoption_tracker: AdoptionTracker,
    fork_thresholds: ForkThresholds,
    detection_history: Vec<ForkDetectionEvent>,
    last_detection: Option<DateTime<Utc>>,
}

/// Fork detection event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkDetectionEvent {
    pub event_id: String,
    pub detected_at: DateTime<Utc>,
    pub ruleset_id: String,
    pub trigger_type: ForkTriggerType,
    pub metrics: AdoptionMetrics,
    pub threshold_met: bool,
    pub action_taken: Option<ForkAction>,
}

/// Types of fork triggers
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForkTriggerType {
    AdoptionThreshold,
    TimeBased,
    Manual,
    Emergency,
    Consensus,
}

/// Actions taken in response to fork detection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForkAction {
    ForkExecuted,
    ForkScheduled,
    ForkRejected,
    MonitoringIncreased,
    AlertSent,
}

impl ForkDetector {
    /// Create a new fork detector
    pub fn new(
        adoption_tracker: AdoptionTracker,
        fork_thresholds: Option<ForkThresholds>,
    ) -> Self {
        Self {
            adoption_tracker,
            fork_thresholds: fork_thresholds.unwrap_or_default(),
            detection_history: Vec::new(),
            last_detection: None,
        }
    }

    /// Run fork detection analysis
    pub async fn detect_forks(&mut self) -> Result<Vec<ForkDetectionEvent>, GovernanceError> {
        info!("Running fork detection analysis...");
        
        let mut new_detections = Vec::new();
        
        // Get current adoption statistics
        let adoption_stats = self.adoption_tracker.get_adoption_statistics().await?;
        
        // Check each ruleset for fork conditions
        for metrics in &adoption_stats.rulesets {
            if let Some(detection) = self.analyze_ruleset_for_fork(metrics).await? {
                new_detections.push(detection);
            }
        }
        
        // Check for time-based triggers
        if let Some(time_detection) = self.check_time_based_triggers().await? {
            new_detections.push(time_detection);
        }
        
        // Check for consensus-based triggers
        if let Some(consensus_detection) = self.check_consensus_triggers(&adoption_stats).await? {
            new_detections.push(consensus_detection);
        }
        
        // Store detections
        for detection in &new_detections {
            self.detection_history.push(detection.clone());
        }
        
        self.last_detection = Some(Utc::now());
        
        info!("Fork detection completed: {} new detections", new_detections.len());
        Ok(new_detections)
    }

    /// Analyze a specific ruleset for fork conditions
    async fn analyze_ruleset_for_fork(
        &self,
        metrics: &AdoptionMetrics,
    ) -> Result<Option<ForkDetectionEvent>, GovernanceError> {
        debug!("Analyzing ruleset {} for fork conditions", metrics.ruleset_id);
        
        let threshold_met = self.check_adoption_thresholds(metrics);
        
        if threshold_met {
            info!("Fork threshold met for ruleset: {}", metrics.ruleset_id);
            
            let detection = ForkDetectionEvent {
                event_id: uuid::Uuid::new_v4().to_string(),
                detected_at: Utc::now(),
                ruleset_id: metrics.ruleset_id.clone(),
                trigger_type: ForkTriggerType::AdoptionThreshold,
                metrics: metrics.clone(),
                threshold_met: true,
                action_taken: Some(ForkAction::ForkExecuted),
            };
            
            return Ok(Some(detection));
        }
        
        // Check if approaching threshold (for early warning)
        if self.is_approaching_threshold(metrics) {
            warn!("Ruleset {} approaching fork threshold", metrics.ruleset_id);
            
            let detection = ForkDetectionEvent {
                event_id: uuid::Uuid::new_v4().to_string(),
                detected_at: Utc::now(),
                ruleset_id: metrics.ruleset_id.clone(),
                trigger_type: ForkTriggerType::AdoptionThreshold,
                metrics: metrics.clone(),
                threshold_met: false,
                action_taken: Some(ForkAction::MonitoringIncreased),
            };
            
            return Ok(Some(detection));
        }
        
        Ok(None)
    }

    /// Check if adoption thresholds are met
    fn check_adoption_thresholds(&self, metrics: &AdoptionMetrics) -> bool {
        metrics.node_count >= self.fork_thresholds.minimum_node_count &&
        metrics.hashpower_percentage >= self.fork_thresholds.minimum_hashpower_percentage &&
        metrics.economic_activity_percentage >= self.fork_thresholds.minimum_economic_activity_percentage &&
        metrics.total_weight >= self.fork_thresholds.minimum_adoption_percentage
    }

    /// Check if approaching threshold (for early warning)
    fn is_approaching_threshold(&self, metrics: &AdoptionMetrics) -> bool {
        let node_threshold = self.fork_thresholds.minimum_node_count as f64 * 0.8;
        let hashpower_threshold = self.fork_thresholds.minimum_hashpower_percentage * 0.8;
        let economic_threshold = self.fork_thresholds.minimum_economic_activity_percentage * 0.8;
        let adoption_threshold = self.fork_thresholds.minimum_adoption_percentage * 0.8;
        
        metrics.node_count as f64 >= node_threshold ||
        metrics.hashpower_percentage >= hashpower_threshold ||
        metrics.economic_activity_percentage >= economic_threshold ||
        metrics.total_weight >= adoption_threshold
    }

    /// Check for time-based fork triggers
    async fn check_time_based_triggers(&self) -> Result<Option<ForkDetectionEvent>, GovernanceError> {
        // Check if grace period has expired for any pending forks
        if let Some(last_detection) = self.last_detection {
            let grace_period = Duration::days(self.fork_thresholds.grace_period_days as i64);
            let time_since_detection = Utc::now() - last_detection;
            
            if time_since_detection > grace_period {
                // Check if there are pending forks that should be executed
                let pending_forks = self.get_pending_forks().await?;
                
                if !pending_forks.is_empty() {
                    info!("Grace period expired, executing pending forks");
                    
                    let detection = ForkDetectionEvent {
                        event_id: uuid::Uuid::new_v4().to_string(),
                        detected_at: Utc::now(),
                        ruleset_id: "time_triggered".to_string(),
                        trigger_type: ForkTriggerType::TimeBased,
                        metrics: AdoptionMetrics {
                            ruleset_id: "time_triggered".to_string(),
                            node_count: 0,
                            hashpower_percentage: 0.0,
                            economic_activity_percentage: 0.0,
                            total_weight: 0.0,
                            last_updated: Utc::now(),
                        },
                        threshold_met: true,
                        action_taken: Some(ForkAction::ForkScheduled),
                    };
                    
                    return Ok(Some(detection));
                }
            }
        }
        
        Ok(None)
    }

    /// Check for consensus-based triggers
    async fn check_consensus_triggers(
        &self,
        adoption_stats: &AdoptionStatistics,
    ) -> Result<Option<ForkDetectionEvent>, GovernanceError> {
        // Check if there's a clear winning ruleset
        if let Some(winning_ruleset) = &adoption_stats.winning_ruleset {
            let winning_metrics = adoption_stats.rulesets.iter()
                .find(|m| &m.ruleset_id == winning_ruleset);
            
            if let Some(metrics) = winning_metrics {
                // Check if winning ruleset has overwhelming support
                let overwhelming_threshold = 80.0; // 80% adoption
                
                if metrics.total_weight >= overwhelming_threshold {
                    info!("Overwhelming consensus detected for ruleset: {}", winning_ruleset);
                    
                    let detection = ForkDetectionEvent {
                        event_id: uuid::Uuid::new_v4().to_string(),
                        detected_at: Utc::now(),
                        ruleset_id: winning_ruleset.clone(),
                        trigger_type: ForkTriggerType::Consensus,
                        metrics: metrics.clone(),
                        threshold_met: true,
                        action_taken: Some(ForkAction::ForkExecuted),
                    };
                    
                    return Ok(Some(detection));
                }
            }
        }
        
        Ok(None)
    }

    /// Get pending forks that are waiting for execution
    async fn get_pending_forks(&self) -> Result<Vec<String>, GovernanceError> {
        // This would check for forks that have been detected but not yet executed
        // For now, return empty list
        Ok(Vec::new())
    }

    /// Get detection history
    pub fn get_detection_history(&self) -> &[ForkDetectionEvent] {
        &self.detection_history
    }

    /// Get recent detections
    pub fn get_recent_detections(&self, hours: i64) -> Vec<&ForkDetectionEvent> {
        let cutoff = Utc::now() - chrono::Duration::hours(hours);
        
        self.detection_history
            .iter()
            .filter(|detection| detection.detected_at > cutoff)
            .collect()
    }

    /// Get detection statistics
    pub fn get_detection_statistics(&self) -> ForkDetectionStats {
        let total_detections = self.detection_history.len();
        let successful_forks = self.detection_history
            .iter()
            .filter(|d| d.action_taken == Some(ForkAction::ForkExecuted))
            .count();
        
        let trigger_counts = self.detection_history
            .iter()
            .fold(HashMap::new(), |mut acc, detection| {
                *acc.entry(detection.trigger_type.clone()).or_insert(0) += 1;
                acc
            });
        
        ForkDetectionStats {
            total_detections,
            successful_forks,
            trigger_counts,
            last_detection: self.last_detection,
        }
    }

    /// Update fork thresholds
    pub fn update_thresholds(&mut self, new_thresholds: ForkThresholds) {
        self.fork_thresholds = new_thresholds;
        info!("Fork thresholds updated");
    }

    /// Get current thresholds
    pub fn get_thresholds(&self) -> &ForkThresholds {
        &self.fork_thresholds
    }
}

/// Fork detection statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkDetectionStats {
    pub total_detections: usize,
    pub successful_forks: usize,
    pub trigger_counts: HashMap<ForkTriggerType, usize>,
    pub last_detection: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_fork_detection() {
        let temp_dir = tempdir().unwrap();
        let adoption_tracker = AdoptionTracker::new().unwrap();
        let mut detector = ForkDetector::new(adoption_tracker, None);
        
        // Test with empty adoption stats
        let detections = detector.detect_forks().await.unwrap();
        assert_eq!(detections.len(), 0);
    }

    #[test]
    fn test_threshold_checking() {
        let temp_dir = tempdir().unwrap();
        let adoption_tracker = AdoptionTracker::new().unwrap();
        let detector = ForkDetector::new(adoption_tracker, None);
        
        let metrics = AdoptionMetrics {
            ruleset_id: "test".to_string(),
            node_count: 100,
            hashpower_percentage: 50.0,
            economic_activity_percentage: 60.0,
            total_weight: 70.0,
            last_updated: Utc::now(),
        };
        
        assert!(detector.check_adoption_thresholds(&metrics));
    }
}
