-- Repository configurations (cached from governance repo)
CREATE TABLE repos (
  id SERIAL PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  layer INTEGER NOT NULL,
  signature_threshold TEXT NOT NULL,
  review_period_days INTEGER NOT NULL,
  synchronized_with TEXT,
  last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Maintainer keys by layer (cached from governance repo)
CREATE TABLE maintainers (
  id SERIAL PRIMARY KEY,
  github_username TEXT NOT NULL UNIQUE,
  public_key TEXT NOT NULL,
  layer INTEGER NOT NULL,
  active BOOLEAN DEFAULT true,
  last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Emergency keyholders (cached from governance repo)
CREATE TABLE emergency_keyholders (
  id SERIAL PRIMARY KEY,
  github_username TEXT NOT NULL UNIQUE,
  public_key TEXT NOT NULL,
  active BOOLEAN DEFAULT true,
  last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Pull request tracking (app state)
CREATE TABLE pull_requests (
  id SERIAL PRIMARY KEY,
  repo_name TEXT NOT NULL,
  pr_number INTEGER NOT NULL,
  opened_at TIMESTAMP NOT NULL,
  layer INTEGER NOT NULL,
  tier INTEGER NOT NULL,  -- Add tier column
  head_sha TEXT NOT NULL,
  signatures JSONB DEFAULT '[]',
  governance_status TEXT DEFAULT 'pending',
  linked_prs JSONB DEFAULT '[]',
  emergency_mode BOOLEAN DEFAULT false,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(repo_name, pr_number)
);

-- Economic nodes registry
CREATE TABLE economic_nodes (
  id SERIAL PRIMARY KEY,
  entity_name TEXT NOT NULL,
  node_type TEXT NOT NULL,
  public_key TEXT NOT NULL UNIQUE,
  qualification_proof JSONB NOT NULL,
  weight REAL NOT NULL,
  status TEXT NOT NULL,
  registered_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  last_verified TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Veto signals
CREATE TABLE veto_signals (
  id SERIAL PRIMARY KEY,
  pr_id INTEGER NOT NULL REFERENCES pull_requests(id),
  node_id INTEGER NOT NULL REFERENCES economic_nodes(id),
  signal_type TEXT NOT NULL,
  weight REAL NOT NULL,
  signature TEXT NOT NULL,
  rationale TEXT NOT NULL,
  verified BOOLEAN DEFAULT false,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(pr_id, node_id)
);

-- Governance events (audit log)
CREATE TABLE governance_events (
  id SERIAL PRIMARY KEY,
  event_type TEXT NOT NULL,
  repo_name TEXT,
  pr_number INTEGER,
  maintainer TEXT,
  details JSONB,
  timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Governance rulesets (for fork system)
CREATE TABLE governance_rulesets (
  id SERIAL PRIMARY KEY,
  version TEXT NOT NULL UNIQUE,
  config_yaml TEXT NOT NULL,
  config_hash TEXT NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  created_by TEXT NOT NULL
);

-- Fork decisions (node adoption tracking)
CREATE TABLE fork_decisions (
  id SERIAL PRIMARY KEY,
  node_identifier TEXT NOT NULL,
  ruleset_version TEXT NOT NULL REFERENCES governance_rulesets(version),
  decision TEXT NOT NULL,
  signature TEXT NOT NULL,
  decided_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(node_identifier, ruleset_version)
);

-- Cross-layer rules (cached from governance repo)
CREATE TABLE cross_layer_rules (
  id SERIAL PRIMARY KEY,
  source_repo TEXT NOT NULL,
  source_pattern TEXT NOT NULL,
  target_repo TEXT NOT NULL,
  target_pattern TEXT NOT NULL,
  validation_type TEXT NOT NULL,
  last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Emergency activations
CREATE TABLE emergency_activations (
  id SERIAL PRIMARY KEY,
  tier INTEGER NOT NULL,
  activated_by TEXT NOT NULL,
  reason TEXT NOT NULL,
  evidence TEXT NOT NULL,
  signatures JSONB DEFAULT '[]',
  activated_at TIMESTAMP,
  expires_at TIMESTAMP,
  active BOOLEAN DEFAULT false,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for performance
CREATE INDEX idx_prs_repo_status ON pull_requests(repo_name, governance_status);
CREATE INDEX idx_prs_opened_at ON pull_requests(opened_at);
CREATE INDEX idx_prs_layer_tier ON pull_requests(layer, tier);
CREATE INDEX idx_maintainers_layer ON maintainers(layer, active);
CREATE INDEX idx_events_timestamp ON governance_events(timestamp DESC);
CREATE INDEX idx_events_pr ON governance_events(pr_number);
CREATE INDEX idx_economic_nodes_status ON economic_nodes(status);
CREATE INDEX idx_veto_signals_pr ON veto_signals(pr_id);
CREATE INDEX idx_emergency_active ON emergency_activations(active, expires_at);






