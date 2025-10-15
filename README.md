# Zed MySQL Context Server

A Model Context Server for MySQL that integrates with Zed AI assistant.

## Prerequisites

- Rust (latest stable version)
- MySQL/MariaDB database
- Zed

## Installation

1. Clone and build:
```bash
git clone <repository-url>
cd mcp-server-mysql
cargo build --release
```

2. Configure Zed (see Configuration section)

## Configuration

Click on the "Toggle Agent Menu" -> "Add custom Server"

```json
{
  /// The name of your MCP server
  "mysql-mcp-server": {
    /// The command which runs the MCP server
    "command": "/path/to/mcp-server-mysql/target/release/mcp-server-mysql",
    /// The arguments to pass to the MCP server
    "args": [
        "--username", "your_username",
        "--password", "your_password",
        "--database", "your_database",
        "--host", "localhost",
        "--port", "3306"
    ],
    /// The environment variables to set
    "env": {}
  }
}
```

To allow dangerous queries (INSERT, UPDATE, DELETE, etc.), add the flag:

```json
{
  "mysql-mcp-server": {
    "command": "/path/to/mcp-server-mysql/target/release/mcp-server-mysql",
    "args": [
        "--username", "your_username",
        "--password", "your_password",
        "--database", "your_database",
        "--host", "localhost",
        "--port", "3306",
        "--allow-dangerous-queries"
    ],
    "env": {}
  }
}
```

### Options

- `--host <HOST>`: MySQL host (default: localhost)
- `--port <PORT>`: MySQL port (default: 3306)
- `--username <USERNAME>`: MySQL username (required)
- `--password <PASSWORD>`: MySQL password (default: empty)
- `--database <DATABASE>`: MySQL database name (required)
- `--allow-dangerous-queries`: Allow dangerous SQL keywords in queries (INSERT, UPDATE, DELETE, etc.)

### Logging

The server uses standard Rust logging. Control log levels with the `RUST_LOG` environment variable:

- `RUST_LOG=error` - Only show errors
- `RUST_LOG=warn` - Show warnings and errors  
- `RUST_LOG=info` - Show info, warnings and errors (recommended for production)
- `RUST_LOG=debug` - Show all messages including detailed debug info

### Testing

Test manually with your database:

```bash
# Test initialization (with info logging)
RUST_LOG=info echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | cargo run --bin mcp-server-mysql -- --username admin --database mydb

# Test schema retrieval (quiet mode)
echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"mysql","arguments":{"table_name":"users"}}}' | cargo run --bin mcp-server-mysql -- --username admin --database mydb

# Test with debug output (for troubleshooting)
RUST_LOG=debug echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | cargo run --bin mcp-server-mysql -- --username admin --database mydb
```

Or use the test script:

```bash
./test-stdio.sh
```

### Building

```bash
# Development
cargo build

# Production
cargo build --release
```

## Available Tools

The server provides the following tools:

- **mysql**: Retrieve MySQL database schema information for tables
- **query**: Execute SQL queries (SELECT only by default, all queries with `--allow-dangerous-queries`)
- **insert**: Insert data into a specified table
- **update**: Update data in a specified table based on conditions
- **delete**: Delete data from a specified table based on conditions

### Query Tool

The `query` tool executes SQL queries on the database.

#### Parameters

- `query` (required): The SQL query to execute
- `database` (optional): Database name to use for this query. If specified, the query will be executed in the context of this database.

#### Database Context

Each query can specify which database to use via the optional `database` parameter. This is useful when working with multiple databases or when you need to query a database different from the default.

**Example with database parameter:**
```json
{
  "name": "query",
  "arguments": {
    "query": "SELECT * FROM crm_sites LIMIT 10",
    "database": "dev_smartConnect_za"
  }
}
```

**Example without database parameter (uses default):**
```json
{
  "name": "query",
  "arguments": {
    "query": "SELECT * FROM users WHERE active = 1 LIMIT 10"
  }
}
```

**Multiple databases in one session:**
```json
// Query from production database
{
  "query": "SELECT COUNT(*) FROM customers",
  "database": "production_db"
}

// Query from test database
{
  "query": "SELECT COUNT(*) FROM test_data",
  "database": "test_db"
}
```

#### Query Safety

By default, only SELECT queries are allowed:

```sql
SELECT COUNT(*) as count FROM accounts WHERE primary_category = 'Medical';
SELECT * FROM users WHERE active = 1 LIMIT 10;
```

With `--allow-dangerous-queries` flag, you can execute any SQL:

```sql
CREATE TABLE new_table (id INT PRIMARY KEY, name VARCHAR(255));
ALTER TABLE users ADD COLUMN last_login TIMESTAMP;
DROP TABLE old_data;
TRUNCATE TABLE logs;
```

#### Notes

- When no `database` parameter is provided, the query uses the default database specified in `--database`
- The `database` parameter allows you to query any database you have permissions for
- Each query with a `database` parameter acquires its own connection to ensure proper database context
- Database names with special characters are automatically escaped

## License

Apache-2.0
