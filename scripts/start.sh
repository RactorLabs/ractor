#!/bin/bash

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Load environment from .env if present (so OLLAMA_* and others are honored)
if [ -f "$PROJECT_ROOT/.env" ]; then
    # Export all variables defined in .env
    set -a
    # shellcheck disable=SC1091
    source "$PROJECT_ROOT/.env"
    set +a
fi

# Get project version from Cargo.toml
if [ -f "$PROJECT_ROOT/Cargo.toml" ]; then
    PROJECT_VERSION=$(grep '^version = ' "$PROJECT_ROOT/Cargo.toml" | cut -d'"' -f2)
    TAG="$PROJECT_VERSION"
else
    TAG="latest"
fi

# Use local image names (built by build.sh)
SERVER_IMAGE="raworc_server:${TAG}"
OPERATOR_IMAGE="raworc_operator:${TAG}"
AGENT_IMAGE="raworc_agent:${TAG}"

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
    echo "Start Raworc services using direct Docker container management"
    echo ""
    echo "Components:"
    echo "  mysql       Start only the MySQL database"
    echo "  ollama      Start only the Ollama runtime"
    echo "  server      Start only the API server"
    echo "  operator    Start only the operator service"
    echo "  (default)   Start server and operator"
    echo ""
    echo "Options:"
    echo "  -b, --build             Build images before starting"
    echo "  -p, --pull              Pull base images (mysql) before starting"
    echo "  -d, --detached          Run in detached mode (default)"
    echo "  -f, --foreground        Run MySQL in foreground mode"
    echo "  -h, --help              Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                      Start server and operator"
    echo "  $0 --build              Build images and start server/operator"
    echo "  $0 mysql server         Start only database and server"
    echo "  $0 --pull               Pull base images and start"
}

# Parse command line arguments
BUILD=false
PULL=false
DETACHED=true
COMPONENTS=()

