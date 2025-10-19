-- Repository configurations (cached from governance repo)
CREATE TABLE repos (
  id SERIAL PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  layer INTEGER NOT NULL,
  signature_threshold TEXT NOT NULL,
  review_period_days INTEGER NOT NULL,
  synchronized_with TEXT[],
  last_updated TIMESTAMP DEFAULT NOW()
);

-- Maintainer keys by layer (cached from governance repo)
CREATE TABLE maintainers (
  id SERIAL PRIMARY KEY,
  github_username TEXT NOT NULL UNIQUE,
  public_key TEXT NOT NULL,
  layer INTEGER NOT NULL,
  active BOOLEAN DEFAULT true,
  last_updated TIMESTAMP DEFAULT NOW()
);

-- Emergency keyholders (cached from governance repo)
CREATE TABLE emergency_keyholders (
  id SERIAL PRIMARY KEY,
  github_username TEXT NOT NULL UNIQUE,
  public_key TEXT NOT NULL,
  active BOOLEAN DEFAULT true,
  last_updated TIMESTAMP DEFAULT NOW()
);

-- Pull request tracking (app state)
CREATE TABLE pull_requests (
  id SERIAL PRIMARY KEY,
  repo_name TEXT NOT NULL,
  pr_number INTEGER NOT NULL,
  opened_at TIMESTAMP NOT NULL,
  layer INTEGER NOT NULL,
  head_sha TEXT NOT NULL,
  signatures JSONB DEFAULT '[]',
  governance_status TEXT DEFAULT 'pending',
  linked_prs JSONB DEFAULT '[]',
  emergency_mode BOOLEAN DEFAULT false,
  created_at TIMESTAMP DEFAULT NOW(),
  updated_at TIMESTAMP DEFAULT NOW(),
  UNIQUE(repo_name, pr_number)
);

-- Cross-layer rules (cached from governance repo)
CREATE TABLE cross_layer_rules (
  id SERIAL PRIMARY KEY,
  source_repo TEXT NOT NULL,
  source_pattern TEXT NOT NULL,
  target_repo TEXT NOT NULL,
  target_pattern TEXT NOT NULL,
  validation_type TEXT NOT NULL,
  last_updated TIMESTAMP DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_prs_repo_status ON pull_requests(repo_name, governance_status);
CREATE INDEX idx_prs_opened_at ON pull_requests(opened_at);
CREATE INDEX idx_maintainers_layer ON maintainers(layer, active);




