#!/bin/bash
# BTCDecoded Governance App - SQLite Backup Script
# This script creates automated backups of the SQLite database

set -euo pipefail

# Configuration
DB_PATH="${DB_PATH:-/opt/governance-app/data/governance.db}"
BACKUP_DIR="${BACKUP_DIR:-/opt/governance-app/backups}"
BACKUP_NAME="governance_$(date +%Y%m%d_%H%M%S).db"
RETENTION_DAYS="${RETENTION_DAYS:-30}"
COMPRESSION="${COMPRESSION:-true}"
LOG_FILE="${LOG_FILE:-/var/log/governance-backup.log}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Logging function
log() {
    echo "$(date '+%Y-%m-%d %H:%M:%S') - $1" | tee -a "$LOG_FILE"
}

log_error() {
    echo -e "${RED}$(date '+%Y-%m-%d %H:%M:%S') - ERROR: $1${NC}" | tee -a "$LOG_FILE"
}

log_success() {
    echo -e "${GREEN}$(date '+%Y-%m-%d %H:%M:%S') - SUCCESS: $1${NC}" | tee -a "$LOG_FILE"
}

log_warning() {
    echo -e "${YELLOW}$(date '+%Y-%m-%d %H:%M:%S') - WARNING: $1${NC}" | tee -a "$LOG_FILE"
}

# Check if database file exists
check_database() {
    if [[ ! -f "$DB_PATH" ]]; then
        log_error "Database file not found: $DB_PATH"
        exit 1
    fi
    
    if [[ ! -r "$DB_PATH" ]]; then
        log_error "Cannot read database file: $DB_PATH"
        exit 1
    fi
    
    log "Database file found: $DB_PATH"
}

# Check if backup directory exists and is writable
check_backup_directory() {
    if [[ ! -d "$BACKUP_DIR" ]]; then
        log "Creating backup directory: $BACKUP_DIR"
        mkdir -p "$BACKUP_DIR"
    fi
    
    if [[ ! -w "$BACKUP_DIR" ]]; then
        log_error "Cannot write to backup directory: $BACKUP_DIR"
        exit 1
    fi
    
    log "Backup directory ready: $BACKUP_DIR"
}

# Create database backup
create_backup() {
    local backup_file="$BACKUP_DIR/$BACKUP_NAME"
    
    log "Starting database backup..."
    
    # Check if WAL mode is active
    local journal_mode=$(sqlite3 "$DB_PATH" "PRAGMA journal_mode;" 2>/dev/null || echo "unknown")
    log "Database journal mode: $journal_mode"
    
    if [[ "$journal_mode" == "wal" ]]; then
        log "WAL mode detected, checkpointing before backup..."
        sqlite3 "$DB_PATH" "PRAGMA wal_checkpoint(TRUNCATE);" || {
            log_warning "WAL checkpoint failed, continuing with backup..."
        }
    fi
    
    # Create backup using sqlite3 backup command
    sqlite3 "$DB_PATH" ".backup '$backup_file'" || {
        log_error "Database backup failed"
        exit 1
    }
    
    # Verify backup integrity
    local integrity_check=$(sqlite3 "$backup_file" "PRAGMA integrity_check;" 2>/dev/null || echo "error")
    if [[ "$integrity_check" != "ok" ]]; then
        log_error "Backup integrity check failed: $integrity_check"
        rm -f "$backup_file"
        exit 1
    fi
    
    log_success "Database backup created: $backup_file"
    
    # Compress backup if enabled
    if [[ "$COMPRESSION" == "true" ]]; then
        log "Compressing backup..."
        gzip "$backup_file" || {
            log_error "Backup compression failed"
            exit 1
        }
        backup_file="${backup_file}.gz"
        log_success "Backup compressed: $backup_file"
    fi
    
    # Set secure permissions
    chmod 600 "$backup_file" || {
        log_warning "Failed to set backup file permissions"
    }
    
    # Get backup file size
    local backup_size=$(du -h "$backup_file" | cut -f1)
    log "Backup size: $backup_size"
    
    echo "$backup_file"
}

