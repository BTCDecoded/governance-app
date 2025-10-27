-- Migration 003: Audit Log System
-- Creates immutable audit log for all governance events

CREATE TABLE governance_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_type TEXT NOT NULL,
  repo_name TEXT,
  pr_number INTEGER,
  maintainer TEXT,
  details TEXT DEFAULT '{}',
  timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Index for performance
CREATE INDEX idx_governance_events_timestamp ON governance_events(timestamp DESC);
CREATE INDEX idx_governance_events_type ON governance_events(event_type);
CREATE INDEX idx_governance_events_repo_pr ON governance_events(repo_name, pr_number);

-- Event types that will be logged:
-- 'pr_opened', 'pr_synchronized', 'signature_collected', 'signature_verified',
-- 'review_period_met', 'threshold_met', 'merge_approved', 'merge_blocked',
-- 'emergency_activated', 'economic_node_veto', 'governance_fork_initiated'