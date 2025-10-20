-- Audit log (immutable record)
CREATE TABLE governance_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_type TEXT NOT NULL,
  repo_name TEXT,
  pr_number INTEGER,
  maintainer TEXT,
  details TEXT,
  timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Index for audit log queries
CREATE INDEX idx_events_timestamp ON governance_events(timestamp DESC);




