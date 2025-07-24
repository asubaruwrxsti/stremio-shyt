#!/bin/bash
set -e

echo "🚀 Starting Stremio BitTorrent API..."

# Check if migrations directory exists
if [ -d "/app/migrations" ]; then
    echo "📦 Running database migrations..."
    
    # Set database URL for diesel
    export DATABASE_URL="sqlite://${DATABASE_PATH}"
    
    # Run migrations
    diesel migration run --migration-dir /app/migrations
    
    echo "✅ Database migrations completed successfully"
else
    echo "⚠️  No migrations directory found, skipping migrations"
fi

echo "🎯 Starting API server..."

# Start the application
exec ./api-server
