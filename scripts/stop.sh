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
    echo "Stop Raworc services using direct Docker container management"
    echo ""
    echo "Components:"
    echo "  mysql       Stop only the MySQL database"
    echo "  server      Stop only the API server"
    echo "  operator    Stop only the operator service"
    echo "  (default)   Stop all services"
    echo ""
    echo "Options:"
    echo "  -c, --cleanup           Clean up agent containers after stopping"
    echo "  -r, --remove            Remove containers after stopping"
    echo "  -v, --volumes           Remove named volumes after stopping"
    echo "  -n, --network           Remove Docker network after stopping"
    echo "  -h, --help              Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                      Stop all services"
    echo "  $0 server               Stop only server"
    echo "  $0 --cleanup            Stop all and clean agent containers"
    echo "  $0 --remove             Stop all and remove containers"
    echo "  $0 --remove --volumes   Stop all, remove containers and volumes"
}

# Parse command line arguments
CLEANUP=false
REMOVE=false
VOLUMES=false
NETWORK=false
COMPONENTS=()

while [[ $# -gt 0 ]]; do
    case $1 in
        -c|--cleanup)
            CLEANUP=true
            shift
            ;;
        -r|--remove)
            REMOVE=true
            shift
            ;;
        -v|--volumes)
            VOLUMES=true
            shift
            ;;
        -n|--network)
            NETWORK=true
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

# Default to all components if none specified
if [ ${#COMPONENTS[@]} -eq 0 ]; then
    COMPONENTS=("server" "operator" "mysql")
fi

print_status "Stopping Raworc services with direct Docker management"
print_status "Cleanup agent containers: $CLEANUP"
print_status "Remove containers: $REMOVE"
print_status "Remove volumes: $VOLUMES"
print_status "Remove network: $NETWORK"
print_status "Components: ${COMPONENTS[*]}"

# Check if Docker is available
if ! command -v docker >/dev/null 2>&1; then
    print_error "Docker is not available. Please install Docker first."
    exit 1
fi

echo ""

# Stop components in reverse order (operator, server, mysql)
declare -A component_containers
component_containers[mysql]="raworc_mysql"
component_containers[server]="raworc_server"
component_containers[operator]="raworc_operator"

# Reverse order for stopping
stop_order=()
for component in operator server mysql; do
    if [[ " ${COMPONENTS[*]} " =~ " ${component} " ]]; then
        stop_order+=("$component")
    fi
done

for component in "${stop_order[@]}"; do
    container_name="${component_containers[$component]}"
    
    print_status "Stopping $component ($container_name)..."
    
    # Check if container is running
    if docker ps -q --filter "name=$container_name" | grep -q .; then
        if docker stop "$container_name"; then
            print_success "Stopped $component"
        else
            print_error "Failed to stop $component"
        fi
    else
        print_success "$component is not running"
    fi
    
    # Remove container if requested
    if [ "$REMOVE" = true ]; then
        print_status "Removing $component container..."
        if docker ps -aq --filter "name=$container_name" | grep -q .; then
            if docker rm "$container_name"; then
                print_success "Removed $component container"
            else
                print_warning "Failed to remove $component container"
            fi
        else
            print_success "$component container already removed"
        fi
    fi
    
    echo ""
done

# Clean up agent containers if requested
if [ "$CLEANUP" = true ]; then
    print_status "Cleaning up agent containers..."
    
    # Find and stop agent containers
    agent_containers=$(docker ps -q --filter "name=raworc_agent_" 2>/dev/null || true)
    
    if [ -n "$agent_containers" ]; then
        container_count=$(echo "$agent_containers" | wc -w)
        print_status "Found $container_count agent container(s) to clean up"
        
        if docker stop $agent_containers 2>/dev/null && docker rm $agent_containers 2>/dev/null; then
            print_success "Cleaned up $container_count agent containers"
        else
            print_warning "Some agent containers could not be cleaned up"
        fi
    else
        print_success "No agent containers found"
    fi
    echo ""
fi

# Remove volumes if requested
if [ "$VOLUMES" = true ]; then
    print_status "Removing volumes..."
    
    for volume in mysql_data operator_data; do
        if docker volume inspect "$volume" >/dev/null 2>&1; then
            print_status "Removing volume $volume..."
            if docker volume rm "$volume" 2>/dev/null; then
                print_success "Removed volume $volume"
            else
                print_warning "Failed to remove volume $volume (may be in use)"
            fi
        else
            print_success "Volume $volume already removed"
        fi
    done
    echo ""
fi

# Remove network if requested
if [ "$NETWORK" = true ]; then
    print_status "Removing Docker network..."
    
    if docker network inspect raworc_network >/dev/null 2>&1; then
        if docker network rm raworc_network 2>/dev/null; then
            print_success "Removed raworc_network"
        else
            print_warning "Failed to remove raworc_network (may be in use)"
        fi
    else
        print_success "Network raworc_network already removed"
    fi
    echo ""
fi

# Show final status
print_status "Checking remaining services..."
echo ""

running_containers=$(docker ps --filter "name=raworc_" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}" 2>/dev/null || echo "")

if [ -n "$running_containers" ] && [ "$running_containers" != "NAMES	STATUS	PORTS" ]; then
    echo "$running_containers"
    echo ""
    print_warning "Some Raworc containers are still running"
else
    print_success "No Raworc containers are running"
fi

echo ""
print_success "🛑 Stop completed!"

if [ "$REMOVE" = false ]; then
    echo ""
    print_status "Services stopped but containers preserved."
    print_status "To start again: ./scripts/start.sh"
    print_status "To remove containers: ./scripts/stop.sh --remove"
fi

if [ "$VOLUMES" = false ] && [ "$REMOVE" = true ]; then
    echo ""
    print_status "Containers removed but volumes preserved."
    print_status "To remove volumes: ./scripts/stop.sh --volumes"
fi