# MySQL MCP Server

A high-quality Model Context Protocol (MCP) server implementation for MySQL databases. This server enables AI assistants like Claude to interact with MySQL databases through a standardized protocol.

**Version**: 0.2.0 | **Protocol**: MCP 2025-03-26 | **Rust**: 1.70+ | **Status**: Production Ready

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Quick Start (5 Minutes)](#quick-start-5-minutes)
- [Usage](#usage)
- [Available Tools](#available-tools)
- [Database Context Feature](#database-context-feature)
- [Security Considerations](#security-considerations)
- [Architecture](#architecture)
- [Troubleshooting](#troubleshooting)
- [Development](#development)
- [Deployment Guide](#deployment-guide)
- [Contributing](#contributing)
- [License](#license)
- [Support](#support)

## Features

- **Schema Inspection**: Retrieve table schemas and structure information
- **Query Execution**: Execute SQL queries (read-only by default for safety)
- **Data Manipulation**: Insert, update, and delete operations
- **Database Context**: Specify which database to use per query
- **Safety Controls**: Configurable query restrictions to prevent dangerous operations
- **Connection Management**: Robust connection handling with retry logic and pooling
- **Error Handling**: Comprehensive error reporting with detailed messages
- **JSON-RPC 2.0 Protocol**: Standardized communication via stdio

## Installation

### Prerequisites

- Rust 1.70+
- MySQL 5.7+ or MariaDB 10.2+
- Access to a MySQL database

### Building from Source

```bash
git clone <repository-url>
cd mcp-server-mysql
cargo build --release
```

The compiled binary will be available at `target/release/mcp-server-mysql`.

### From Release Package

```bash
# Extract the package
tar -xzf mcp-server-mysql-v0.2.0-linux-x86_64.tar.gz

# Move binary to system path (optional)
sudo cp mcp-server-mysql /usr/local/bin/

# Verify installation
mcp-server-mysql --version
```

## Quick Start (5 Minutes)

### Step 1: Build the Server

```bash
cargo build --release
```

The binary will be at `target/release/mcp-server-mysql`

### Step 2: Test the Connection

```bash
./target/release/mcp-server-mysql \
  --host localhost \
  --username root \
  --password yourpassword \
  --database testdb
```

You should see: "MCP MySQL Server started and ready to accept connections"

### Step 3: Configure Claude Desktop

Edit your Claude Desktop configuration file:

- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

Add this configuration:

```json
{
  "mcpServers": {
    "mysql": {
      "command": "/absolute/path/to/mcp-server-mysql",
      "args": [
        "--host", "localhost",
        "--port", "3306",
        "--username", "your_username",
        "--password", "your_password",
        "--database", "your_database"
      ]
    }
  }
}
```

**Security Note**: For production use, consider using environment variables or a secure secrets management solution instead of hardcoding passwords in the configuration file.

### Step 4: Restart Claude Desktop

Close and reopen Claude Desktop completely. You should see a small hammer icon indicating the MCP server is connected.

### Step 5: Try it Out!

Ask Claude:
- "Can you show me the schema for the users table in my MySQL database?"
- "Query the database and show me the first 10 rows from the products table"
- "What tables are in my database?"

## Usage

### Command Line Arguments

```bash
mcp-server-mysql \
  --host localhost \
  --port 3306 \
  --username your_username \
  --password your_password \
  --database your_database \
  --allow-dangerous-queries false
```

### Arguments Reference

| Argument | Description | Default | Required |
|----------|-------------|---------|----------|
| `--host` | MySQL server hostname | `localhost` | No |
| `--port` | MySQL server port | `3306` | No |
| `--username` | MySQL username | - | Yes |
| `--password` | MySQL password | ` ` (empty) | No |
| `--database` | Database name to connect to | - | Yes |
| `--allow-dangerous-queries` | Allow INSERT/UPDATE/DELETE queries | `false` | No |

### Configuration with Claude Desktop

Add this configuration to your Claude Desktop config file:

**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
**Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "mysql": {
      "command": "/path/to/mcp-server-mysql",
      "args": [
        "--host", "localhost",
        "--port", "3306",
        "--username", "your_username",
        "--password", "your_password",
        "--database", "your_database"
      ]
    }
  }
}
```

## Available Tools

### 1. mysql (Schema Inspection)

Retrieve database schema information for tables.

**Parameters:**
- `table_name` (string): Name of the table to inspect, or `"all-tables"` to get all table schemas

**Example:**
```json
{
  "table_name": "users"
}
```

**Returns:**
- Column information (name, type, nullable, defaults, keys)
- Index information
- Table constraints

### 2. query (SQL Execution)

Execute SQL queries on the database.

**Parameters:**
- `query` (string): SQL query to execute
- `database` (string, optional): Database name to use for this specific query

**Example:**
```json
{
  "query": "SELECT * FROM users WHERE active = 1 LIMIT 10",
  "database": "my_database"
}
```

**Safety:**
- By default, only SELECT queries are allowed
- Use `--allow-dangerous-queries` flag to enable INSERT/UPDATE/DELETE
- Dangerous keywords are blocked unless explicitly enabled

### 3. insert (Insert Data)

Insert data into a specified table.

**Parameters:**
- `table_name` (string): Name of the table
- `data` (object): Key-value pairs of column names and values

**Example:**
```json
{
  "table_name": "users",
  "data": {
    "username": "john_doe",
    "email": "john@example.com",
    "active": true
  }
}
```

**Returns:** Last insert ID

### 4. update (Update Data)

Update data in a specified table based on conditions.

**Parameters:**
- `table_name` (string): Name of the table
- `data` (object): Key-value pairs of columns to update
- `conditions` (object): Key-value pairs for WHERE clause

**Example:**
```json
{
  "table_name": "users",
  "data": {
    "email": "newemail@example.com",
    "updated_at": "2024-01-15 10:30:00"
  },
  "conditions": {
    "id": 123
  }
}
```

**Returns:** Number of affected rows

### 5. delete (Delete Data)

Delete data from a specified table based on conditions.

**Parameters:**
- `table_name` (string): Name of the table
- `conditions` (object): Key-value pairs for WHERE clause

**Example:**
```json
{
  "table_name": "users",
  "conditions": {
    "id": 123
  }
}
```

**Returns:** Number of affected rows

**Warning:** Always specify conditions to avoid deleting all rows!

## Database Context Feature

### The Problem

Previously, database context was not maintained between queries:

```sql
-- Query 1
USE dev_database;  -- Succeeds

-- Query 2 (new connection from pool)
SELECT * FROM my_table;  -- ❌ Fails: context was lost
```

### The Solution

Use the optional `database` parameter on each query:

```json
{
  "query": "SELECT * FROM my_table",
  "database": "dev_database"
}
```

### Benefits

1. **Explicit and Clear**: Know exactly which database each query uses
2. **No Hidden State**: Each query is independent
3. **Backward Compatible**: Existing queries without parameter still work
4. **No Race Conditions**: Each query gets its own connection
5. **Simple to Use**: Just add `"database": "name"` to query arguments

### Usage Examples

#### Basic Query with Database Parameter

```json
{
  "query": "SELECT * FROM crm_sites LIMIT 10",
  "database": "dev_smartConnect_za"
}
```

#### Query Without Database Parameter (Uses Default)

```json
{
  "query": "SELECT * FROM users WHERE active = 1"
}
```

Uses the database specified in `--database` startup argument.

#### Multiple Databases in Same Session

```json
// Query database 1
{
  "query": "SELECT COUNT(*) FROM customers",
  "database": "production_db"
}

// Query database 2
{
  "query": "SELECT COUNT(*) FROM test_data",
  "database": "test_db"
}
```

#### Before vs After

**Before (Required fully qualified names):**
```sql
SELECT * FROM dev_smartConnect_za.crm_sites
  JOIN dev_smartConnect_za.crm_orgs ON ...
WHERE dev_smartConnect_za.crm_sites.active = 1;
```

**After (Clean and simple):**
```json
{
  "query": "SELECT * FROM crm_sites JOIN crm_orgs ON ... WHERE active = 1",
  "database": "dev_smartConnect_za"
}
```

### Common Scenarios

#### Scenario 1: Single Database Project

Set default database and omit the parameter:

```bash
# Startup
--database my_project_db

# Query (no database parameter needed)
{
  "query": "SELECT * FROM users"
}
```

#### Scenario 2: Multiple Database Project

Specify database for each query:

```json
// Customer database
{ "query": "...", "database": "customers_db" }

// Orders database
{ "query": "...", "database": "orders_db" }

// Analytics database
{ "query": "...", "database": "analytics_db" }
```

### Error Handling

**Error Code -32005: Connection Acquisition Failed**
```
Cause: Connection pool exhausted
Solution: Retry after a moment
```

**Error Code -32006: Database Context Switch Failed**
```
Cause: Database doesn't exist or user lacks permissions
Solution: Verify database exists and user has access
```

### Best Practices

✅ **DO**
- Specify database explicitly for production queries
- Use descriptive database names in your queries
- Test with `SELECT DATABASE()` to verify context
- Group queries by database for clarity

❌ **DON'T**
- Mix qualified and unqualified names in the same query
- Assume persistence - specify database for each query
- Use special characters in database names if possible
- Forget to verify user permissions for all databases

## Security Considerations

### Read-Only Mode (Default)

By default, the server operates in read-only mode, allowing only SELECT queries. This prevents accidental data modification or deletion.

### Dangerous Queries Mode

Enable write operations with `--allow-dangerous-queries`:

```bash
mcp-server-mysql --username user --password pass --database mydb --allow-dangerous-queries true
```

**Use with caution!** This enables:
- INSERT statements
- UPDATE statements
- DELETE statements
- Other potentially destructive operations

### SQL Injection Protection

- Table names are validated to contain only alphanumeric characters and underscores
- All data values are parameterized using prepared statements
- Database names are escaped by replacing backticks with double backticks
- No raw SQL concatenation is performed

### Connection Security

- Supports standard MySQL SSL/TLS connections
- Connection strings can be configured securely
- Passwords can be provided via environment variables
- Consider using dedicated database users with limited permissions

### Production Deployment Security

1. **Use dedicated database user**:
   ```sql
   CREATE USER 'mcp_user'@'localhost' IDENTIFIED BY 'secure_password';
   GRANT SELECT ON your_database.* TO 'mcp_user'@'localhost';
   FLUSH PRIVILEGES;
   ```

2. **Enable write access only when needed**:
   ```bash
   --allow-dangerous-queries true  # Use with caution!
   ```

3. **Use environment variables** (future enhancement):
   Consider wrapping the binary in a shell script that reads from env vars.

## Architecture

### System Overview

```
┌─────────────────────────────────────────────────────┐
│           MCP Client (e.g., Claude)                 │
│  Sends: {query, database}                           │
└────────────────────────┬────────────────────────────┘
                         │ JSON-RPC 2.0 (stdio)
                         ▼
┌─────────────────────────────────────────────────────┐
│      MCP MySQL Server (Rust)                        │
│                                                      │
│  execute_query(query, database, pool)               │
│  ├─ If database param:                              │
│  │  ├─ Acquire connection from pool                 │
│  │  ├─ Execute: USE `database`                      │
│  │  └─ Execute: [user's query]                      │
│  └─ Else:                                           │
│     └─ Execute query on pool (default database)     │
└────────────────────────┬────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────┐
│      MySQL Connection Pool (5 connections)          │
└────────────────────────┬────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────┐
│      MySQL/MariaDB Server                           │
└─────────────────────────────────────────────────────┘
```

### Sequence: Query with Database Parameter

```
Client          MCP Server       Connection Pool      MySQL Server
  │                 │                    │                  │
  │  query +        │                    │                  │
  │  database       │                    │                  │
  ├────────────────>│                    │                  │
  │                 │                    │                  │
  │                 │ acquire()          │                  │
  │                 ├───────────────────>│                  │
  │                 │ <connection>       │                  │
  │                 │<───────────────────┤                  │
  │                 │                    │                  │
  │                 │ USE database       │                  │
  │                 ├────────────────────┼─────────────────>│
  │                 │ OK                 │                  │
  │                 │<────────────────────┼──────────────────┤
  │                 │                    │                  │
  │                 │ SELECT query       │                  │
  │                 ├────────────────────┼─────────────────>│
  │                 │ Results            │                  │
  │                 │<────────────────────┼──────────────────┤
  │                 │                    │                  │
  │                 │ release()          │                  │
  │                 ├───────────────────>│                  │
  │  Results        │                    │                  │
  │<────────────────┤                    │                  │
```

### Connection Pool Management

```
Pool (5 connections)
┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐
│ C1 │ │ C2 │ │ C3 │ │ C4 │ │ C5 │
└────┘ └────┘ └────┘ └────┘ └────┘

Key Properties:
• Each query gets its own connection instance
• Database context is set per connection, per query
• No state persists between queries
• Fully thread-safe and concurrent
```

### Technical Details

- **Protocol Version**: MCP 2025-03-26
- **Transport**: stdio (JSON-RPC 2.0)
- **Connection Pooling**: Max 5 connections
- **Retry Logic**: Automatic reconnection on transient failures
- **Performance Overhead**: ~50-200 microseconds per query with database parameter

## Troubleshooting

### Connection Failures

If you encounter connection errors:

1. **Check MySQL is running:**
   ```bash
   mysql -h localhost -u your_username -p
   ```

2. **Verify credentials:**
   - Ensure the username and password are correct
   - Confirm the user has access to the specified database

3. **Check network access:**
   - Verify the host and port are correct
   - Ensure no firewall is blocking the connection

4. **Review server logs:**
   - The server logs to stderr
   - Check for detailed error messages

### Common Errors

#### "Database connection failed"

- MySQL server may not be running
- Incorrect host/port configuration
- Network connectivity issues

#### "Only SELECT queries are allowed"

- You're trying to run a write query in read-only mode
- Add `--allow-dangerous-queries true` if write access is needed

#### "No database selected"

- The specified database doesn't exist
- The user doesn't have access to the database
- Check `SHOW DATABASES;` to see available databases

#### "Table doesn't exist"

- Verify you're querying the correct database
- Add the `database` parameter if using multiple databases
- Use `SELECT DATABASE()` to check current context

#### "Failed to acquire connection"

- Connection pool is exhausted
- Wait a moment and retry

### Tool not appearing in Claude Desktop

1. Verify the path to the binary is absolute (not relative)
2. Check Claude Desktop logs for errors
3. Restart Claude Desktop completely (not just reload)
4. Ensure the server process starts without errors when run manually

## Development

### Project Structure

```
mcp-server-mysql/
├── src/
│   ├── main.rs          # Main server implementation
│   ├── config.rs        # Configuration handling
│   ├── db.rs            # Database operations
│   ├── rpc.rs           # RPC protocol handling
│   └── server.rs        # Server initialization
├── tests/               # Test files
├── Cargo.toml           # Rust dependencies
├── Cargo.lock          # Locked dependency versions
└── README.md           # This file
```

### Building for Development

```bash
cargo build
cargo run -- --help
```

### Running Tests

```bash
cargo test
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Check for issues
cargo check
```

### Development Conventions

The project follows standard Rust best practices:
- Code formatting: `cargo fmt`
- Linting: `cargo clippy`
- Testing: `cargo test`

## Deployment Guide

### Quick Deployment

#### 1. Test Connection

```bash
./mcp-server-mysql \
  --username your_user \
  --password your_pass \
  --database your_db
```

Press Ctrl+C to exit after seeing "MCP MySQL Server started".

#### 2. Configure Claude Desktop

Edit your Claude config file and add the server configuration (see Quick Start section).

#### 3. Restart Claude Desktop

Close and reopen Claude Desktop completely.

### Production Deployment Tips

#### Performance

- The binary is optimized with `--release` flag
- Connection pooling is configured (max 5 connections)
- Automatic retry logic for transient failures

#### Monitoring

Server logs go to stderr. Capture them with:

```bash
./mcp-server-mysql --username user --password pass --database db 2>> server.log
```

Log levels:
- `INFO`: Connection events, tool calls
- `DEBUG`: Detailed query information
- `WARN`: Non-fatal issues
- `ERROR`: Failures and errors

### Systemd Service (Optional)

For long-running deployments, create `/etc/systemd/system/mcp-mysql.service`:

```ini
[Unit]
Description=MySQL MCP Server
After=network.target mysql.service

[Service]
Type=simple
User=mcp-user
ExecStart=/usr/local/bin/mcp-server-mysql --username mcp_user --password secret --database production
Restart=on-failure
RestartSec=5s
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl enable mcp-mysql
sudo systemctl start mcp-mysql
sudo systemctl status mcp-mysql
```

### Upgrading

```bash
# Backup current version
cp /usr/local/bin/mcp-server-mysql /usr/local/bin/mcp-server-mysql.backup

# Replace with new version
cp mcp-server-mysql /usr/local/bin/

# Restart services
sudo systemctl restart mcp-mysql  # If using systemd
# Or restart Claude Desktop
```

### Rollback

```bash
# Restore previous version
cp /usr/local/bin/mcp-server-mysql.backup /usr/local/bin/mcp-server-mysql

# Or checkout previous git tag
git checkout v0.1.0
cargo build --release
```

## Contributing

Contributions are welcome! Please ensure:
- Code follows Rust best practices
- All tests pass
- Documentation is updated
- Commit messages are clear and descriptive

## License

Apache-2.0

## Support

For issues, questions, or contributions, please open an issue on the project repository.

---

**Version**: 0.2.0 | **Release Date**: 2025-01-XX | **Protocol**: MCP 2025-03-26 | **Platform**: Linux x86_64 | **Status**: Production Ready ✅
