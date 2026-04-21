#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "=== P2P Network Demo ==="
echo ""

# Clean up
echo "Cleaning up old processes..."
pkill -f "zzp2p" 2>/dev/null || true
sleep 1

# Create directories
NODE1_DIR="/tmp/p2p_node1"
NODE2_DIR="/tmp/p2p_node2"

rm -rf "$NODE1_DIR" "$NODE2_DIR"
mkdir -p "$NODE1_DIR" "$NODE2_DIR"

echo "Node1 dir: $NODE1_DIR"
echo "Node2 dir: $NODE2_DIR"

# Build
echo "Building..."
cargo build --release 2>&1 | tail -3

# Start Node1 in background
echo ""
echo "=== Starting Node1 (port 19001) ==="
NODE1_FIFO="$NODE1_DIR/commands"
mkfifo "$NODE1_FIFO"

cargo run --release -- \
    --name "Node1" \
    --ip "127.0.0.1" \
    --port 19001 \
    --data-dir "$NODE1_DIR" \
    < "$NODE1_FIFO" &
NODE1_PID=$!

# Start Node2 in background  
echo "=== Starting Node2 (port 19002) ==="
NODE2_FIFO="$NODE2_DIR/commands"
mkfifo "$NODE2_FIFO"

cargo run --release -- \
    --name "Node2" \
    --ip "127.0.0.1" \
    --port 19002 \
    --data-dir "$NODE2_DIR" \
    < "$NODE2_FIFO" &
NODE2_PID=$!

sleep 3

echo ""
echo "=== Node1 Address ==="
cat "$NODE1_DIR/address.json" | head -c 200
echo ""

echo ""
echo "=== Node2 Address ==="
cat "$NODE2_DIR/address.json" | head -c 200
echo ""

# Send connect command to Node2
echo ""
echo "=== Node2 connecting to Node1 ==="
echo "connect 127.0.0.1 19001" > "$NODE2_FIFO"
sleep 2

# Send message from Node2 to Node1
echo ""
echo "=== Node2 sending message to Node1 ==="
echo "send Hello from Node2" > "$NODE2_FIFO"
sleep 2

# Check Node1's server list
echo ""
echo "=== Node1 connected servers ==="
cat "$NODE1_DIR/servers.json" 2>/dev/null || echo "(no servers file)"

# Cleanup
echo ""
echo "=== Cleanup ==="
echo "quit" > "$NODE1_FIFO" 2>/dev/null || true
echo "quit" > "$NODE2_FIFO" 2>/dev/null || true

sleep 1
kill $NODE1_PID $NODE2_PID 2>/dev/null || true
rm -f "$NODE1_FIFO" "$NODE2_FIFO"

echo "Done!"