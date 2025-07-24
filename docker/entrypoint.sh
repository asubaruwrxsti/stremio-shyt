#!/bin/bash
set -e

echo "🚀 Starting Stremio BitTorrent API..."

# Ensure directories exist
mkdir -p "$(dirname "$DATABASE_PATH")"
mkdir -p "$DOWNLOAD_DIR"

# Check if database exists, if not create it
if [ ! -f "$DATABASE_PATH" ]; then
    echo "📦 Creating new database at $DATABASE_PATH"
    touch "$DATABASE_PATH"
fi

echo "🎯 Starting API server..."
echo "🌐 API will be available at: http://${API_HOST}:${API_PORT}"
echo "📁 Download directory: $DOWNLOAD_DIR"
echo "🗄️  Database: $DATABASE_PATH"

# Start the application
exec ./api-server
