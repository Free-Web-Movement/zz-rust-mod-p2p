#!/bin/bash

cd /home/eric/Projects/FreeWebMovement/zz-rust-mod-crypto-currency/modules/zz-rust-mod-p2p

echo "=== P2P Network Interactive Demo ==="

# Clean up
pkill -f "zzp2p" 2>/dev/null
sleep 1

# Build
echo "Building..."
cargo build --release 2>&1 | grep -E "Finished|error" || true
echo ""

# Create dirs
rm -rf /tmp/p2p_n1 /tmp/p2p_n2
mkdir -p /tmp/p2p_n1 /tmp/p2p_n2

echo "=== Node1 (port 19001) ==="
cargo run --release --quiet -- \
    --name "Node1" \
    --ip "127.0.0.1" \
    --port 19001 \
    --data-dir /tmp/p2p_n1 \
    --address-file /tmp/p2p_n1/addr.json \
    2>&1 &
N1=$!

echo "=== Node2 (port 19002) ==="
cargo run --release --quiet -- \
    --name "Node2" \
    --ip "127.0.0.1" \
    --port 19002 \
    --data-dir /tmp/p2p_n2 \
    --address-file /tmp/p2p_n2/addr.json \
    2>&1 &
N2=$!

sleep 3

echo "Addresses:"
echo "Node1: $(head -c 80 /tmp/p2p_n1/addr.json 2>/dev/null)"
echo "Node2: $(head -c 80 /tmp/p2p_n2/addr.json 2>/dev/null)"

echo ""
echo "Servers saved:"
cat /tmp/p2p_n1/servers.json 2>/dev/null
cat /tmp/p2p_n2/servers.json 2>/dev/null

echo ""
kill $N1 $N2 2>/dev/null
echo "Done!"