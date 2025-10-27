-- Migration 005: Governance Fork System
-- Creates tables for governance ruleset versioning, adoption tracking, and fork decisions

CREATE TABLE governance_rulesets (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  version_major INTEGER NOT NULL,
  version_minor INTEGER NOT NULL,
  version_patch INTEGER NOT NULL,
  version_pre_release TEXT,
  version_build_metadata TEXT,
  hash TEXT NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  config TEXT NOT NULL, -- JSON configuration
  description TEXT,
  status TEXT DEFAULT 'active' -- 'active', 'deprecated', 'archived'
);

CREATE TABLE fork_decisions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  ruleset_id TEXT NOT NULL,
  node_id TEXT NOT NULL,
  node_type TEXT NOT NULL,
  weight REAL NOT NULL,
  decision_reason TEXT NOT NULL,
  signature TEXT NOT NULL,
  timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (ruleset_id) REFERENCES governance_rulesets(id)
);

CREATE TABLE fork_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id TEXT UNIQUE NOT NULL,
  event_type TEXT NOT NULL,
  ruleset_id TEXT,
  node_id TEXT,
  details TEXT NOT NULL, -- JSON details
  timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (ruleset_id) REFERENCES governance_rulesets(id)
);

CREATE TABLE adoption_metrics (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  ruleset_id TEXT NOT NULL,
  node_count INTEGER NOT NULL,
  hashpower_percentage REAL NOT NULL,
  economic_activity_percentage REAL NOT NULL,
  total_weight REAL NOT NULL,
  calculated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (ruleset_id) REFERENCES governance_rulesets(id)
);

-- Indexes for performance
CREATE INDEX idx_fork_decisions_ruleset ON fork_decisions(ruleset_id);
CREATE INDEX idx_fork_decisions_node ON fork_decisions(node_id);
CREATE INDEX idx_fork_decisions_timestamp ON fork_decisions(timestamp DESC);
CREATE INDEX idx_fork_events_type ON fork_events(event_type);
CREATE INDEX idx_fork_events_ruleset ON fork_events(ruleset_id);
CREATE INDEX idx_fork_events_timestamp ON fork_events(timestamp DESC);
CREATE INDEX idx_adoption_metrics_ruleset ON adoption_metrics(ruleset_id);
CREATE INDEX idx_adoption_metrics_calculated ON adoption_metrics(calculated_at DESC);

-- Event types enum values:
-- 'ruleset_created' - New ruleset created
-- 'ruleset_adopted' - Node adopted a ruleset
-- 'ruleset_abandoned' - Node abandoned a ruleset
-- 'fork_decision' - Node made a fork decision
-- 'adoption_threshold_met' - Adoption threshold reached
-- 'governance_fork' - Governance fork occurred

-- Node types for fork decisions:
-- 'mining_pool' - Mining pools
-- 'exchange' - Exchanges
-- 'custodian' - Custodians
-- 'payment_processor' - Payment processors
-- 'major_holder' - Major holders




