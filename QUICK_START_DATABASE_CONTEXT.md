# Quick Start: Database Context Feature

## 🎯 What's New?

You can now specify which database to use for each query using the `database` parameter!

## 🚀 Quick Example

### Before (Required Fully Qualified Names)
```sql
SELECT * FROM dev_smartConnect_za.crm_sites 
  JOIN dev_smartConnect_za.crm_orgs 
  ON dev_smartConnect_za.crm_sites.org_id = dev_smartConnect_za.crm_orgs.id;
```
**Painful!** 😫

### After (Clean and Simple)
```json
{
  "query": "SELECT * FROM crm_sites JOIN crm_orgs ON crm_sites.org_id = crm_orgs.id",
  "database": "dev_smartConnect_za"
}
```
**Much better!** 🎉

## 📖 How To Use

### Option 1: Specify Database for Each Query (Recommended)
```json
{
  "name": "query",
  "arguments": {
    "query": "SELECT COUNT(*) FROM users",
    "database": "my_database"
  }
}
```

### Option 2: Use Default Database (Omit Parameter)
```json
{
  "name": "query",
  "arguments": {
    "query": "SELECT COUNT(*) FROM users"
  }
}
```
Uses the database specified in `--database` startup argument.

### Option 3: Still Use Fully Qualified Names (Still Works)
```json
{
  "query": "SELECT * FROM my_database.users"
}
```

## 🔥 Common Use Cases

### Single Database Project
```bash
# Startup with default database
mcp-server-mysql --username user --password pass --database my_project

# Query (no database param needed)
{
  "query": "SELECT * FROM users"
}
```

### Multiple Database Project
```json
// Customer database
{
  "query": "SELECT COUNT(*) FROM customers",
  "database": "customers_db"
}

// Orders database
{
  "query": "SELECT COUNT(*) FROM orders",
  "database": "orders_db"
}

// Analytics database
{
  "query": "SELECT SUM(revenue) FROM sales",
  "database": "analytics_db"
}
```

### Verify Which Database You're Using
```json
{
  "query": "SELECT DATABASE()",
  "database": "dev_smartConnect_za"
}
```
Returns: `"dev_smartConnect_za"`

## ⚠️ Common Pitfalls

### ❌ DON'T: Assume Context Persists
```json
// Query 1
{
  "query": "USE dev_smartConnect_za"  // ❌ Won't persist!
}

// Query 2
{
  "query": "SELECT * FROM crm_sites"  // ❌ Won't work!
}
```

### ✅ DO: Specify Database Each Time
```json
// Query 1
{
  "query": "SELECT * FROM crm_sites",
  "database": "dev_smartConnect_za"  // ✅ Explicit
}

// Query 2
{
  "query": "SELECT * FROM crm_orgs",
  "database": "dev_smartConnect_za"  // ✅ Explicit
}
```

## 🐛 Troubleshooting

### Error: "Failed to set database context"
**Problem**: Database doesn't exist or you don't have permission.

**Solution**:
```sql
-- Check available databases
SHOW DATABASES;

-- Verify you have access
SELECT * FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = 'your_database';
```

### Error: "Table doesn't exist"
**Problem**: Wrong database context.

**Solution**: Add the `database` parameter!
```json
{
  "query": "SELECT * FROM my_table",
  "database": "correct_database"  // ← Add this!
}
```

### Query returns wrong data
**Problem**: Querying wrong database.

**Solution**: Check current database:
```json
{
  "query": "SELECT DATABASE()"
}
```

## 📚 More Information

- **Quick Reference**: `DATABASE_CONTEXT_QUICK_GUIDE.md`
- **Full Documentation**: `.ai-instructions/DATABASE_CONTEXT_FIX.md`
- **Changelog**: `CHANGELOG.md`
- **Implementation Details**: `IMPLEMENTATION_SUMMARY.md`
- **Test Cases**: `test_database_context.json`

## ✨ Summary

| Feature | Status |
|---------|--------|
| Add `database` parameter | ✅ Works |
| Multiple databases | ✅ Supported |
| Backward compatible | ✅ Yes |
| Performance impact | ✅ Negligible |
| Easy to use | ✅ Very |

**That's it!** Just add `"database": "your_database"` to your queries. Simple! 🚀

---

**Pro Tip**: Start with the `database` parameter on all queries. It's explicit, clear, and prevents mistakes!