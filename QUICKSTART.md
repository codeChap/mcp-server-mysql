# Quick Start Guide

Get up and running with the MySQL MCP Server in 5 minutes.

## Step 1: Build the Server

```bash
cargo build --release
```

The binary will be at `target/release/mcp-server-mysql`

## Step 2: Test the Connection

```bash
./target/release/mcp-server-mysql \
  --host localhost \
  --username root \
  --password yourpassword \
  --database testdb
```

If you see "MCP MySQL Server started and ready to accept connections", you're good to go!

## Step 3: Configure Claude Desktop

Edit your Claude Desktop configuration file:

- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

Add:

```json
{
  "mcpServers": {
    "mysql": {
      "command": "/absolute/path/to/mcp-server-mysql",
      "args": [
        "--username", "your_username",
        "--password", "your_password",
        "--database", "your_database"
      ]
    }
  }
}
```

## Step 4: Restart Claude Desktop

Close and reopen Claude Desktop. You should see a small hammer icon (🔨) indicating the MCP server is connected.

## Step 5: Try it Out!

Ask Claude:

> "Can you show me the schema for the users table in my MySQL database?"

> "Query the database and show me the first 10 rows from the products table"

> "What tables are in my database?"

## Common Issues

### "Connection refused"

- Make sure MySQL is running
- Check your host/port settings
- Verify firewall isn't blocking the connection

### "Access denied"

- Double-check your username and password
- Ensure the user has permissions on the database

### Tool not appearing in Claude

- Verify the path to the binary is absolute (not relative)
- Check Claude Desktop logs for errors
- Restart Claude Desktop completely

## Next Steps

- Read the full [README.md](README.md) for all features
- See [example-config.json](example-config.json) for more configuration examples
- Enable write operations with `--allow-dangerous-queries true` (use with caution!)

## Safety Tips

🛡️ **Default Mode is Read-Only**
- Only SELECT queries work by default
- This protects your data from accidental modifications

🔓 **Write Mode**
- Add `--allow-dangerous-queries true` to enable writes
- Use this carefully, especially on production databases

🔐 **Security**
- Don't commit passwords to version control
- Consider using environment variables for sensitive data
- Use dedicated database users with limited permissions
