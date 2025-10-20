#!/bin/bash
# Setup SQLx offline mode for governance-app

echo "Setting up SQLx offline mode..."

# Set DATABASE_URL for SQLx prepare
export DATABASE_URL="sqlite://governance.db"

# Create a temporary database file for SQLx prepare
touch governance.db

# Run migrations first to create tables
echo "Running migrations..."
sqlx migrate run

# Run SQLx prepare to generate the query cache
echo "Running cargo sqlx prepare..."
cargo sqlx prepare

# Clean up temporary database
rm -f governance.db

echo "SQLx offline mode setup complete!"
echo "The .sqlx/ directory has been generated and can be committed to git."
