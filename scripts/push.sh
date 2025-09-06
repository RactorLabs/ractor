#!/bin/bash

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Get project version from Cargo.toml
if [ -f "$PROJECT_ROOT/Cargo.toml" ]; then
  PROJECT_VERSION=$(grep '^version = ' "$PROJECT_ROOT/Cargo.toml" | cut -d'"' -f2)
  DEFAULT_TAG="$PROJECT_VERSION"
else
  DEFAULT_TAG="latest"
fi

DEFAULT_REGISTRY="raworc"

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
  echo "Push Docker images for Raworc components to registry"
  echo ""
  echo "Components:"
  echo "  server      Push the server image"
  echo "  controller  Push the controller image"
  echo "  agent       Push the agent image"
  echo "  all         Push all components (default)"
  echo ""
  echo "Options:"
  echo "  -t, --tag TAG           Docker image tag (default: $DEFAULT_TAG from Cargo.toml)"
  echo "  -r, --registry REGISTRY Registry/user prefix (default: $DEFAULT_REGISTRY)"
  echo "  -h, --help              Show this help message"
  echo ""
  echo "Examples:"
  echo "  $0                      Push all components with version tag ($DEFAULT_TAG)"
  echo "  $0 server controller    Push only server and controller"
  echo "  $0 --tag latest         Push all components with 'latest' tag"
  echo "  $0 --registry myuser    Push to myuser Docker Hub account"
}

# Parse command line arguments
TAG="$DEFAULT_TAG"
REGISTRY="$DEFAULT_REGISTRY"
COMPONENTS=()

while [[ $# -gt 0 ]]; do
  case $1 in
  -t | --tag)
    TAG="$2"
    shift 2
    ;;
  -r | --registry)
    REGISTRY="$2"
    shift 2
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

# Set default components if none specified
if [ ${#COMPONENTS[@]} -eq 0 ]; then
  COMPONENTS=("server" "controller" "agent" "operator")
fi

print_status "Pushing Raworc Docker images"
print_status "Tag: $TAG"
print_status "Registry: $REGISTRY"
print_status "Components: ${COMPONENTS[*]}"

# Change to project root
cd "$PROJECT_ROOT"

# Check if Docker is installed
if ! command -v docker >/dev/null 2>&1; then
  print_error "Docker is not installed. Please install Docker first."
  exit 1
fi

# Check Docker login
print_status "Checking Docker registry login..."
if ! docker info >/dev/null 2>&1; then
  print_error "Docker daemon is not running"
  exit 1
fi

echo ""

# Push components
for component in "${COMPONENTS[@]}"; do
  case $component in
  server)
    image_name="${REGISTRY}/raworc_server:${TAG}"
    ;;
  controller)
    image_name="${REGISTRY}/raworc_controller:${TAG}"
    ;;
  agent)
    image_name="${REGISTRY}/raworc_agent:${TAG}"
    ;;
  operator)
    image_name="${REGISTRY}/raworc_operator:${TAG}"
    ;;
  *)
    print_warning "Unknown component: $component. Skipping..."
    continue
    ;;
  esac

  print_status "Pushing $component ($image_name)..."

  # Check if local image exists (built by build.sh uses project version)
  local_image="raworc_${component}:${TAG}"
  if ! docker images --format "{{.Repository}}:{{.Tag}}" | grep -q "^${local_image}$"; then
    print_error "Local image $local_image not found. Build it first with:"
    print_error "  ./scripts/build.sh $component"
    exit 1
  fi

  # Tag local image with registry prefix for pushing
  print_status "Tagging $local_image as $image_name..."
  if ! docker tag "$local_image" "$image_name"; then
    print_error "Failed to tag image $local_image"
    exit 1
  fi

  # Also tag as 'latest' if we're pushing a version tag
  latest_image="${REGISTRY}/raworc_${component}:latest"
  if [ "$TAG" != "latest" ]; then
    print_status "Tagging $local_image as $latest_image..."
    if ! docker tag "$local_image" "$latest_image"; then
      print_error "Failed to tag image as latest"
      exit 1
    fi
  fi

  # Push version tag
  print_status "Pushing $image_name..."
  if docker push "$image_name"; then
    print_success "Pushed $image_name successfully"
  else
    print_error "Failed to push $image_name"
    exit 1
  fi

  # Push latest tag if different from version tag
  if [ "$TAG" != "latest" ]; then
    print_status "Pushing $latest_image..."
    if docker push "$latest_image"; then
      print_success "Pushed $latest_image successfully"
    else
      print_error "Failed to push $latest_image"
      exit 1
    fi
  fi

  echo ""
done

print_success "Push completed!"

echo ""
print_status "Pushed images:"
for component in "${COMPONENTS[@]}"; do
  case $component in
  server | controller | agent | operator)
    echo "  ${REGISTRY}/raworc_${component}:${TAG}"
    if [ "$TAG" != "latest" ]; then
      echo "  ${REGISTRY}/raworc_${component}:latest"
    fi
    ;;
  esac
done
