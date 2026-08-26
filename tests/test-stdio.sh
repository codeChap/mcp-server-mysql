#!/bin/bash

# Test script for the stdio-based MCP server
# Run from the project root or from tests/

set -euo pipefail

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

# Run the server and pipe commands to it.
# --quiet keeps cargo compile chatter off stdout so we can parse JSON-RPC lines.
OUTPUT=$(cargo run --quiet --bin mcp-server-mysql -- $DB_ARGS <<EOF
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"test-client","version":"1.0.0"}}}
{"jsonrpc":"2.0","method":"initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"mysql","arguments":{"table_name":"all-tables"}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"mysql","arguments":{"table_name":"newsletter","database":"hs2026"}}}
EOF
)

# all-tables payload is large; write to a temp file so python is not given it via argv.
TMP=$(mktemp)
trap 'rm -f "$TMP"' EXIT
printf '%s\n' "$OUTPUT" > "$TMP"

python3 - "$TMP" <<'PY'
import json, sys

with open(sys.argv[1], encoding="utf-8") as f:
    raw = f.read()

messages = {}
for line in raw.splitlines():
    line = line.strip()
    if not line.startswith("{"):
        continue
    try:
        msg = json.loads(line)
    except json.JSONDecodeError:
        continue
    if "id" in msg:
        messages[msg["id"]] = msg

def content_text(msg):
    result = msg.get("result") or {}
    content = result.get("content") or []
    if not content:
        return ""
    return content[0].get("text") or ""

# all-tables smoke
all_tables = messages.get(3)
if not all_tables:
    print("FAIL: missing all-tables response (id=3)", file=sys.stderr)
    sys.exit(1)
if "error" in all_tables:
    print(f"FAIL: all-tables error: {all_tables['error']}", file=sys.stderr)
    sys.exit(1)
all_text = content_text(all_tables)
if "Retrieved schemas" not in all_text:
    print("FAIL: all-tables content missing description", file=sys.stderr)
    sys.exit(1)
if '"columns"' not in all_text and "'columns'" not in all_text:
    print("FAIL: all-tables content has no column data (schema must be in content[0].text)", file=sys.stderr)
    sys.exit(1)
print("OK: all-tables includes schema columns in content text")

# single-table describe — hosts only see content[].text
describe = messages.get(4)
if not describe:
    print("FAIL: missing newsletter describe response (id=4)", file=sys.stderr)
    sys.exit(1)
if "error" in describe:
    print(f"FAIL: newsletter describe error: {describe['error']}", file=sys.stderr)
    sys.exit(1)
text = content_text(describe)
if "Retrieved schema for table 'newsletter'" not in text:
    print(f"FAIL: describe missing description line; got: {text[:200]!r}", file=sys.stderr)
    sys.exit(1)
for col in ("email", "unsubscribed"):
    if col not in text:
        print(f"FAIL: describe content text missing column '{col}'", file=sys.stderr)
        print(text[:2000], file=sys.stderr)
        sys.exit(1)
print("OK: newsletter describe includes column names in content text")
print("--- newsletter content[0].text (first 800 chars) ---")
print(text[:800])
PY
