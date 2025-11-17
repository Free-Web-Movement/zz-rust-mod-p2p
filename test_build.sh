# test_build.sh

#!/usr/bin/env bash

set -e

# Enable Rust coverage instrumentation
export RUSTFLAGS="-C instrument-coverage -Awarnings"
export LLVM_PROFILE_FILE=".output/cargo-test-%p-%m.profraw"

# Run tests
echo "Running cargo tests with coverage instrumentation..."
cargo test --tests -- --nocapture --show-output

# Find all .profraw files
PROFRAW_FILES=$(find . -name ".output/*.profraw")

# Generate coverage report if grcov is installed
rm -rf coverages || true
if command -v grcov &> /dev/null; then
    echo "Generating coverage report with grcov..."
    cargo llvm-cov nextest --html --output-dir coverages
    xdg-open coverages/html/index.html
else
    echo "grcov not found. Skipping coverage report generation."
    echo "Install grcov for coverage reports: cargo install grcov"
fi

# Clean up .profraw files
rm -rf .output/

echo "Test and coverage script completed."