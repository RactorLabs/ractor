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
    echo "Publish the Raworc CLI npm package"
    echo ""
    echo "Options:"
    echo "  --dry-run               Show what would be published without actually publishing"
    echo "  --tag TAG               npm dist-tag (default: latest)"
    echo "  -h, --help              Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                      Publish to npm with 'latest' tag"
    echo "  $0 --tag beta           Publish with 'beta' tag"
    echo "  $0 --dry-run            Check what would be published"
    echo ""
    echo "Prerequisites:"
    echo "  • npm login (run 'npm login' first)"
    echo "  • Update version in cli/package.json if needed"
    echo ""
    echo "Note: For Docker images, use ./scripts/push.sh instead"
}

# Parse command line arguments
DRY_RUN=false
NPM_TAG="latest"

while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --tag)
            NPM_TAG="$2"
            shift 2
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

print_status "Publishing Raworc NPM CLI package"
print_status "Tag: $NPM_TAG"
print_status "Dry run: $DRY_RUN"

# Change to project root
cd "$PROJECT_ROOT"

# Check if npm is installed
if ! command -v npm >/dev/null 2>&1; then
    print_error "npm is not installed. Please install Node.js and npm first."
    exit 1
fi

# Check CLI directory exists
CLI_DIR="$PROJECT_ROOT/cli"
if [ ! -d "$CLI_DIR" ]; then
    print_error "CLI directory not found: $CLI_DIR"
    exit 1
fi

cd "$CLI_DIR"

# Check if package.json exists
if [ ! -f "package.json" ]; then
    print_error "package.json not found in CLI directory"
    exit 1
fi

# Get package info
current_version=$(node -p "require('./package.json').version" 2>/dev/null || echo "unknown")
package_name=$(node -p "require('./package.json').name" 2>/dev/null || echo "unknown")

print_status "Package: $package_name@$current_version"

if [ "$DRY_RUN" = false ]; then
    # Check npm login
    print_status "Verifying npm login..."
    if ! npm whoami >/dev/null 2>&1; then
        print_error "Not logged in to npm. Please run 'npm login' first."
        exit 1
    fi
    
    npm_user=$(npm whoami)
    print_status "NPM logged in as: $npm_user"

    # Check if this version already exists
    if npm view "$package_name@$current_version" version >/dev/null 2>&1; then
        print_warning "Version $current_version already exists on npm"
        print_status "Please update the version in package.json first:"
        print_status "  npm version patch   # 0.1.1 -> 0.1.2"
        print_status "  npm version minor   # 0.1.1 -> 0.2.0"
        print_status "  npm version major   # 0.1.1 -> 1.0.0"
        exit 1
    fi
fi

# Install dependencies
print_status "Installing dependencies..."
if ! npm install; then
    print_error "Failed to install NPM dependencies"
    exit 1
fi

echo ""

# Publish package
if [ "$DRY_RUN" = true ]; then
    print_status "🔍 Dry run - checking package contents..."
    if npm pack --dry-run; then
        print_success "✓ Package contents look good"
        print_status "To actually publish, run: $0"
    else
        print_error "✗ Package validation failed"
        exit 1
    fi
else
    print_status "📦 Publishing to npm registry..."
    
    if [ "$NPM_TAG" != "latest" ]; then
        # Publish with specific tag
        if npm publish --tag "$NPM_TAG"; then
            print_success "✓ Published $package_name@$current_version with tag '$NPM_TAG'"
        else
            print_error "✗ Failed to publish NPM package"
            exit 1
        fi
    else
        # Publish with default latest tag
        if npm publish; then
            print_success "✓ Published $package_name@$current_version to npm"
        else
            print_error "✗ Failed to publish NPM package"
            exit 1
        fi
    fi
fi

echo ""
print_success "🎉 NPM publishing completed!"
echo ""

if [ "$DRY_RUN" = false ]; then
    print_status "Package published:"
    print_status "  📦 $package_name@$current_version"
    print_status "  🏷️  Tag: $NPM_TAG"
    echo ""
    print_status "Install with:"
    print_status "  npm install -g $package_name"
    print_status ""
    print_status "Next steps:"
    print_status "  • Test installation: npm install -g $package_name"
    print_status "  • Verify CLI works: raworc --help"
    print_status "  • Update documentation if needed"
fi