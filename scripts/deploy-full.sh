#!/bin/bash
set -e

echo "=== Full Deployment Script ==="
echo "$(date)"

SERVER="64.176.44.233"
USER="root"
# Note: password should be passed via SSH keys or sshpass

# Pull latest code on server
echo "Pulling latest code..."
ssh $USER@$SERVER 'cd /root/Haunt && git pull origin main'
ssh $USER@$SERVER 'cd /root/Wraith && git pull origin main'
ssh $USER@$SERVER 'cd /root/ghost && git pull origin main'

# Build Haunt (backend)
echo "Building Haunt..."
ssh $USER@$SERVER 'source ~/.cargo/env && cd /root/Haunt && cargo build --release'

# Restart Haunt
echo "Restarting Haunt..."
ssh $USER@$SERVER 'pkill -f "target/release/haunt" || true'
sleep 2
ssh $USER@$SERVER 'cd /root/Haunt && nohup ./target/release/haunt > /var/log/haunt.log 2>&1 &'
sleep 3

# Verify Haunt is running
ssh $USER@$SERVER 'pgrep -f "target/release/haunt" && curl -s http://localhost:3001/api/health'

# Build Wraith (frontend)
echo "Building Wraith..."
ssh $USER@$SERVER 'cd /root/Wraith && npm run build'

# Deploy to nginx
echo "Deploying frontend..."
ssh $USER@$SERVER 'rm -rf /var/www/haunt/* && cp -r /root/Wraith/dist/* /var/www/haunt/ && chown -R www-data:www-data /var/www/haunt'

# Clear nginx cache
ssh $USER@$SERVER 'systemctl reload nginx'

echo "=== Deployment Complete ==="
echo "Backend: http://$SERVER:3001/api/health"
echo "Frontend: http://$SERVER/"
