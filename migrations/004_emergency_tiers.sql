-- Migration 004: Emergency Tier System
-- Adds three-tiered emergency response system

CREATE TABLE emergency_tiers (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  
  -- Tier configuration
  tier INTEGER NOT NULL CHECK (tier IN (1, 2, 3)),
  
  -- Activation metadata
  activated_by TEXT NOT NULL,  -- GitHub username of initiating keyholder
  reason TEXT NOT NULL,
  evidence TEXT NOT NULL,
  
  -- Approval tracking
  signatures TEXT DEFAULT '[]',  -- Array of {keyholder, signature, timestamp}
  activation_threshold TEXT NOT NULL DEFAULT '5-of-7',
  
  -- Timing
  activated_at TIMESTAMP,
  expires_at TIMESTAMP,
  extended BOOLEAN DEFAULT false,
  extension_count INTEGER DEFAULT 0,
  
  -- Status
  active BOOLEAN DEFAULT false,
  
  -- Post-activation requirements
  post_mortem_published BOOLEAN DEFAULT false,
  post_mortem_url TEXT,
  post_mortem_deadline TIMESTAMP,
  
  security_audit_completed BOOLEAN DEFAULT false,
  security_audit_url TEXT,
  security_audit_deadline TIMESTAMP,
  
  public_disclosure_completed BOOLEAN DEFAULT false,
  public_disclosure_url TEXT,
  
  -- Audit trail
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  deactivated_at TIMESTAMP,
  deactivation_reason TEXT
);

-- Index for active emergency lookups
CREATE INDEX idx_emergency_tiers_active 
  ON emergency_tiers(active, tier, expires_at) 
  WHERE active = true;

-- Index for expiration checking
CREATE INDEX idx_emergency_tiers_expiration 
  ON emergency_tiers(expires_at) 
  WHERE active = true;

-- Index for post-activation requirement tracking
CREATE INDEX idx_emergency_tiers_requirements 
  ON emergency_tiers(
    post_mortem_published, 
    security_audit_completed, 
    public_disclosure_completed
  ) 
  WHERE active = false;

-- Table for emergency activation votes
CREATE TABLE emergency_activation_votes (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  emergency_tier_id INTEGER REFERENCES emergency_tiers(id) ON DELETE CASCADE,
  
  -- Voter info
  keyholder_github_username TEXT NOT NULL,
  keyholder_public_key TEXT NOT NULL,
  
  -- Vote
  vote BOOLEAN NOT NULL,  -- true = approve, false = reject
  signature TEXT NOT NULL,
  signed_message TEXT NOT NULL,  -- JSON of emergency details
  
  -- Metadata
  voted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  
  UNIQUE(emergency_tier_id, keyholder_github_username)
);

CREATE INDEX idx_emergency_votes_tier 
  ON emergency_activation_votes(emergency_tier_id, vote);

-- Table for emergency extensions
CREATE TABLE emergency_extensions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  emergency_tier_id INTEGER REFERENCES emergency_tiers(id) ON DELETE CASCADE,
  
  -- Extension request
  requested_by TEXT NOT NULL,
  justification TEXT NOT NULL,
  extension_duration_days INTEGER NOT NULL,
  
  -- Approval
  signatures TEXT DEFAULT '[]',
  approval_threshold TEXT NOT NULL,
  approved BOOLEAN DEFAULT false,
  
  -- Timing
  requested_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  approved_at TIMESTAMP,
  new_expiration TIMESTAMP
);

CREATE INDEX idx_emergency_extensions_tier 
  ON emergency_extensions(emergency_tier_id, approved);

-- Table for emergency tier audit log
CREATE TABLE emergency_audit_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  emergency_tier_id INTEGER REFERENCES emergency_tiers(id) ON DELETE CASCADE,
  
  -- Event
  event_type TEXT NOT NULL,  -- 'activated', 'extended', 'expired', 'deactivated', 'requirement_met'
  event_data TEXT,
  
  -- Actor
  actor TEXT,  -- GitHub username or 'system'
  
  -- Timing
  occurred_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_emergency_audit_tier 
  ON emergency_audit_log(emergency_tier_id, occurred_at);

CREATE INDEX idx_emergency_audit_type 
  ON emergency_audit_log(event_type, occurred_at);

-- Note: PostgreSQL-specific functions and triggers removed for SQLite compatibility
-- These will be implemented in Rust application code:
-- - updated_at timestamp updates
-- - emergency expiration checking
-- - emergency event logging

-- Documentation comments removed for SQLite compatibility
-- See source code for table and column documentation


