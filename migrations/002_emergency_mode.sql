-- Emergency mode state (app state)
CREATE TABLE emergency_activations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  activated_by TEXT NOT NULL,
  reason TEXT NOT NULL,
  evidence TEXT NOT NULL,
  signatures TEXT DEFAULT '[]',
  activated_at TIMESTAMP,
  expires_at TIMESTAMP,
  active BOOLEAN DEFAULT false,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Index for emergency mode queries
CREATE INDEX idx_emergency_active ON emergency_activations(active, expires_at);




