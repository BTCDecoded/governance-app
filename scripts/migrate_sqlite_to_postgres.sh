#!/bin/bash
# Migration script from SQLite to PostgreSQL
# Usage: ./migrate_sqlite_to_postgres.sh <sqlite_db> <postgres_url>

set -e

echo "BTCDecoded Governance App: SQLite to PostgreSQL Migration"
echo "=========================================================="

# Configuration
SQLITE_DB="${1:-governance.db}"
POSTGRES_URL="${2:-postgresql://localhost/governance}"

echo "Source: $SQLITE_DB"
echo "Target: $POSTGRES_URL"
echo ""

# Check if SQLite database exists
if [ ! -f "$SQLITE_DB" ]; then
    echo "Error: SQLite database '$SQLITE_DB' not found"
    exit 1
fi

# Check if PostgreSQL is accessible
echo "Testing PostgreSQL connection..."
if ! psql "$POSTGRES_URL" -c "SELECT 1;" > /dev/null 2>&1; then
    echo "Error: Cannot connect to PostgreSQL at '$POSTGRES_URL'"
    echo "Please ensure PostgreSQL is running and the connection string is correct"
    exit 1
fi
echo "PostgreSQL connection successful"
echo ""

# Create temporary directory for export files
TEMP_DIR=$(mktemp -d)
echo "Using temporary directory: $TEMP_DIR"

# Function to export SQLite data
export_sqlite_data() {
    local table=$1
    local output_file=$2
    
    echo "Exporting $table..."
    
    # Get column names
    local columns=$(sqlite3 "$SQLITE_DB" "PRAGMA table_info($table);" | cut -d'|' -f2 | tr '\n' ',' | sed 's/,$//')
    
    # Export data with proper formatting
    sqlite3 "$SQLITE_DB" <<EOF
.mode csv
.headers off
.output "$output_file"
SELECT $columns FROM $table;
EOF
}

