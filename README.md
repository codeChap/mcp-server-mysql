# MySQL MCP Server

A high-quality Model Context Protocol (MCP) server implementation for MySQL databases. This server enables AI assistants like Claude to interact with MySQL databases through a standardized protocol.

## Features

- **Schema Inspection**: Retrieve table schemas and structure information
- **Query Execution**: Execute SQL queries (read-only by default for safety)
- **Data Manipulation**: Insert, update, and delete operations
- **Safety Controls**: Configurable query restrictions to prevent dangerous operations
- **Connection Management**: Robust connection handling with retry logic
- **Error Handling**: Comprehensive error reporting with detailed messages

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

### Arguments

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

**Security Note**: For production use, consider using environment variables or a secure secrets management solution instead of hardcoding passwords in the configuration file.

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
  "query": "SELECT * FROM users WHERE active = 1 LIMIT 10"
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
- No raw SQL concatenation is performed

### Connection Security

- Supports standard MySQL SSL/TLS connections
- Connection strings can be configured securely
- Passwords can be provided via environment variables

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

## Development

### Project Structure

```
mcp-server-mysql/
├── src/
│   └── main.rs          # Main server implementation
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
```

## Protocol Information

This server implements the Model Context Protocol (MCP) specification:
- **Protocol Version:** 2025-03-26
- **Transport:** stdio (standard input/output)
- **Message Format:** JSON-RPC 2.0

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
