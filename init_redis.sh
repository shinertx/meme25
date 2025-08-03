#!/bin/bash

# Redis stream initialization script
echo "Starting Redis stream and consumer group initialization..."

# Connect to Redis and initialize all streams and consumer groups
redis-cli -h localhost -p 6379 <<EOF
# Create streams and consumer groups for all event types
XGROUP CREATE events:price executor_group $ MKSTREAM
XGROUP CREATE events:social executor_group $ MKSTREAM
XGROUP CREATE events:depth executor_group $ MKSTREAM
XGROUP CREATE events:bridge executor_group $ MKSTREAM
XGROUP CREATE events:funding executor_group $ MKSTREAM
XGROUP CREATE events:onchain executor_group $ MKSTREAM
XGROUP CREATE events:solprice executor_group $ MKSTREAM
XGROUP CREATE events:twitter executor_group $ MKSTREAM
XGROUP CREATE events:farcaster executor_group $ MKSTREAM
XGROUP CREATE events:whale executor_group $ MKSTREAM
XGROUP CREATE events:liquidation executor_group $ MKSTREAM

# Verify all streams and groups exist
XINFO GROUPS events:price
XINFO GROUPS events:social
XINFO GROUPS events:depth
XINFO GROUPS events:bridge
XINFO GROUPS events:funding
XINFO GROUPS events:onchain
XINFO GROUPS events:solprice
XINFO GROUPS events:twitter
XINFO GROUPS events:farcaster
XINFO GROUPS events:whale
XINFO GROUPS events:liquidation

PING
QUIT
EOF

echo "Redis initialization complete"
