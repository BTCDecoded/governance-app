-- Key metadata table for production key management
CREATE TABLE IF NOT EXISTS key_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key_id TEXT NOT NULL UNIQUE,
    key_type TEXT NOT NULL, -- 'maintainer', 'economic_node', 'emergency', 'github_app', 'system'
    owner TEXT NOT NULL,
    public_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending', -- 'active', 'pending', 'revoked', 'expired', 'compromised'
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME NOT NULL,
    last_used DATETIME,
    usage_count INTEGER DEFAULT 0,
    metadata JSON NOT NULL DEFAULT '{}'
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_key_metadata_key_id ON key_metadata(key_id);
CREATE INDEX IF NOT EXISTS idx_key_metadata_key_type ON key_metadata(key_type);
CREATE INDEX IF NOT EXISTS idx_key_metadata_owner ON key_metadata(owner);
CREATE INDEX IF NOT EXISTS idx_key_metadata_status ON key_metadata(status);
CREATE INDEX IF NOT EXISTS idx_key_metadata_expires_at ON key_metadata(expires_at);
CREATE INDEX IF NOT EXISTS idx_key_metadata_last_used ON key_metadata(last_used);

-- Composite indexes for common queries
CREATE INDEX IF NOT EXISTS idx_key_metadata_type_status ON key_metadata(key_type, status);
CREATE INDEX IF NOT EXISTS idx_key_metadata_owner_status ON key_metadata(owner, status);
CREATE INDEX IF NOT EXISTS idx_key_metadata_expires_status ON key_metadata(expires_at, status);