# Function to import to PostgreSQL
import_to_postgres() {
    local table=$1
    local csv_file=$2
    
    echo "Importing $table to PostgreSQL..."
    
    # Create temporary table for import
    psql "$POSTGRES_URL" -c "CREATE TEMP TABLE temp_${table}_import (LIKE $table);"
    
    # Import CSV data
    psql "$POSTGRES_URL" -c "\\copy temp_${table}_import FROM '$csv_file' WITH CSV;"
    
    # Insert into main table, handling conflicts
    case $table in
        "repos")
            psql "$POSTGRES_URL" -c "
                INSERT INTO $table (name, layer, signature_threshold, review_period_days, synchronized_with, last_updated)
                SELECT name, layer, signature_threshold, review_period_days, synchronized_with, last_updated
                FROM temp_${table}_import
                ON CONFLICT (name) DO UPDATE SET
                    layer = EXCLUDED.layer,
                    signature_threshold = EXCLUDED.signature_threshold,
                    review_period_days = EXCLUDED.review_period_days,
                    synchronized_with = EXCLUDED.synchronized_with,
                    last_updated = EXCLUDED.last_updated;
            "
            ;;
        "maintainers")
            psql "$POSTGRES_URL" -c "
                INSERT INTO $table (github_username, public_key, layer, active, last_updated)
                SELECT github_username, public_key, layer, active, last_updated
                FROM temp_${table}_import
                ON CONFLICT (github_username) DO UPDATE SET
                    public_key = EXCLUDED.public_key,
                    layer = EXCLUDED.layer,
                    active = EXCLUDED.active,
                    last_updated = EXCLUDED.last_updated;
            "
            ;;
        "emergency_keyholders")
            psql "$POSTGRES_URL" -c "
                INSERT INTO $table (github_username, public_key, active, last_updated)
                SELECT github_username, public_key, active, last_updated
                FROM temp_${table}_import
                ON CONFLICT (github_username) DO UPDATE SET
                    public_key = EXCLUDED.public_key,
                    active = EXCLUDED.active,
                    last_updated = EXCLUDED.last_updated;
            "
            ;;
        "pull_requests")
            psql "$POSTGRES_URL" -c "
                INSERT INTO $table (repo_name, pr_number, opened_at, layer, tier, head_sha, signatures, governance_status, linked_prs, emergency_mode, created_at, updated_at)
                SELECT repo_name, pr_number, opened_at, layer, COALESCE(tier, 1), head_sha, 
                       COALESCE(signatures, '[]'::jsonb), 
                       COALESCE(governance_status, 'pending'), 
                       COALESCE(linked_prs, '[]'::jsonb), 
                       COALESCE(emergency_mode, false), 
                       COALESCE(created_at, CURRENT_TIMESTAMP), 
                       COALESCE(updated_at, CURRENT_TIMESTAMP)
                FROM temp_${table}_import
                ON CONFLICT (repo_name, pr_number) DO UPDATE SET
                    opened_at = EXCLUDED.opened_at,
                    layer = EXCLUDED.layer,
                    tier = EXCLUDED.tier,
                    head_sha = EXCLUDED.head_sha,
                    signatures = EXCLUDED.signatures,
                    governance_status = EXCLUDED.governance_status,
                    linked_prs = EXCLUDED.linked_prs,
                    emergency_mode = EXCLUDED.emergency_mode,
                    updated_at = EXCLUDED.updated_at;
            "
            ;;
        "economic_nodes")
            psql "$POSTGRES_URL" -c "
                INSERT INTO $table (entity_name, node_type, public_key, qualification_proof, weight, status, registered_at, last_verified)
                SELECT entity_name, node_type, public_key, 
                       COALESCE(qualification_proof, '{}'::jsonb), 
                       weight, status, 
                       COALESCE(registered_at, CURRENT_TIMESTAMP), 
                       COALESCE(last_verified, CURRENT_TIMESTAMP)
                FROM temp_${table}_import
                ON CONFLICT (public_key) DO UPDATE SET
                    entity_name = EXCLUDED.entity_name,
                    node_type = EXCLUDED.node_type,
                    qualification_proof = EXCLUDED.qualification_proof,
                    weight = EXCLUDED.weight,
                    status = EXCLUDED.status,
                    last_verified = EXCLUDED.last_verified;
            "
            ;;
        "veto_signals")
            psql "$POSTGRES_URL" -c "
                INSERT INTO $table (pr_id, node_id, signal_type, weight, signature, rationale, verified, created_at)
                SELECT pr_id, node_id, signal_type, weight, signature, rationale, 
                       COALESCE(verified, false), 
                       COALESCE(created_at, CURRENT_TIMESTAMP)
                FROM temp_${table}_import
                ON CONFLICT (pr_id, node_id) DO UPDATE SET
                    signal_type = EXCLUDED.signal_type,
                    weight = EXCLUDED.weight,
                    signature = EXCLUDED.signature,
                    rationale = EXCLUDED.rationale,
                    verified = EXCLUDED.verified;
            "
            ;;
        "governance_events")
            psql "$POSTGRES_URL" -c "
                INSERT INTO $table (event_type, repo_name, pr_number, maintainer, details, timestamp)
                SELECT event_type, repo_name, pr_number, maintainer, 
                       COALESCE(details, '{}'::jsonb), 
                       COALESCE(timestamp, CURRENT_TIMESTAMP)
                FROM temp_${table}_import;
            "
            ;;
        "cross_layer_rules")
            psql "$POSTGRES_URL" -c "
                INSERT INTO $table (source_repo, source_pattern, target_repo, target_pattern, validation_type, last_updated)
                SELECT source_repo, source_pattern, target_repo, target_pattern, validation_type, 
                       COALESCE(last_updated, CURRENT_TIMESTAMP)
                FROM temp_${table}_import;
            "
            ;;
        "emergency_activations")
            psql "$POSTGRES_URL" -c "
                INSERT INTO $table (tier, activated_by, reason, evidence, signatures, activated_at, expires_at, active, created_at)
                SELECT tier, activated_by, reason, evidence, 
                       COALESCE(signatures, '[]'::jsonb), 
                       activated_at, expires_at, 
                       COALESCE(active, false), 
                       COALESCE(created_at, CURRENT_TIMESTAMP)
                FROM temp_${table}_import;
            "
            ;;
    esac
    
    # Clean up temporary table
    psql "$POSTGRES_URL" -c "DROP TABLE temp_${table}_import;"
}

