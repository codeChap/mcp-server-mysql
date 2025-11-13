# Deployment Guide

## v0.1.1 - Production Ready Release

### What's Included

The deployment package contains:
- `mcp-server-mysql` - Optimized release binary (5.4MB)
- `README.md` - Complete documentation
- `QUICKSTART.md` - Quick setup guide
- `example-config.json` - Configuration templates

### Installation

#### Option 1: From Release Package

```bash
# Extract the package
tar -xzf mcp-server-mysql-v0.1.1-linux-x86_64.tar.gz

# Move binary to system path (optional)
sudo cp mcp-server-mysql /usr/local/bin/

# Verify installation
mcp-server-mysql --version
```

#### Option 2: Build from Source

```bash
# Clone the repository
git clone <repository-url>
cd mcp-server-mysql

# Checkout the release
git checkout v0.1.1

# Build
cargo build --release

# Binary is at target/release/mcp-server-mysql
```

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

Edit your Claude config file and add:

```json
{
  "mcpServers": {
    "mysql": {
      "command": "/full/path/to/mcp-server-mysql",
      "args": [
        "--username", "your_username",
        "--password", "your_password",
        "--database", "your_database"
      ]
    }
  }
}
```

#### 3. Restart Claude Desktop

Close and reopen Claude Desktop completely.

### Production Deployment Tips

#### Security

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

### Troubleshooting

#### Binary doesn't run

```bash
# Check architecture
file mcp-server-mysql
# Should show: ELF 64-bit LSB executable, x86-64

# Make executable
chmod +x mcp-server-mysql
```

#### Connection issues

```bash
# Test MySQL connection
mysql -h localhost -u your_user -p your_database

# Check firewall
sudo ufw status
```

#### Claude doesn't see the server

1. Verify absolute path in config (not relative)
2. Check Claude Desktop logs
3. Restart Claude completely (not just reload)

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

### Support

For issues or questions:
- Check the README.md for detailed documentation
- Review QUICKSTART.md for common setup issues
- Open an issue on the project repository

### Version Information

- **Version**: 0.1.1
- **Protocol**: MCP 2025-03-26
- **Platform**: Linux x86_64
- **Rust Version**: 1.70+
- **Binary Size**: 5.4MB (optimized)
