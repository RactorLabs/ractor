#!/bin/bash

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
CLI_DIR="$PROJECT_ROOT/cli"

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
    echo "Link the TaskSandbox CLI for development (npm link)"
    echo "This creates a live development link so changes reflect immediately"
    echo ""
    echo "Options:"
    echo "  -f, --force             Force relink (unlink first)"
    echo "  -h, --help              Show this help message"
    echo ""
    echo "What this does:"
    echo "  1. Install npm dependencies in cli/"
    echo "  2. Create development link with 'npm link'"
    echo "  3. Make 'tsbx' command available globally (linked to source)"
    echo ""
    echo "Examples:"
    echo "  $0                      Link CLI for development"
    echo "  $0 --force              Force relink (unlink first)"
}

# Parse command line arguments
FORCE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -f|--force)
            FORCE=true
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

print_status "🔗 Linking TaskSandbox CLI for development"

# Check if Node.js is installed
if ! command -v node >/dev/null 2>&1; then
    print_error "Node.js is not installed. Please install Node.js first."
    print_status "Visit: https://nodejs.org/"
    exit 1
fi

# Check if npm is installed
if ! command -v npm >/dev/null 2>&1; then
    print_error "npm is not installed. Please install npm first."
    exit 1
fi

# Show Node.js and npm versions
node_version=$(node --version)
npm_version=$(npm --version)
print_status "Node.js version: $node_version"
print_status "npm version: $npm_version"

# Check if CLI directory exists
if [ ! -d "$CLI_DIR" ]; then
    print_error "CLI directory not found: $CLI_DIR"
    exit 1
fi

# Check if package.json exists
if [ ! -f "$CLI_DIR/package.json" ]; then
    print_error "package.json not found in CLI directory"
    exit 1
fi

# Change to CLI directory
cd "$CLI_DIR"

echo ""
print_status "Linking from: $CLI_DIR"

# Step 1: Install dependencies
print_status "Installing npm dependencies..."
if npm install; then
    print_success "Dependencies installed successfully"
else
    print_error "Failed to install dependencies"
    exit 1
fi

echo ""

# Step 2: Unlink if force option or already linked
if [ "$FORCE" = true ]; then
    print_status "Force option detected, unlinking first..."
    npm unlink -g tsbx-cli 2>/dev/null || true
fi

# Step 3: Create development link
print_status "Creating development link..."

if npm link; then
    print_success "Development link created successfully"
    print_status "The 'tsbx' command is now linked to your source code"
else
    print_error "Failed to create development link"
    print_status "You may need to run with elevated privileges:"
    print_status "  sudo $0"
    exit 1
fi

echo ""
print_success "🎉 Development link completed!"

# Show usage instructions
echo ""
print_status "Development Workflow:"
echo "  1. Edit source code:     vim cli/commands/start.js"
echo "  2. Test immediately:     tsbx start"
echo "  3. Changes are live:     No reinstall needed!"

echo ""
print_status "Available Commands:"
echo "  tsbx --help           # Show help"
echo "  tsbx start            # Start services"
echo "  tsbx auth login       # Authenticate"
echo "  tsbx api version      # Check API"
echo "  tsbx session create     # Interactive session"

# Show installed version
package_version=$(node -p "require('./package.json').version" 2>/dev/null || echo "unknown")
echo ""
print_status "Linked TaskSandbox CLI version: $package_version"

# Check if tsbx command is available
echo ""
if command -v tsbx >/dev/null 2>&1; then
    print_success "✓ 'tsbx' command is available (linked to development source)"
    tsbx --version 2>/dev/null || echo "  Run 'tsbx --help' to get started"
    
    # Show what the link points to
    which_tsbx=$(which tsbx 2>/dev/null || echo "unknown")
    if [ "$which_tsbx" != "unknown" ]; then
        print_status "Link location: $which_tsbx"
        if [ -L "$which_tsbx" ]; then
            link_target=$(readlink "$which_tsbx" 2>/dev/null || echo "unknown")
            print_status "Points to: $link_target"
        fi
    fi
else
    print_warning "⚠ 'tsbx' command not found in PATH"
    print_status "You may need to restart your terminal or add npm global bin to PATH"
    print_status "Global npm bin: $(npm bin -g 2>/dev/null || echo 'unknown')"
fi

echo ""
print_status "💡 Development Tips:"
echo "  • Edit any file in cli/ - changes take effect immediately"
echo "  • No need to reinstall after code changes"  
echo "  • Use 'scripts/install.sh' for stable global installation"
echo "  • Use 'npm unlink -g tsbx-cli' to remove the link"