# Clean up old backups
cleanup_old_backups() {
    log "Cleaning up backups older than $RETENTION_DAYS days..."
    
    local deleted_count=0
    while IFS= read -r -d '' file; do
        rm -f "$file"
        ((deleted_count++))
        log "Deleted old backup: $(basename "$file")"
    done < <(find "$BACKUP_DIR" -name "governance_*.db*" -mtime +$RETENTION_DAYS -print0 2>/dev/null)
    
    if [[ $deleted_count -eq 0 ]]; then
        log "No old backups to clean up"
    else
        log_success "Cleaned up $deleted_count old backup(s)"
    fi
}

# Verify backup
verify_backup() {
    local backup_file="$1"
    
    log "Verifying backup integrity..."
    
    # Decompress if needed
    local temp_db="/tmp/verify_$(basename "$backup_file" .gz).db"
    if [[ "$backup_file" == *.gz ]]; then
        gunzip -c "$backup_file" > "$temp_db" || {
            log_error "Failed to decompress backup for verification"
            return 1
        }
    else
        cp "$backup_file" "$temp_db"
    fi
    
    # Run integrity check
    local integrity_result=$(sqlite3 "$temp_db" "PRAGMA integrity_check;" 2>/dev/null || echo "error")
    
    # Clean up temp file
    rm -f "$temp_db"
    
    if [[ "$integrity_result" == "ok" ]]; then
        log_success "Backup verification passed"
        return 0
    else
        log_error "Backup verification failed: $integrity_result"
        return 1
    fi
}

# Send notification (if configured)
send_notification() {
    local status="$1"
    local message="$2"
    
    # Check if webhook URL is configured
    if [[ -n "${WEBHOOK_URL:-}" ]]; then
        local payload=$(cat <<EOF
{
    "text": "Governance App Backup $status",
    "attachments": [
        {
            "color": "$(if [[ "$status" == "SUCCESS" ]]; then echo "good"; else echo "danger"; fi)",
            "fields": [
                {
                    "title": "Status",
                    "value": "$status",
                    "short": true
                },
                {
                    "title": "Message",
                    "value": "$message",
                    "short": false
                },
                {
                    "title": "Timestamp",
                    "value": "$(date '+%Y-%m-%d %H:%M:%S')",
                    "short": true
                }
            ]
        }
    ]
}
EOF
        )
        
        curl -s -X POST -H 'Content-type: application/json' \
            --data "$payload" \
            "$WEBHOOK_URL" || {
            log_warning "Failed to send notification"
        }
    fi
}

# Main function
main() {
    log "Starting BTCDecoded Governance App backup process"
    
    # Check prerequisites
    check_database
    check_backup_directory
    
    # Create backup
    local backup_file
    backup_file=$(create_backup)
    
    # Verify backup
    if verify_backup "$backup_file"; then
        log_success "Backup process completed successfully"
        send_notification "SUCCESS" "Database backup completed successfully: $(basename "$backup_file")"
    else
        log_error "Backup verification failed"
        send_notification "FAILED" "Database backup verification failed"
        exit 1
    fi
    
    # Clean up old backups
    cleanup_old_backups
    
    log "Backup process finished"
}

# Handle script arguments
case "${1:-}" in
    --help|-h)
        echo "Usage: $0 [options]"
        echo "Options:"
        echo "  --help, -h          Show this help message"
        echo "  --verify FILE       Verify a specific backup file"
        echo "  --cleanup           Only clean up old backups"
        echo ""
        echo "Environment variables:"
        echo "  DB_PATH             Database file path (default: /opt/governance-app/data/governance.db)"
        echo "  BACKUP_DIR          Backup directory (default: /opt/governance-app/backups)"
        echo "  RETENTION_DAYS      Days to retain backups (default: 30)"
        echo "  COMPRESSION         Enable compression (default: true)"
        echo "  LOG_FILE            Log file path (default: /var/log/governance-backup.log)"
        echo "  WEBHOOK_URL         Webhook URL for notifications"
        exit 0
        ;;
    --verify)
        if [[ -z "${2:-}" ]]; then
            log_error "No backup file specified for verification"
            exit 1
        fi
        verify_backup "$2"
        ;;
    --cleanup)
        check_backup_directory
        cleanup_old_backups
        ;;
    "")
        main
        ;;
    *)
        log_error "Unknown option: $1"
        echo "Use --help for usage information"
        exit 1
        ;;
esac




