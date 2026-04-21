#!/bin/bash
# Interactive P2P Network Test

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "=== P2P Network Interactive Demo ==="

# Clean up
pkill -f "zzp2p" 2>/dev/null || true
sleep 1

# Create directories
NODE1_DIR="/tmp/p2p_node1"
NODE2_DIR="/tmp/p2p_node2"

rm -rf "$NODE1_DIR" "$NODE2_DIR"
mkdir -p "$NODE1_DIR" "$NODE2_DIR"

echo "Node1: $NODE1_DIR (port 19001)"
echo "Node2: $NODE2_DIR (port 19002)"
echo ""

# Build
echo "Building..."
cargo build --release 2>&1 | tail -1

# Start Node1
echo "=== Starting Node1 ==="
cargo run --release --quiet -- \
    --name "Node1" \
    --ip "127.0.0.1" \
    --port 19001 \
    --data-dir "$NODE1_DIR" \
    --address-file "$NODE1_DIR/address.json" \
    2>&1 | head -5 &
NODE1_PID=$!

# Start Node2
echo "=== Starting Node2 ==="
cargo run --release --quiet -- \
    --name "Node2" \
    --ip "127.0.0.1" \
    --port 19002 \
    --data-dir "$NODE2_DIR" \
    --address-file "$NODE2_DIR/address.json" \
    2>&1 | head -5 &
NODE2_PID=$!

sleep 3

# Show addresses
echo ""
echo "=== Node1 Address ==="
cat "$NODE1_DIR/address.json" 2>/dev/null | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('address','N/A')[:50])" 2>/dev/null || echo "(checking...)"

echo "=== Node2 Address ==="
cat "$NODE2_DIR/address.json" 2>/dev/null | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('address','N/A')[:50])" 2>/dev/null || echo "(checking...)"

echo ""
echo "=== Connected nodes after startup ==="
cat "$NODE1_DIR/servers.json" 2>/dev/null || echo "No servers yet"
cat "$NODE2_DIR/servers.json" 2>/dev/null || echo "No servers yet"

echo ""
echo "=== Test Complete ==="

# Cleanup
kill $NODE1_PID $NODE2_PID 2>/dev/null || true

echo "Done!"