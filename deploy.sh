#!/bin/bash

# Stremio BitTorrent API Deployment Script
set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Stremio BitTorrent API Deployment${NC}"
echo "=================================="

# Create data directory if it doesn't exist
if [ ! -d "data" ]; then
    echo -e "${YELLOW}📁 Creating data directory...${NC}"
    mkdir -p data
fi

# Function to build and run with Docker Compose
deploy() {
    echo -e "${BLUE}🏗️  Building Docker image...${NC}"
    docker-compose -f docker/docker-compose.yml build

    echo -e "${BLUE}📦 Starting services...${NC}"
    docker-compose -f docker/docker-compose.yml up -d

    echo -e "${GREEN}✅ Deployment complete!${NC}"
    echo ""
    echo "🌐 API is available at: http://localhost:8080"
    echo "📊 Health check: http://localhost:8080/health"
    echo ""
    echo "📖 Available endpoints:"
    echo "  GET  /api/torrents          - List all torrents"
    echo "  POST /api/torrents          - Add torrent by URL"
    echo "  POST /api/torrents/upload   - Upload .torrent file"
    echo "  GET  /api/torrents/:id      - Get torrent details"
    echo "  POST /api/torrents/:id/start - Start download"
    echo "  POST /api/torrents/:id/pause - Pause torrent"
    echo "  POST /api/torrents/:id/resume - Resume torrent"
    echo "  GET  /api/status             - System status"
    echo "  GET  /health                 - Health check"
    echo ""
    echo "🔧 Management commands:"
    echo "  ./deploy.sh logs     - View logs"
    echo "  ./deploy.sh stop     - Stop services"
    echo "  ./deploy.sh restart  - Restart services"
    echo "  ./deploy.sh status   - Check service status"
}

# Function to show logs
logs() {
    echo -e "${BLUE}📋 Showing logs...${NC}"
    docker-compose -f docker/docker-compose.yml logs -f
}

# Function to stop services
stop() {
    echo -e "${YELLOW}🛑 Stopping services...${NC}"
    docker-compose -f docker/docker-compose.yml down
    echo -e "${GREEN}✅ Services stopped${NC}"
}

# Function to restart services
restart() {
    echo -e "${BLUE}🔄 Restarting services...${NC}"
    docker-compose -f docker/docker-compose.yml restart
    echo -e "${GREEN}✅ Services restarted${NC}"
}

# Function to check status
status() {
    echo -e "${BLUE}📊 Service status:${NC}"
    docker-compose -f docker/docker-compose.yml ps
    echo ""
    echo -e "${BLUE}🏥 Health check:${NC}"
    curl -s http://localhost:8080/health | jq . 2>/dev/null || echo "Service not responding or jq not installed"
}

# Function to show help
help() {
    echo "Stremio BitTorrent API Deployment Script"
    echo ""
    echo "Usage: $0 [COMMAND]"
    echo ""
    echo "Commands:"
    echo "  deploy   - Build and deploy the service (default)"
    echo "  logs     - Show service logs"
    echo "  stop     - Stop all services"
    echo "  restart  - Restart all services"
    echo "  status   - Show service status and health"
    echo "  help     - Show this help message"
}

# Parse command line arguments
case "${1:-deploy}" in
    deploy)
        deploy
        ;;
    logs)
        logs
        ;;
    stop)
        stop
        ;;
    restart)
        restart
        ;;
    status)
        status
        ;;
    help|--help|-h)
        help
        ;;
    *)
        echo -e "${YELLOW}❌ Unknown command: $1${NC}"
        help
        exit 1
        ;;
esac
