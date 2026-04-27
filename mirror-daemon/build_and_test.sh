#!/bin/bash
# Build and basic smoke test for mirror-daemon

set -e

echo "================================"
echo "Mirror Daemon - Build & Test"
echo "================================"
echo

# Check Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust/Cargo not found. Install from https://rustup.rs/"
    exit 1
fi

echo "✓ Rust/Cargo found"

# Check Nu is installed (optional, but needed for pipelines)
if ! command -v nu &> /dev/null; then
    echo "⚠ Warning: Nushell (nu) not found. Pipeline execution will fail."
    echo "  Install from: https://www.nushell.sh/"
else
    echo "✓ Nushell found"
fi

echo

# Build
echo "Building mirror-daemon..."
cargo build --release

if [ $? -eq 0 ]; then
    echo "✓ Build successful"
else
    echo "✗ Build failed"
    exit 1
fi

echo

# Run tests
echo "Running tests..."
cargo test

if [ $? -eq 0 ]; then
    echo "✓ Tests passed"
else
    echo "✗ Tests failed"
    exit 1
fi

echo

# Create test directories
echo "Setting up test environment..."
rm -rf test-ledger test-pipelines
mkdir -p test-ledger test-pipelines

# Create a simple test pipeline
cat > test-pipelines/test.nu << 'EOF'
#!/usr/bin/env nu
print "Test pipeline executed"

let result = {
    status: "success",
    timestamp: (date now | format date "%Y-%m-%d %H:%M:%S")
}

$result | to json | save test_output.json
EOF

echo "✓ Test environment ready"
echo

# Run basic CLI commands
echo "Testing CLI commands..."

echo "  - List (should be empty)..."
./target/release/mirror --ledger test-ledger --pipelines test-pipelines list

echo "  - Stats (should be zero)..."
./target/release/mirror --ledger test-ledger --pipelines test-pipelines stats

if command -v nu &> /dev/null; then
    echo "  - Run test pipeline..."
    ./target/release/mirror --ledger test-ledger --pipelines test-pipelines run test.nu
    
    echo "  - Recent executions..."
    ./target/release/mirror --ledger test-ledger --pipelines test-pipelines recent --limit 5
    
    echo "  - Stats after execution..."
    ./target/release/mirror --ledger test-ledger --pipelines test-pipelines stats
else
    echo "  ⚠ Skipping pipeline execution (Nushell not installed)"
fi

echo
echo "================================"
echo "✓ All checks passed!"
echo "================================"
echo
echo "Next steps:"
echo "  1. Copy example pipelines: cp examples/pipelines/* test-pipelines/"
echo "  2. Run a pipeline: ./target/release/mirror --ledger test-ledger --pipelines test-pipelines run cashflow.nu"
echo "  3. Explore the ledger: tree test-ledger/"
echo
echo "Or read GETTING_STARTED.md for full instructions."
