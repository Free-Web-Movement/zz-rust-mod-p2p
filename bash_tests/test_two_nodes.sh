#!/bin/bash
# Simple test: Two nodes, different directories, different ports

set -e

cd /home/eric/Projects/FreeWebMovement/zz-rust-mod-crypto-currency/modules/zz-rust-mod-p2p

echo "=== P2P Network Test ==="
echo ""

# Clean up old processes
pkill -f "zzp2p" 2>/dev/null || true
sleep 1

# Build
echo "Building..."
cargo build --release 2>&1 | tail -2

# Create directories
rm -rf /tmp/zz_n1 /tmp/zz_n2
mkdir -p /tmp/zz_n1 /tmp/zz_n2

echo ""
echo "=== Starting Node1 (port 19001, dir: /tmp/zz_n1) ==="
timeout 5s cargo run --release --quiet -- \
    --name "Node1" \
    --ip "127.0.0.1" \
    --port 19001 \
    --data-dir /tmp/zz_n1 \
    --address-file /tmp/zz_n1/addr.json \
    2>&1 &
N1=$!

sleep 2

echo ""
echo "=== Starting Node2 (port 19002, dir: /tmp/zz_n2) ==="
timeout 5s cargo run --release --quiet -- \
    --name "Node2" \
    --ip "127.0.0.1" \
    --port 19002 \
    --data-dir /tmp/zz_n2 \
    --address-file /tmp/zz_n2/addr.json \
    2>&1 &
N2=$!

sleep 3

echo ""
echo "=== Node1 directory contents ==="
ls -la /tmp/zz_n1/
echo ""
echo "=== Node2 directory contents ==="
ls -la /tmp/zz_n2/

echo ""
echo "=== Node1 address ==="
cat /tmp/zz_n1/addr.json 2>/dev/null | head -3 || echo "File not found"

echo ""
echo "=== Node2 address ==="
cat /tmp/zz_n2/addr.json 2>/dev/null | head -3 || echo "File not found"

# Cleanup
kill $N1 $N2 2>/dev/null || true

echo ""
echo "=== Test Complete ==="