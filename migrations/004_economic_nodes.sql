-- Migration 004: Economic Node Registry and Veto System
-- Creates tables for economic node registration and veto signal collection

CREATE TABLE economic_nodes (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  node_type TEXT NOT NULL, -- 'mining_pool', 'exchange', 'custodian', 'payment_processor', 'major_holder'
  entity_name TEXT NOT NULL,
  public_key TEXT NOT NULL,
  qualification_data TEXT DEFAULT '{}', -- JSON with proof of qualification
  weight REAL DEFAULT 0.0, -- Calculated weight (0.0-1.0)
  status TEXT DEFAULT 'pending', -- 'pending', 'active', 'suspended', 'removed'
  registered_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  verified_at TIMESTAMP,
  last_verified_at TIMESTAMP,
  created_by TEXT, -- GitHub username of who registered
  notes TEXT DEFAULT ''
);

CREATE TABLE veto_signals (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  pr_id INTEGER NOT NULL,
  node_id INTEGER NOT NULL,
  signal_type TEXT NOT NULL, -- 'veto', 'support', 'abstain'
  weight REAL NOT NULL, -- Weight of this signal (0.0-1.0)
  signature TEXT NOT NULL, -- Cryptographic signature of the signal
  rationale TEXT NOT NULL, -- Required explanation for veto
  timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  verified BOOLEAN DEFAULT FALSE,
  FOREIGN KEY (pr_id) REFERENCES pull_requests(id),
  FOREIGN KEY (node_id) REFERENCES economic_nodes(id)
);

-- Indexes for performance
CREATE INDEX idx_economic_nodes_type ON economic_nodes(node_type);
CREATE INDEX idx_economic_nodes_status ON economic_nodes(status);
CREATE INDEX idx_economic_nodes_weight ON economic_nodes(weight DESC);
CREATE INDEX idx_veto_signals_pr ON veto_signals(pr_id);
CREATE INDEX idx_veto_signals_node ON veto_signals(node_id);
CREATE INDEX idx_veto_signals_timestamp ON veto_signals(timestamp DESC);
CREATE INDEX idx_veto_signals_type ON veto_signals(signal_type);

-- Node types enum values:
-- 'mining_pool' - Mining pools with 1%+ hashpower
-- 'exchange' - Exchanges with $100M+ daily volume and 10K+ BTC
-- 'custodian' - Custodians with 10K+ BTC holdings
-- 'payment_processor' - Payment processors with $50M+ monthly BTC transactions
-- 'major_holder' - Major holders with 5K+ BTC

-- Signal types:
-- 'veto' - Object to the change
-- 'support' - Support the change (for Tier 5 governance changes)
-- 'abstain' - No position (still counts for participation)




