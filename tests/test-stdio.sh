#!/bin/bash

# Test script for the stdio-based MCP server
# Run from the project root or from tests/

cd "$(dirname "$0")/.." || exit 1

# Database connection settings - modify these for your database
DB_HOST="localhost"
DB_PORT="3306"
DB_USER="admin"
DB_PASS=""
DB_NAME="wts"

# Build the command line arguments
if [ -n "$DB_PASS" ]; then
    DB_ARGS="--host $DB_HOST --port $DB_PORT --username $DB_USER --password $DB_PASS --database $DB_NAME"
else
    DB_ARGS="--host $DB_HOST --port $DB_PORT --username $DB_USER --database $DB_NAME"
fi

echo "Testing MCP MySQL Server with database: $DB_NAME"
echo "Connection: $DB_USER@$DB_HOST:$DB_PORT"
echo ""

# Run the server and pipe commands to it
# We use a heredoc to send multiple commands to the same process
cargo run --bin mcp-server-mysql -- $DB_ARGS <<EOF
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"test-client","version":"1.0.0"}}}
{"jsonrpc":"2.0","method":"initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"mysql","arguments":{"table_name":"all-tables"}}}
EOF