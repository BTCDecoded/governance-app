-- Migration 004: Emergency Tier System
-- Adds three-tiered emergency response system

CREATE TABLE emergency_tiers (
  id SERIAL PRIMARY KEY,
  
  -- Tier configuration
  tier INTEGER NOT NULL CHECK (tier IN (1, 2, 3)),
  
  -- Activation metadata
  activated_by TEXT NOT NULL,  -- GitHub username of initiating keyholder
  reason TEXT NOT NULL,
  evidence TEXT NOT NULL,
  
  -- Approval tracking
  signatures JSONB DEFAULT '[]',  -- Array of {keyholder, signature, timestamp}
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
  created_at TIMESTAMP DEFAULT NOW(),
  updated_at TIMESTAMP DEFAULT NOW(),
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
  id SERIAL PRIMARY KEY,
  emergency_tier_id INTEGER REFERENCES emergency_tiers(id) ON DELETE CASCADE,
  
  -- Voter info
  keyholder_github_username TEXT NOT NULL,
  keyholder_public_key TEXT NOT NULL,
  
  -- Vote
  vote BOOLEAN NOT NULL,  -- true = approve, false = reject
  signature TEXT NOT NULL,
  signed_message TEXT NOT NULL,  -- JSON of emergency details
  
  -- Metadata
  voted_at TIMESTAMP DEFAULT NOW(),
  
  UNIQUE(emergency_tier_id, keyholder_github_username)
);

CREATE INDEX idx_emergency_votes_tier 
  ON emergency_activation_votes(emergency_tier_id, vote);

-- Table for emergency extensions
CREATE TABLE emergency_extensions (
  id SERIAL PRIMARY KEY,
  emergency_tier_id INTEGER REFERENCES emergency_tiers(id) ON DELETE CASCADE,
  
  -- Extension request
  requested_by TEXT NOT NULL,
  justification TEXT NOT NULL,
  extension_duration_days INTEGER NOT NULL,
  
  -- Approval
  signatures JSONB DEFAULT '[]',
  approval_threshold TEXT NOT NULL,
  approved BOOLEAN DEFAULT false,
  
  -- Timing
  requested_at TIMESTAMP DEFAULT NOW(),
  approved_at TIMESTAMP,
  new_expiration TIMESTAMP
);

CREATE INDEX idx_emergency_extensions_tier 
  ON emergency_extensions(emergency_tier_id, approved);

-- Table for emergency tier audit log
CREATE TABLE emergency_audit_log (
  id SERIAL PRIMARY KEY,
  emergency_tier_id INTEGER REFERENCES emergency_tiers(id) ON DELETE CASCADE,
  
  -- Event
  event_type TEXT NOT NULL,  -- 'activated', 'extended', 'expired', 'deactivated', 'requirement_met'
  event_data JSONB,
  
  -- Actor
  actor TEXT,  -- GitHub username or 'system'
  
  -- Timing
  occurred_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_emergency_audit_tier 
  ON emergency_audit_log(emergency_tier_id, occurred_at);

CREATE INDEX idx_emergency_audit_type 
  ON emergency_audit_log(event_type, occurred_at);

-- Function to automatically update updated_at timestamp
CREATE OR REPLACE FUNCTION update_emergency_tier_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER emergency_tier_updated_at
  BEFORE UPDATE ON emergency_tiers
  FOR EACH ROW
  EXECUTE FUNCTION update_emergency_tier_updated_at();

-- Function to check for expired emergencies
CREATE OR REPLACE FUNCTION check_emergency_expiration()
RETURNS TABLE(expired_tier_id INTEGER, tier INTEGER, reason TEXT) AS $$
BEGIN
  RETURN QUERY
  SELECT 
    id,
    tier,
    'Emergency tier expired at ' || expires_at::TEXT as reason
  FROM emergency_tiers
  WHERE active = true 
    AND expires_at < NOW();
END;
$$ LANGUAGE plpgsql;

-- Function to log emergency events
CREATE OR REPLACE FUNCTION log_emergency_event(
  p_emergency_tier_id INTEGER,
  p_event_type TEXT,
  p_event_data JSONB,
  p_actor TEXT
)
RETURNS VOID AS $$
BEGIN
  INSERT INTO emergency_audit_log (
    emergency_tier_id,
    event_type,
    event_data,
    actor
  ) VALUES (
    p_emergency_tier_id,
    p_event_type,
    p_event_data,
    p_actor
  );
END;
$$ LANGUAGE plpgsql;

-- Comments for documentation
COMMENT ON TABLE emergency_tiers IS 'Three-tiered emergency response system for critical issues';
COMMENT ON COLUMN emergency_tiers.tier IS '1=Critical (0d review, 4-of-7, 7d max), 2=Urgent (7d review, 5-of-7, 30d max), 3=Elevated (30d review, 6-of-7, 90d max)';
COMMENT ON COLUMN emergency_tiers.signatures IS 'Array of emergency keyholder signatures approving activation';
COMMENT ON TABLE emergency_activation_votes IS 'Individual keyholder votes for emergency tier activation';
COMMENT ON TABLE emergency_extensions IS 'Extension requests for active emergency tiers';
COMMENT ON TABLE emergency_audit_log IS 'Audit trail of all emergency tier events';


