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
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Clean up everything: stop services, remove containers, and prune Docker"
    echo ""
    echo "Options:"
    echo "  -y, --yes               Confirm without prompting (non-interactive)"
    echo "  -s, --services-only     Only stop services, don't clean Docker resources"
    echo "  -h, --help              Show this help message"
    echo ""
    echo "What this script does:"
    echo "  1. Stop all Raworc services (direct Docker management)"
    echo "  2. Remove ALL Raworc containers (raworc_*)"
    echo "  3. Remove ALL Raworc images (raworc/*)"
    echo "  4. Remove ALL Docker volumes"
    echo "  5. Prune unused networks and build cache"
    echo ""
    echo "⚠️  WARNING: This is a destructive operation!"
}

# Parse command line arguments
CONFIRM=false
SERVICES_ONLY=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -y|--yes)
            CONFIRM=true
            shift
            ;;
        -s|--services-only)
            SERVICES_ONLY=true
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
            print_error "Unknown argument: $1"
            usage
            exit 1
            ;;
    esac
done

# Change to project root
cd "$PROJECT_ROOT"

print_status "Raworc Reset - Complete Cleanup"
echo ""

if [ "$SERVICES_ONLY" = true ]; then
    print_status "Services-only mode: Will stop services but skip Docker cleanup"
else
    print_status "Full reset mode: This will remove ALL Raworc-related Docker resources"
fi

echo ""
print_warning "This will:"
print_warning "  - Stop all Raworc services"
print_warning "  - Remove ALL Raworc containers (session, server, operator, mysql)"

if [ "$SERVICES_ONLY" = false ]; then
    print_warning "  - Remove ALL Raworc images (including space images)"
    print_warning "  - Remove ALL Docker volumes"
    print_warning "  - Remove unused Docker networks"
    print_warning "  - Clean up build cache"
fi

echo ""

# Confirm with user unless --yes flag is provided
if [ "$CONFIRM" = false ]; then
    read -p "$(echo -e "${YELLOW}This is a destructive operation. Continue? [y/N]: ${NC}")" -r
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        print_status "Operation cancelled"
        exit 0
    fi
fi

echo ""
print_status "Starting reset process..."

# Step 1: Stop all Raworc services
echo ""
print_status "[1/8] Stopping Raworc services..."

# Try to use the stop script if available
STOP_SCRIPT="$SCRIPT_DIR/stop.sh"
if [ -f "$STOP_SCRIPT" ]; then
    print_status "Using stop script: $STOP_SCRIPT"
    if "$STOP_SCRIPT" --cleanup --remove 2>/dev/null; then
        print_success "Services stopped using stop script"
    else
        print_warning "Stop script failed, proceeding with manual cleanup"
    fi
else
    print_warning "Stop script not found, proceeding with manual cleanup"
fi

# Manual cleanup - stop any remaining containers
running_containers=$(docker ps -q --filter "name=raworc_" 2>/dev/null || true)
if [ -n "$running_containers" ]; then
    print_status "Stopping remaining running containers..."
    if docker stop $running_containers 2>/dev/null; then
        print_success "Stopped remaining containers"
    else
        print_warning "Failed to stop some running containers"
    fi
else
    print_success "No running containers found"
fi

# Step 2: Remove ALL raworc containers (running and stopped)
echo ""
print_status "[2/8] Removing ALL raworc containers..."

container_ids=$(docker ps -a -q --filter "name=raworc" 2>/dev/null || true)
if [ -n "$container_ids" ]; then
    container_count=$(echo "$container_ids" | wc -l)
    print_status "Found $container_count containers to remove"
    
    if docker rm -f $container_ids; then
        print_success "Removed $container_count containers"
    else
        print_warning "Failed to remove some containers"
    fi
else
    print_success "No containers found"
fi

if [ "$SERVICES_ONLY" = true ]; then
    print_success "Services-only reset completed!"
    exit 0
fi

# Step 3: Remove ALL raworc images (including space images)
echo ""
print_status "[3/8] Removing ALL raworc images..."

image_ids=$(docker images -q --filter "reference=raworc*" 2>/dev/null || true)
if [ -n "$image_ids" ]; then
    image_count=$(echo "$image_ids" | wc -l)
    print_status "Found $image_count images to remove"
    
    if docker rmi -f $image_ids; then
        print_success "Removed $image_count images"
    else
        print_warning "Failed to remove some images"
    fi
else
    print_success "No raworc images found"
fi

# Step 4: Prune unused networks
echo ""
print_status "[4/8] Pruning unused networks..."

if docker network prune -f; then
    print_success "Networks pruned"
else
    print_warning "Failed to prune networks"
fi

# Step 5: Remove ALL volumes
echo ""
print_status "[5/8] Removing ALL Docker volumes..."

volume_names=$(docker volume ls -q 2>/dev/null || true)
if [ -n "$volume_names" ]; then
    volume_count=$(echo "$volume_names" | wc -l)
    print_status "Found $volume_count volume(s) to remove"
    
    if docker volume rm -f $volume_names 2>/dev/null; then
        print_success "Removed all volumes"
    else
        print_warning "Some volumes may be in use, trying individual removal..."
        removed_count=0
        for volume in $volume_names; do
            if docker volume rm -f "$volume" 2>/dev/null; then
                ((removed_count++))
            fi
        done
        print_success "Removed $removed_count of $volume_count volumes"
    fi
else
    print_success "No volumes found"
fi

# Step 6: Prune dangling images
echo ""
print_status "[6/8] Pruning dangling images..."

if docker image prune -f; then
    print_success "Dangling images pruned"
else
    print_warning "Failed to prune images"
fi

# Step 7: Prune build cache (but preserve some cache for faster rebuilds)
echo ""
print_status "[7/8] Pruning build cache..."

if docker builder prune -f; then
    print_success "Build cache pruned"
else
    print_warning "Failed to prune build cache"
fi

# Step 8: Show final disk usage
echo ""
print_status "[8/8] Final Docker disk usage:"

if docker system df; then
    print_success "Disk usage displayed"
else
    print_warning "Failed to show disk usage"
fi

echo ""
print_success "🎉 Reset completed!"
echo ""
print_status "Summary:"
print_success "  ✓ Docker containers, volumes, and networks cleaned up"
print_success "  ✓ Raworc images removed"
print_success "  ✓ Build cache cleaned (preserving some for faster rebuilds)"
echo ""
print_status "To start Raworc again:"
echo "  • Scripts: ./scripts/start.sh --build"
echo "  • CLI: raworc start"
echo "  • Production: Use published Docker images with raworc CLI"