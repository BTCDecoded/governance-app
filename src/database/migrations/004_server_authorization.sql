-- Server Authorization Schema
-- Creates tables for managing authorized governance servers

CREATE TABLE IF NOT EXISTS authorized_servers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id TEXT UNIQUE NOT NULL,
    operator_name TEXT NOT NULL,
    operator_jurisdiction TEXT NOT NULL,
    operator_contact TEXT,
    nostr_npub TEXT UNIQUE NOT NULL,
    ssh_fingerprint TEXT NOT NULL,
    vpn_ip TEXT,
    added_at TIMESTAMP NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    last_verified TIMESTAMP,
    
    CHECK (status IN ('active', 'retiring', 'inactive', 'compromised'))
);

CREATE INDEX idx_authorized_servers_status ON authorized_servers(status);
CREATE INDEX idx_authorized_servers_npub ON authorized_servers(nostr_npub);

CREATE TABLE IF NOT EXISTS server_approvals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id TEXT NOT NULL,
    maintainer_id INTEGER NOT NULL,
    action TEXT NOT NULL,
    signature TEXT NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    
    FOREIGN KEY (server_id) REFERENCES authorized_servers(server_id),
    FOREIGN KEY (maintainer_id) REFERENCES maintainers(id),
    CHECK (action IN ('add', 'remove', 'compromise'))
);

CREATE INDEX idx_server_approvals_server_id ON server_approvals(server_id);
CREATE INDEX idx_server_approvals_maintainer_id ON server_approvals(maintainer_id);
CREATE INDEX idx_server_approvals_action ON server_approvals(action);
