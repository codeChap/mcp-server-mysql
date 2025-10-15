# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2024-01-XX

### Added
- **Database Context Parameter**: Added optional `database` parameter to the `query` tool
  - Allows specifying which database to use for each query
  - Enables querying multiple databases in the same session
  - Properly maintains database context per query execution
  - Automatically escapes special characters in database names

### Fixed
- **Database Context Persistence**: Fixed issue where database context was not maintained between queries
  - Previously, `USE database` commands would not persist across query invocations
  - Each query would get a different connection from the pool, losing context
  - Users had to use fully qualified table names (database.table) for all queries
  - Now, users can specify database context per query using the `database` parameter

### Technical Details
- Modified `QueryArguments` struct to include optional `database: Option<String>` field
- Updated `execute_query` function to acquire dedicated connection when database is specified
- Executes `USE database` command before the actual query on the same connection
- Added proper error handling for connection acquisition and database context switching
- Error codes: -32005 (connection acquisition), -32006 (database context switch)

### Migration Guide

**Before (required fully qualified names):**
```sql
SELECT * FROM dev_smartConnect_za.crm_sites LIMIT 10;
```

**After (using database parameter):**
```json
{
  "query": "SELECT * FROM crm_sites LIMIT 10",
  "database": "dev_smartConnect_za"
}
```

**Still supported (backward compatible):**
```sql
SELECT * FROM dev_smartConnect_za.crm_sites LIMIT 10;
```

### Documentation
- Added comprehensive documentation in `.ai-instructions/DATABASE_CONTEXT_FIX.md`
- Updated README.md with database parameter usage examples
- Added testing examples for database context switching

## [0.1.0] - Initial Release

### Added
- Basic MCP MySQL server implementation
- Support for MySQL/MariaDB databases
- Connection pooling with configurable parameters
- JSON-RPC 2.0 protocol support
- MCP protocol version 2025-03-26

### Tools
- `mysql`: Retrieve database schema information for tables
- `query`: Execute SQL queries (SELECT only by default)
- `insert`: Insert data into specified tables
- `update`: Update data based on conditions
- `delete`: Delete data based on conditions

### Features
- Command-line arguments for database connection configuration
- `--allow-dangerous-queries` flag for enabling write operations via query tool
- Connection retry logic with exponential backoff
- Comprehensive error handling and logging
- Environment variable support for RUST_LOG

### Security
- Query validation to prevent dangerous operations (unless explicitly allowed)
- SQL injection protection through parameterized queries
- Read-only mode by default for query tool

### Configuration
- Support for Zed AI assistant integration
- stdio-based communication
- Configurable connection parameters (host, port, username, password, database)