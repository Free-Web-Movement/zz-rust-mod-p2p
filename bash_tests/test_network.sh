#!/bin/bash
# P2P Network Demo - Two nodes communicating

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$SCRIPT_DIR"

echo "=== P2P Network Demo ==="
echo ""

# Clean up any existing processes
echo "Cleaning up..."
pkill -f "zzp2p" 2>/dev/null || true
sleep 1

# Create test directories
NODE1_DIR="/tmp/p2p_node1"
NODE2_DIR="/tmp/p2p_node2"

rm -rf "$NODE1_DIR" "$NODE2_DIR"
mkdir -p "$NODE1_DIR" "$NODE2_DIR"

echo "Node1 directory: $NODE1_DIR"
echo "Node2 directory: $NODE2_DIR"
echo ""

# Build the project
echo "Building project..."
cd "$PROJECT_DIR"
cargo build --release 2>/dev/null

NODE1_BIN="./target/release/zzp2p"
NODE2_BIN="./target/release/zzp2p"

echo ""
echo "=== Starting Node1 on port 19001 ==="
$NODE1_BIN \
    --name "Node1" \
    --ip "127.0.0.1" \
    --port 19001 \
    --data-dir "$NODE1_DIR" \
    --address-file "$NODE1_DIR/address.json" \
    --inner-server-file "$NODE1_DIR/servers.json" \
    &
NODE1_PID=$!

sleep 2

echo ""
echo "=== Starting Node2 on port 19002 ==="
$NODE2_BIN \
    --name "Node2" \
    --ip "127.0.0.1" \
    --port 19002 \
    --data-dir "$NODE2_DIR" \
    --address-file "$NODE2_DIR/address.json" \
    --inner-server-file "$NODE2_DIR/servers.json" \
    &
NODE2_PID=$!

sleep 2

echo ""
echo "=== Node1 address ==="
cat "$NODE1_DIR/address.json"

echo ""
echo "=== Node2 address ==="
cat "$NODE2_DIR/address.json"

echo ""
echo "=== Sending commands via Node1 CLI ==="

# Give Node2 address to Node1
echo "connect 127.0.0.1 19002" | timeout 5s nc -w 3 127.0.0.1 19001 || true

sleep 1

echo ""
echo "=== Commands sent ==="

# Cleanup
echo ""
echo "=== Cleaning up ==="
kill $NODE1_PID $NODE2_PID 2>/dev/null || true
sleep 1

echo "Done!"
echo ""
echo "Summary:"
echo "- Node1: $NODE1_DIR (port 19001)"
echo "- Node2: $NODE2_DIR (port 19002)"