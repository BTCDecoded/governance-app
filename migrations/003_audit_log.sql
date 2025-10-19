-- Audit log (immutable record)
CREATE TABLE governance_events (
  id SERIAL PRIMARY KEY,
  event_type TEXT NOT NULL,
  repo_name TEXT,
  pr_number INTEGER,
  maintainer TEXT,
  details JSONB,
  timestamp TIMESTAMP DEFAULT NOW()
);

-- Index for audit log queries
CREATE INDEX idx_events_timestamp ON governance_events(timestamp DESC);