# Export all tables
echo "Exporting SQLite data..."
export_sqlite_data "repos" "$TEMP_DIR/repos.csv"
export_sqlite_data "maintainers" "$TEMP_DIR/maintainers.csv"
export_sqlite_data "emergency_keyholders" "$TEMP_DIR/emergency_keyholders.csv"
export_sqlite_data "pull_requests" "$TEMP_DIR/pull_requests.csv"
export_sqlite_data "economic_nodes" "$TEMP_DIR/economic_nodes.csv"
export_sqlite_data "veto_signals" "$TEMP_DIR/veto_signals.csv"
export_sqlite_data "governance_events" "$TEMP_DIR/governance_events.csv"
export_sqlite_data "cross_layer_rules" "$TEMP_DIR/cross_layer_rules.csv"
export_sqlite_data "emergency_activations" "$TEMP_DIR/emergency_activations.csv"

echo ""

# Import to PostgreSQL
echo "Importing to PostgreSQL..."
import_to_postgres "repos" "$TEMP_DIR/repos.csv"
import_to_postgres "maintainers" "$TEMP_DIR/maintainers.csv"
import_to_postgres "emergency_keyholders" "$TEMP_DIR/emergency_keyholders.csv"
import_to_postgres "pull_requests" "$TEMP_DIR/pull_requests.csv"
import_to_postgres "economic_nodes" "$TEMP_DIR/economic_nodes.csv"
import_to_postgres "veto_signals" "$TEMP_DIR/veto_signals.csv"
import_to_postgres "governance_events" "$TEMP_DIR/governance_events.csv"
import_to_postgres "cross_layer_rules" "$TEMP_DIR/cross_layer_rules.csv"
import_to_postgres "emergency_activations" "$TEMP_DIR/emergency_activations.csv"

echo ""

# Verify migration
echo "Verifying migration..."
echo "SQLite record counts:"
sqlite3 "$SQLITE_DB" <<EOF
SELECT 'repos' as table_name, COUNT(*) as count FROM repos
UNION ALL
SELECT 'maintainers', COUNT(*) FROM maintainers
UNION ALL
SELECT 'emergency_keyholders', COUNT(*) FROM emergency_keyholders
UNION ALL
SELECT 'pull_requests', COUNT(*) FROM pull_requests
UNION ALL
SELECT 'economic_nodes', COUNT(*) FROM economic_nodes
UNION ALL
SELECT 'veto_signals', COUNT(*) FROM veto_signals
UNION ALL
SELECT 'governance_events', COUNT(*) FROM governance_events
UNION ALL
SELECT 'cross_layer_rules', COUNT(*) FROM cross_layer_rules
UNION ALL
SELECT 'emergency_activations', COUNT(*) FROM emergency_activations;
EOF

echo ""
echo "PostgreSQL record counts:"
psql "$POSTGRES_URL" -c "
SELECT 'repos' as table_name, COUNT(*) as count FROM repos
UNION ALL
SELECT 'maintainers', COUNT(*) FROM maintainers
UNION ALL
SELECT 'emergency_keyholders', COUNT(*) FROM emergency_keyholders
UNION ALL
SELECT 'pull_requests', COUNT(*) FROM pull_requests
UNION ALL
SELECT 'economic_nodes', COUNT(*) FROM economic_nodes
UNION ALL
SELECT 'veto_signals', COUNT(*) FROM veto_signals
UNION ALL
SELECT 'governance_events', COUNT(*) FROM governance_events
UNION ALL
SELECT 'cross_layer_rules', COUNT(*) FROM cross_layer_rules
UNION ALL
SELECT 'emergency_activations', COUNT(*) FROM emergency_activations;
"

# Clean up
echo ""
echo "Cleaning up temporary files..."
rm -rf "$TEMP_DIR"

echo ""
echo "Migration complete!"
echo ""
echo "Next steps:"
echo "1. Update your DATABASE_URL environment variable to use PostgreSQL"
echo "2. Test the application with the new database"
echo "3. Consider backing up the original SQLite database before removing it"
echo ""
echo "Example DATABASE_URL: $POSTGRES_URL"






