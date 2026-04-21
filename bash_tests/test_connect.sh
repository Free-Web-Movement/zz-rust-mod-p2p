#!/bin/bash
# Two nodes communicate via CLI

set -e
cd /home/eric/Projects/FreeWebMovement/zz-rust-mod-crypto-currency

echo "=== P2P Two-Node Communication Test ==="

# Clean up
pkill -f "zzp2p" 2>/dev/null || true
sleep 1

rm -rf /tmp/n1 /tmp/n2
mkdir -p /tmp/n1 /tmp/n2

# Build
cargo build --release -p zz-p2p 2>/dev/null

BIN="./target/release/zzp2p"

echo ""
echo "=== Starting Node1 (port 19001) ==="
mkfifo /tmp/n1_cmd
$BIN \
    --name "Node1" \
    --ip "127.0.0.1" \
    --port 19001 \
    --data-dir /tmp/n1 \
    --address-file /tmp/n1/addr.json \
    < /tmp/n1_cmd &
N1=$!

sleep 2

echo ""
echo "=== Starting Node2 (port 19002) ==="
mkfifo /tmp/n2_cmd
$BIN \
    --name "Node2" \
    --ip "127.0.0.1" \
    --port 19002 \
    --data-dir /tmp/n2 \
    --address-file /tmp/n2/addr.json \
    < /tmp/n2_cmd &
N2=$!

sleep 3

echo ""
echo "=== Node1 connects to Node2 ==="
echo "connect 127.0.0.1 19002" > /tmp/n1_cmd
sleep 2

echo ""
echo "=== Node2 sends message to Node1 ==="
echo "send 127.0.0.1 HelloFromNode2" > /tmp/n2_cmd
sleep 2

echo ""
echo "=== Node1 servers ==="
cat /tmp/n1/servers.json 2>/dev/null || echo "(empty)"

echo ""
echo "=== Node2 servers ==="
cat /tmp/n2/servers.json 2>/dev/null || echo "(empty)"

echo ""
echo "=== Cleanup ==="
echo "exit" > /tmp/n1_cmd 2>/dev/null || true
echo "exit" > /tmp/n2_cmd 2>/dev/null || true
sleep 1
kill $N1 $N2 2>/dev/null || true

echo ""
echo "=== Done! ==="