#!/bin/bash
set -e

echo "=== Haunt Deployment Script ==="
echo "$(date)"

# Source cargo environment
source ~/.cargo/env

# Navigate to project directory
cd /root/Haunt

# Pull latest code
echo "Pulling latest code..."
git pull origin main || echo "Git pull failed, continuing with existing code"

# Build release
echo "Building release..."
cargo build --release

# Stop existing process if running
echo "Stopping existing Haunt process..."
pkill -f "target/release/haunt" || echo "No existing process found"
sleep 2

# Start the server in background
echo "Starting Haunt server..."
nohup ./target/release/haunt > /var/log/haunt.log 2>&1 &

# Wait and verify
sleep 3
if pgrep -f "target/release/haunt" > /dev/null; then
    echo "Haunt started successfully!"
    echo "PID: $(pgrep -f 'target/release/haunt')"
else
    echo "Failed to start Haunt. Check /var/log/haunt.log"
    tail -20 /var/log/haunt.log
    exit 1
fi

echo "=== Deployment Complete ==="
