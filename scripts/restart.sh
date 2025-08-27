#!/bin/bash

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Show usage
usage() {
    echo "Usage: $0 [OPTIONS] [COMPONENTS...]"
    echo ""
    echo "Restart Raworc services (stop then start with direct Docker management)"
    echo ""
    echo "Components:"
    echo "  mysql       Restart only the MySQL database"
    echo "  server      Restart only the API server"
    echo "  operator    Restart only the operator service"
    echo "  (default)   Restart all services"
    echo ""
    echo "Options:"
    echo "  -b, --build             Build images before starting"
    echo "  -c, --cleanup           Clean up session containers during stop"
    echo "  -p, --pull              Pull base images before starting"
    echo "  -d, --detached          Run in detached mode (default)"
    echo "  -f, --foreground        Run MySQL in foreground mode"
    echo "  --clean=false           Don't clean up session containers (default: true)"
    echo "  -h, --help              Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                      Restart all services"
    echo "  $0 --build              Stop all, then rebuild and start all"
    echo "  $0 --cleanup            Stop with cleanup, then start all"
    echo "  $0 server mysql         Restart only server and database"
    echo "  $0 --build --foreground Complete restart with rebuild in foreground"
}

# Parse command line arguments
BUILD=false
CLEANUP=true
PULL=false
DETACHED=true
COMPONENTS=()

while [[ $# -gt 0 ]]; do
    case $1 in
        -b|--build)
            BUILD=true
            shift
            ;;
        -c|--cleanup)
            CLEANUP=true
            shift
            ;;
        -p|--pull)
            PULL=true
            shift
            ;;
        -d|--detached)
            DETACHED=true
            shift
            ;;
        -f|--foreground)
            DETACHED=false
            shift
            ;;
        --clean=false)
            CLEANUP=false
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        -*)
            print_error "Unknown option: $1"
            usage
            exit 1
            ;;
        *)
            COMPONENTS+=("$1")
            shift
            ;;
    esac
done

print_status "🔄 Restarting Raworc services with direct Docker management"

if [ ${#COMPONENTS[@]} -gt 0 ]; then
    print_status "Components: ${COMPONENTS[*]}"
else
    print_status "Components: all"
fi

print_status "Options: Build=$BUILD, Cleanup=$CLEANUP, Pull=$PULL, Detached=$DETACHED"

echo ""

# Check if scripts exist
STOP_SCRIPT="$SCRIPT_DIR/stop.sh"
START_SCRIPT="$SCRIPT_DIR/start.sh"

if [ ! -f "$STOP_SCRIPT" ]; then
    print_error "Stop script not found: $STOP_SCRIPT"
    exit 1
fi

if [ ! -f "$START_SCRIPT" ]; then
    print_error "Start script not found: $START_SCRIPT"
    exit 1
fi

# Step 1: Stop services
print_status "🛑 Step 1: Stopping services..."
echo ""

# Build stop command
stop_args=()
if [ "$CLEANUP" = true ]; then
    stop_args+=("--cleanup")
fi

# Add components if specified
if [ ${#COMPONENTS[@]} -gt 0 ]; then
    stop_args+=("${COMPONENTS[@]}")
fi

if ! "$STOP_SCRIPT" "${stop_args[@]}"; then
    print_error "Failed to stop services"
    exit 1
fi

echo ""
print_success "✅ Stop completed"

# Small delay to ensure services are fully stopped
print_status "⏳ Waiting 3 seconds for complete shutdown..."
sleep 3

echo ""

# Step 2: Start services
print_status "🚀 Step 2: Starting services..."
echo ""

# Build start command
start_args=()
if [ "$BUILD" = true ]; then
    start_args+=("--build")
fi

if [ "$PULL" = true ]; then
    start_args+=("--pull")
fi

# No registry/tag needed for local images

if [ "$DETACHED" = true ]; then
    start_args+=("--detached")
else
    start_args+=("--foreground")
fi

# Add components if specified
if [ ${#COMPONENTS[@]} -gt 0 ]; then
    start_args+=("${COMPONENTS[@]}")
fi

if "$START_SCRIPT" "${start_args[@]}"; then
    echo ""
    print_success "🎉 Restart completed successfully!"
    
    echo ""
    print_status "Services have been restarted and are now running"
    
    if [ "$DETACHED" = true ]; then
        echo ""
        print_status "Next steps:"
        echo "  • Check logs: docker logs raworc_server -f"
        echo "  • Check status: docker ps --filter 'name=raworc_'"
        echo "  • Test API: raworc api health"
    else
        print_status "Running in foreground mode. Press Ctrl+C to stop services."
    fi
else
    print_error "❌ Failed to start services after stop"
    echo ""
    print_status "Services have been stopped but failed to restart"
    print_status "You may need to investigate and start manually:"
    print_status "  $START_SCRIPT ${start_args[*]}"
    exit 1
fi