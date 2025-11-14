#!/bin/bash

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Get project version from Cargo.toml
if [ -f "$PROJECT_ROOT/Cargo.toml" ]; then
  PROJECT_VERSION=$(grep '^version = ' "$PROJECT_ROOT/Cargo.toml" | cut -d'"' -f2)
  TAG="$PROJECT_VERSION"
else
  TAG="latest"
fi

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
  echo "Build Docker images for TSBX components"
  echo ""
  echo "Components:"
  echo "  api         Build the api image"
  echo "  controller  Build the controller image"
  echo "  sandbox     Build the sandbox image"
  echo "  operator    Build the operator UI image"
  echo "  gateway     Build the gateway image"
  echo "  all         Build all components (api, sandbox, controller, operator, gateway)"
  echo ""
  echo "Options:"
  echo "  -n, --no-cache          Build without cache"
  echo "  -h, --help              Show this help message"
  echo ""
  echo "Examples:"
  echo "  $0                            Build all components"
  echo "  $0 api controller             Build only api and controller"
  echo "  $0 --no-cache api             Build api without cache"
}

# Parse command line arguments
NO_CACHE=false
COMPONENTS=()

while [[ $# -gt 0 ]]; do
  case $1 in
  -n | --no-cache)
    NO_CACHE=true
    shift
    ;;
  -h | --help)
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
  COMPONENTS=("all")
fi

# Expand 'all' to actual components (ensure sandbox precedes controller)
if [[ " ${COMPONENTS[*]} " =~ " all " ]]; then
  COMPONENTS=("api" "sandbox" "controller" "operator" "gateway")
fi

print_status "Building TSBX Docker images"
if [[ -n "${PROJECT_VERSION:-}" ]]; then
  print_status "Tag: $TAG (from Cargo.toml $PROJECT_VERSION)"
else
  print_status "Tag: $TAG"
fi
print_status "Components: ${COMPONENTS[*]}"

# Change to project root
cd "$PROJECT_ROOT"

# Check if Rust is installed
if ! command -v cargo >/dev/null 2>&1; then
  print_error "Cargo (Rust) is not installed. Please install Rust first."
  exit 1
fi

# Check if Docker is installed
if ! command -v docker >/dev/null 2>&1; then
  print_error "Docker is not installed. Please install Docker first."
  exit 1
fi

# Build Rust binaries first
print_status "Building Rust binaries..."
if cargo build --release --bins; then
  print_success "Rust binaries built successfully"
else
  print_error "Failed to build Rust binaries"
  exit 1
fi

echo ""

# Build components
for component in "${COMPONENTS[@]}"; do
  case $component in
  api)
    image_name="tsbx_api:${TAG}"
    dockerfile="Dockerfile.api"
    ;;
  controller)
    image_name="tsbx_controller:${TAG}"
    dockerfile="Dockerfile.controller"
    ;;
  sandbox)
    image_name="tsbx_sandbox:${TAG}"
    dockerfile="Dockerfile.sandbox"
    ;;
  operator)
    image_name="tsbx_operator:${TAG}"
    dockerfile="Dockerfile.operator"
    # Build Operator (npm) outside Docker for speed and reproducibility
    print_status "Cleaning Operator build caches (.svelte-kit, build, Vite cache)"
    (cd operator && rm -rf .svelte-kit build node_modules/.vite 2>/dev/null || true)
    print_status "Installing Operator dependencies (npm ci)"
    (cd operator && npm ci)
    print_status "Building Operator (npm run build)"
    (cd operator && npm run build)
    print_status "Installing production deps for runtime (npm ci --omit=dev)"
    (cd operator && npm ci --omit=dev)
    # If an Operator container exists, remove it so the next start uses the fresh image
    if docker ps -a --format '{{.Names}}' | grep -q '^tsbx_operator$'; then
      print_status "Removing existing tsbx_operator container to avoid stale UI"
      docker rm -f tsbx_operator >/dev/null 2>&1 || true
    fi
    ;;
  gateway)
    image_name="tsbx_gateway:${TAG}"
    dockerfile="Dockerfile.gateway"
    ;;
  *)
    print_warning "Unknown component: $component. Skipping..."
    continue
    ;;
  esac

  if [ ! -f "$dockerfile" ]; then
    print_error "Dockerfile not found: $dockerfile"
    continue
  fi

  print_status "Building $component ($image_name)..."

  # Build Docker image
  build_cmd="docker build -f $dockerfile -t $image_name"

  if [ "$NO_CACHE" = true ]; then
    build_cmd="$build_cmd --no-cache"
  fi

  build_cmd="$build_cmd ."

  if eval "$build_cmd"; then
    print_success "Built $component successfully"
  else
    print_error "Failed to build $component"
    exit 1
  fi

  echo ""
done

print_success "Build completed!"

echo ""
print_status "Built images:"
for component in "${COMPONENTS[@]}"; do
  case $component in
  api | controller | sandbox | operator | gateway)
    echo "  tsbx_${component}:${TAG}"
    ;;
  esac
done

echo ""
print_status "To push images to a registry, run:"
echo "  ./scripts/push.sh ${COMPONENTS[*]}"
print_status "To start services with these images, run:"
echo "  tsbx start"
