# test_build.sh

#!/usr/bin/env bash

set -e

sudo apt install gcc 

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
if ! [command -v grcov &> /dev/null]; then
    echo "Installing grcov"
    cargo install grcov
fi

if ! [command -v llvm-cov &> /dev/null]; then
    echo "Installing llvm-cov"
    cargo install cargo-llvm-cov
fi

if ! [command -v nextest &> /dev/null]; then
    echo "Installing cargo-nextest"
    cargo install cargo-nextest
fi

echo "Generating coverage report with grcov..."
cargo llvm-cov nextest --html --output-dir coverages

# Clean up .profraw files
rm -rf .output/
echo "Test and coverage script completed."

# Show result in browser

xdg-open coverages/html/index.html
