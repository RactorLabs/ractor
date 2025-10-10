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
    echo "Install development dependencies for Ractor"
    echo ""
    echo "Options:"
    echo "  -v, --verbose           Show detailed output"
    echo "  -h, --help              Show this help message"
    echo ""
    echo "What this installs:"
    echo "  • Rust dependencies (cargo check)"
    echo "  • Node.js CLI dependencies (npm install)"
    echo "  • Verifies toolchain requirements"
    echo ""
    echo "Prerequisites:"
    echo "  • Rust toolchain (rustup)"
    echo "  • Node.js 16+ and npm"
    echo "  • Docker (for running services)"
}

# Parse command line arguments
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--verbose)
            VERBOSE=true
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

print_status "🔧 Installing Ractor development dependencies"

# Change to project root
cd "$PROJECT_ROOT"

echo ""

# Step 1: Check prerequisites
print_status "[1/4] Checking prerequisites..."

# Check Rust
if ! command -v cargo >/dev/null 2>&1; then
    print_error "Rust/Cargo is not installed."
    print_status "Install from: https://rustup.rs/"
    exit 1
fi

# Check Node.js
if ! command -v node >/dev/null 2>&1; then
    print_error "Node.js is not installed."
    print_status "Install from: https://nodejs.org/"
    exit 1
fi

# Check npm
if ! command -v npm >/dev/null 2>&1; then
    print_error "npm is not installed."
    print_status "npm usually comes with Node.js"
    exit 1
fi

# Check Docker
if ! command -v docker >/dev/null 2>&1; then
    print_warning "Docker is not installed. You'll need it to run services."
    print_status "Install from: https://docs.docker.com/get-docker/"
fi

# Show versions
rust_version=$(rustc --version 2>/dev/null | cut -d' ' -f2 || echo "unknown")
node_version=$(node --version 2>/dev/null || echo "unknown")
npm_version=$(npm --version 2>/dev/null || echo "unknown")

print_success "✓ Prerequisites found:"
print_status "  Rust: $rust_version"
print_status "  Node.js: $node_version"  
print_status "  npm: $npm_version"

echo ""

# Step 2: Install Rust dependencies
print_status "[2/4] Installing Rust dependencies..."

if [ "$VERBOSE" = true ]; then
    if cargo check; then
        print_success "✓ Rust dependencies installed and checked"
    else
        print_error "✗ Failed to install Rust dependencies"
        exit 1
    fi
else
    if cargo check --quiet; then
        print_success "✓ Rust dependencies installed and checked"
    else
        print_error "✗ Failed to install Rust dependencies"
        exit 1
    fi
fi

echo ""

# Step 3: Install Node.js CLI dependencies  
print_status "[3/4] Installing Node.js CLI dependencies..."

CLI_DIR="$PROJECT_ROOT/cli"
if [ ! -d "$CLI_DIR" ]; then
    print_error "CLI directory not found: $CLI_DIR"
    exit 1
fi

cd "$CLI_DIR"

if [ "$VERBOSE" = true ]; then
    if npm install; then
        print_success "✓ NPM dependencies installed"
    else
        print_error "✗ Failed to install NPM dependencies"
        exit 1
    fi
else
    if npm install --silent; then
        print_success "✓ NPM dependencies installed"
    else
        print_error "✗ Failed to install NPM dependencies"
        exit 1
    fi
fi

cd "$PROJECT_ROOT"

echo ""

# Step 4: Verify installation
print_status "[4/4] Verifying installation..."

# Check if Rust builds work
if cargo check --quiet >/dev/null 2>&1; then
    print_success "✓ Rust codebase compiles"
else
    print_warning "⚠ Rust compilation issues detected"
fi

# Check if CLI dependencies are OK
cd "$CLI_DIR"
if node index.js --help >/dev/null 2>&1; then
    print_success "✓ CLI runs correctly"
else
    print_warning "⚠ CLI issues detected"  
fi

cd "$PROJECT_ROOT"

echo ""
print_success "🎉 Installation completed!"

echo ""
print_status "Next steps:"
print_status "  1. Link CLI for development:    ./scripts/link.sh"
print_status "  2. Build services:              ./scripts/build.sh"
print_status "  3. Start services:              ractor start"
print_status "  4. Test authentication:         ractor auth login --user admin --pass admin"
print_status "  5. Start agent:                 ractor agent create"

echo ""
print_status "💡 Development workflow:"
print_status "  • Edit code in src/ or cli/"
print_status "  • Rebuild services: ./scripts/build.sh"
print_status "  • CLI changes are live (if linked)"
print_status "  • Stop/Start services: ractor stop && ractor start"