while [[ $# -gt 0 ]]; do
    case $1 in
        -b|--build)
            BUILD=true
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

# Default to server and operator if none specified
if [ ${#COMPONENTS[@]} -eq 0 ]; then
    COMPONENTS=("server" "operator")
fi

print_status "Starting Raworc services with direct Docker management"
print_status "Image tag: $TAG (from Cargo.toml $PROJECT_VERSION)"
print_status "Build images: $BUILD"
print_status "Pull base images: $PULL"
print_status "Detached mode: $DETACHED"
print_status "Components: ${COMPONENTS[*]}"

# Change to project root
cd "$PROJECT_ROOT"

# Check if Docker is available
if ! command -v docker >/dev/null 2>&1; then
    print_error "Docker is not available. Please install Docker first."
    exit 1
fi

echo ""

# Build images if requested
if [ "$BUILD" = true ]; then
    print_status "Building images..."
    
    build_components=()
    for component in "${COMPONENTS[@]}"; do
        case $component in
            server|operator|agent)
                build_components+=("$component")
                ;;
        esac
    done
    
    if [ ${#build_components[@]} -gt 0 ]; then
        if "$SCRIPT_DIR/build.sh" "${build_components[@]}"; then
            print_success "Images built successfully"
        else
            print_error "Failed to build images"
            exit 1
        fi
    else
        print_status "No images to build for selected components"
    fi
    echo ""
fi

# Pull base images if requested
if [ "$PULL" = true ]; then
    print_status "Pulling base images..."
    if docker pull mysql:8.0; then
        print_success "Base images pulled"
    else
        print_warning "Failed to pull base images, continuing..."
    fi
    echo ""
fi

# Create network if it doesn't exist
print_status "Creating Docker network..."
if ! docker network inspect raworc_network >/dev/null 2>&1; then
    if docker network create raworc_network; then
        print_success "Created raworc_network"
    else
        print_error "Failed to create Docker network"
        exit 1
    fi
else
    print_success "Network raworc_network already exists"
fi

echo ""

# Create volumes if they don't exist
print_status "Creating Docker volumes..."
for volume in raworc_mysql_data raworc_public_data raworc_ollama_data; do
    if ! docker volume inspect "$volume" >/dev/null 2>&1; then
        if docker volume create "$volume"; then
            print_success "Created volume $volume"
        else
            print_error "Failed to create volume $volume"
            exit 1
        fi
    else
        print_success "Volume $volume already exists"
    fi
done

echo ""

# Start components
for component in "${COMPONENTS[@]}"; do
    case $component in
        mysql)
            print_status "Starting MySQL database..."
            
            # Stop existing container if running
            if docker ps -q --filter "name=raworc_mysql" | grep -q .; then
                print_status "Stopping existing MySQL container..."
                docker stop raworc_mysql >/dev/null 2>&1 || true
            fi
            
            # Remove existing container if exists
            if docker ps -aq --filter "name=raworc_mysql" | grep -q .; then
                print_status "Removing existing MySQL container..."
                docker rm raworc_mysql >/dev/null 2>&1 || true
            fi
            
            # Start MySQL container
            docker_cmd="docker run"
            if [ "$DETACHED" = true ]; then
                docker_cmd="$docker_cmd -d"
            fi
            
            if eval "$docker_cmd --name raworc_mysql \
                --network raworc_network \
                -p 3307:3306 \
                -v raworc_mysql_data:/var/lib/mysql \
                -e MYSQL_ROOT_PASSWORD=root \
                -e MYSQL_DATABASE=raworc \
                -e MYSQL_USER=raworc \
                -e MYSQL_PASSWORD=raworc \
                --health-cmd=\"mysqladmin ping -h localhost -u root -proot\" \
                --health-interval=10s \
                --health-timeout=5s \
                --health-retries=5 \
                mysql:8.0 \
                --default-authentication-plugin=mysql_native_password \
                --collation-server=utf8mb4_unicode_ci \
                --character-set-server=utf8mb4"; then
                print_success "MySQL container started"
            else
                print_error "Failed to start MySQL container"
                exit 1
            fi
            ;;

        ollama)
            print_status "Starting Ollama runtime..."

            # Stop existing container if running
            if docker ps -q --filter "name=raworc_ollama" | grep -q .; then
                print_status "Stopping existing Ollama container..."
                docker stop raworc_ollama >/dev/null 2>&1 || true
            fi

            # Remove existing container if exists
            if docker ps -aq --filter "name=raworc_ollama" | grep -q .; then
                print_status "Removing existing Ollama container..."
                docker rm raworc_ollama >/dev/null 2>&1 || true
            fi

            # Optional GPU support
            GPU_FLAGS=""
            if [ "${OLLAMA_ENABLE_GPU:-}" = "true" ]; then
                GPU_FLAGS="--gpus all"
                print_status "GPU enabled for Ollama"
            fi

            # Optional resource limits
            CPU_FLAG=""
            MEM_FLAG=""
            SHM_FLAG=""
            if [ -n "${OLLAMA_CPUS:-}" ]; then
                CPU_FLAG="--cpus ${OLLAMA_CPUS}"
                print_status "Allocating CPUs to Ollama: ${OLLAMA_CPUS}"
            fi
            if [ -n "${OLLAMA_MEMORY:-}" ]; then
                MEM_FLAG="--memory ${OLLAMA_MEMORY} --memory-swap ${OLLAMA_MEMORY}"
                print_status "Allocating memory to Ollama: ${OLLAMA_MEMORY}"
            fi
            # Larger shared memory can help with big models
            if [ -n "${OLLAMA_SHM_SIZE:-}" ]; then
                SHM_FLAG="--shm-size ${OLLAMA_SHM_SIZE}"
                print_status "Shared memory size for Ollama: ${OLLAMA_SHM_SIZE}"
            fi

            # Provide sane defaults if not provided via env
            if [ -z "${OLLAMA_MEMORY:-}" ]; then
                OLLAMA_MEMORY="24g"
                MEM_FLAG="--memory ${OLLAMA_MEMORY} --memory-swap ${OLLAMA_MEMORY}"
                print_status "No OLLAMA_MEMORY set; defaulting to ${OLLAMA_MEMORY}"
            fi
            if [ -z "${OLLAMA_SHM_SIZE:-}" ]; then
                OLLAMA_SHM_SIZE="24g"
                SHM_FLAG="--shm-size ${OLLAMA_SHM_SIZE}"
                print_status "No OLLAMA_SHM_SIZE set; defaulting to ${OLLAMA_SHM_SIZE}"
            fi

            # Use host port mapping only if 11434 is free
            PUBLISH_FLAG="-p 11434:11434"
            if ss -ltn 2>/dev/null | awk '{print $4}' | grep -q ':11434$'; then
                print_warning "Host port 11434 is in use; starting without host port mapping"
                PUBLISH_FLAG=""
            fi

            # Start Ollama container
            if docker run -d \
                --name raworc_ollama \
                --network raworc_network \
                ${PUBLISH_FLAG} \
                -v raworc_ollama_data:/root/.ollama \
                -e OLLAMA_KEEP_ALIVE=1h \
                $GPU_FLAGS \
                $CPU_FLAG \
                $MEM_FLAG \
                $SHM_FLAG \
                ollama/ollama:latest; then
                print_success "Ollama container started"
            else
                print_error "Failed to start Ollama container"
                exit 1
            fi

            # Wait for Ollama to be ready
            timeout=120
            if [ -n "${PUBLISH_FLAG}" ]; then
                print_status "Waiting for Ollama to be ready on host :11434..."
                until curl -fsS http://localhost:11434/api/tags >/dev/null 2>&1; do
                    sleep 2
                    timeout=$((timeout-2))
                    if [ $timeout -le 0 ]; then
                        print_error "Ollama did not become ready in time (host port)"
                        exit 1
                    fi
                done
            else
                print_status "Waiting for Ollama container to be ready (no host port)..."
                until docker exec raworc_ollama ollama list >/dev/null 2>&1; do
                    sleep 2
                    timeout=$((timeout-2))
                    if [ $timeout -le 0 ]; then
                        print_error "Ollama did not become ready in time (container)"
                        exit 1
                    fi
                done
            fi
            print_success "Ollama is ready"

            # Ensure gpt-oss model is available
            print_status "Pulling gpt-oss:20b model (if needed)..."
            if docker exec raworc_ollama ollama pull gpt-oss:20b >/dev/null 2>&1; then
                print_success "gpt-oss:20b model available"
            else
                print_warning "Failed to pull gpt-oss:20b model. You may need to pull manually."
            fi

            ;;
            
        server)
            print_status "Starting API server..."
            
            # Check if MySQL is running and healthy (if it was requested)
            if [[ " ${COMPONENTS[*]} " =~ " mysql " ]] || docker ps --filter "name=raworc_mysql" --format "{{.Names}}" | grep -q "raworc_mysql"; then
                print_status "Waiting for MySQL to be healthy..."
                timeout=60
                while [ $timeout -gt 0 ]; do
                    if docker exec raworc_mysql mysqladmin ping -h localhost -u root -proot >/dev/null 2>&1; then
                        print_success "MySQL is ready"
                        break
                    fi
                    print_status "Waiting for MySQL... ($timeout seconds left)"
                    sleep 2
                    timeout=$((timeout-2))
                done
                
                if [ $timeout -le 0 ]; then
                    print_error "MySQL failed to become healthy"
                    exit 1
                fi
            fi
            
            # Stop existing container if running
            if docker ps -q --filter "name=raworc_server" | grep -q .; then
                print_status "Stopping existing server container..."
                docker stop raworc_server >/dev/null 2>&1 || true
            fi
            
            # Remove existing container if exists
            if docker ps -aq --filter "name=raworc_server" | grep -q .; then
                print_status "Removing existing server container..."
                docker rm raworc_server >/dev/null 2>&1 || true
            fi
            
            # Start server container
            if docker run -d \
                --name raworc_server \
                --network raworc_network \
                -p 9000:9000 \
                -p 8000:8000 \
                -v ./logs:/app/logs \
                -v raworc_public_data:/public \
                -e DATABASE_URL=mysql://raworc:raworc@raworc_mysql:3306/raworc \
                -e JWT_SECRET="${JWT_SECRET:-development-secret-key}" \
                -e RUST_LOG=info \
                "$SERVER_IMAGE"; then
                print_success "API server container started"
            else
                print_error "Failed to start API server container"
                exit 1
            fi
            ;;
            
        operator)
            print_status "Starting operator service..."

            # Set OLLAMA_HOST default: prefer raworc_ollama if running
            if docker ps -q --filter "name=raworc_ollama" | grep -q .; then
                : "${OLLAMA_HOST:=http://raworc_ollama:11434}"
            else
                : "${OLLAMA_HOST:=http://host.docker.internal:11434}"
            fi
            
            # Check if MySQL is running and healthy (if it was requested)
            if [[ " ${COMPONENTS[*]} " =~ " mysql " ]] || docker ps --filter "name=raworc_mysql" --format "{{.Names}}" | grep -q "raworc_mysql"; then
                print_status "Waiting for MySQL to be healthy..."
                timeout=60
                while [ $timeout -gt 0 ]; do
                    if docker exec raworc_mysql mysqladmin ping -h localhost -u root -proot >/dev/null 2>&1; then
                        print_success "MySQL is ready"
                        break
                    fi
                    print_status "Waiting for MySQL... ($timeout seconds left)"
                    sleep 2
                    timeout=$((timeout-2))
                done
                
                if [ $timeout -le 0 ]; then
                    print_error "MySQL failed to become healthy"
                    exit 1
                fi
            fi
            
            # Stop existing container if running
            if docker ps -q --filter "name=raworc_operator" | grep -q .; then
                print_status "Stopping existing operator container..."
                docker stop raworc_operator >/dev/null 2>&1 || true
            fi
            
            # Remove existing container if exists
            if docker ps -aq --filter "name=raworc_operator" | grep -q .; then
                print_status "Removing existing operator container..."
                docker rm raworc_operator >/dev/null 2>&1 || true
            fi
            
            # Start operator container
            if docker run -d \
                --name raworc_operator \
                --network raworc_network \
                -v /var/run/docker.sock:/var/run/docker.sock \
                -e DATABASE_URL=mysql://raworc:raworc@raworc_mysql:3306/raworc \
                -e JWT_SECRET="${JWT_SECRET:-development-secret-key}" \
                -e OLLAMA_HOST="$OLLAMA_HOST" \
                -e OLLAMA_MODEL="${OLLAMA_MODEL:-gpt-oss:20b}" \
                -e AGENT_IMAGE="$AGENT_IMAGE" \
                -e AGENT_CPU_LIMIT="0.5" \
                -e AGENT_MEMORY_LIMIT="536870912" \
                -e AGENT_DISK_LIMIT="1073741824" \
                -e RUST_LOG=info \
                "$OPERATOR_IMAGE"; then
                print_success "Operator service container started"
            else
                print_error "Failed to start operator service container"
                exit 1
            fi
            ;;
            
        *)
            print_warning "Unknown component: $component. Skipping..."
            continue
            ;;
    esac
    
    echo ""
done

# Show running services status
print_status "Checking running services..."
echo ""

running_containers=$(docker ps --filter "name=raworc_" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}")

if [ -n "$running_containers" ]; then
    echo "$running_containers"
    echo ""
    print_success "🎉 Raworc services are now running!"
    
    echo ""
    print_status "Service URLs:"
    if docker ps --filter "name=raworc_server" --format "{{.Names}}" | grep -q "raworc_server"; then
        echo "  • API Server: http://localhost:9000"
        echo "  • Public Content: http://localhost:8000"
    fi
    if docker ps --filter "name=raworc_mysql" --format "{{.Names}}" | grep -q "raworc_mysql"; then
        echo "  • MySQL Port: 3307"
    fi
    
    echo ""
    print_status "Next steps:"
    echo "  • Check logs: docker logs raworc_server -f"
    echo "  • Authenticate: raworc login -u admin -p admin"
    echo "  • Check version: raworc api version"
    echo "  • Start agent: raworc agent create"
    echo ""
    print_status "Container management:"
    echo "  • Stop services: ./scripts/stop.sh"
    echo "  • View logs: docker logs <container_name>"
    echo "  • Check status: docker ps --filter 'name=raworc_'"
else
    print_error "No Raworc containers are running"
    exit 1
fi
