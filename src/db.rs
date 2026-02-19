use log::{debug, error, info, warn};
use serde_json::{json, Value};
use sqlx::{Column, MySql, Pool, Row, TypeInfo};
use std::time::Duration;
use crate::error::DbError;

/// Validates that an identifier (table name, database name) is safe for use in backtick-quoted SQL.
/// Rejects empty strings, strings longer than 64 chars, and strings containing backticks or null bytes.
pub fn is_valid_identifier(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.contains('`')
        && !name.contains('\0')
}

pub async fn connect_with_retry(database_url: &str) -> Result<Pool<MySql>, Box<dyn std::error::Error>> {
    let mut retry_count = 0;
    const MAX_RETRIES: u32 = 5;
    const RETRY_DELAY_MS: u64 = 1000;

    loop {
        match sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
        {
            Ok(pool) => {
                info!("Successfully connected to MySQL database");
                return Ok(pool);
            }
            Err(e) => {
                retry_count += 1;
                if retry_count >= MAX_RETRIES {
                    error!("Failed to connect to database after {MAX_RETRIES} retries: {e}");
                    return Err(e.into());
                }
                warn!("Database connection failed (attempt {retry_count}/{MAX_RETRIES}): {e}");
                info!("Retrying in {RETRY_DELAY_MS}ms...");
                tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
            }
        }
    }
}

// Domain result types

pub struct SchemaResult {
    pub schemas: Vec<Value>,
    pub description: String,
}

pub struct QueryResult {
    pub rows: Vec<Value>,
    pub row_count: usize,
    pub truncated: bool,
}

pub struct InsertResult {
    pub last_insert_id: u64,
}

pub struct MutationResult {
    pub affected_rows: u64,
}

pub async fn get_schema(
    table_name: String,
    pool: &Pool<MySql>,
) -> Result<SchemaResult, DbError> {
    debug!("Getting schema for: {table_name}");

    if table_name == "all-tables" {
        let schemas = get_all_table_schemas(pool).await?;
        let description = format!("Retrieved schemas for {} tables.", schemas.len());
        info!("Successfully retrieved schemas for {} tables", schemas.len());
        Ok(SchemaResult { schemas, description })
    } else {
        if !is_valid_identifier(&table_name) {
            return Err(DbError::InvalidIdentifier(table_name));
        }

        let schema = get_table_schema(pool, &table_name).await?;
        let description = format!("Retrieved schema for table '{}'.", table_name);
        info!("Successfully retrieved schema for table '{table_name}'");
        Ok(SchemaResult { schemas: vec![schema], description })
    }
}

async fn get_table_schema(pool: &Pool<MySql>, table_name: &str) -> Result<Value, DbError> {
    let current_db: Option<String> = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_optional(pool)
        .await?;
    let current_db = current_db.ok_or(DbError::NoDatabaseSelected)?;

    let table_info_query = "SELECT * FROM information_schema.tables WHERE table_name = ? AND table_schema = ?";
    let table_info = sqlx::query(table_info_query)
        .bind(table_name)
        .bind(&current_db)
        .fetch_optional(pool)
        .await?;

    if table_info.is_none() {
        return Err(DbError::NotFound(format!("Table '{}' not found", table_name)));
    }

    let columns_query =
        "SELECT column_name, data_type, is_nullable, column_default, column_key, extra, column_comment
         FROM information_schema.columns
         WHERE table_name = ? AND table_schema = ?
         ORDER BY ordinal_position";

    let columns = sqlx::query(columns_query)
        .bind(table_name)
        .bind(&current_db)
        .fetch_all(pool)
        .await?;

    let indexes_query = format!("SHOW INDEX FROM `{}`.`{}`", current_db, table_name);
    let indexes = sqlx::query(&indexes_query).fetch_all(pool).await?;

    let column_info: Vec<Value> = columns
        .into_iter()
        .map(|row| {
            json!({
                "name": row.try_get::<String, _>("column_name").unwrap_or_default(),
                "type": row.try_get::<String, _>("data_type").unwrap_or_default(),
                "nullable": row.try_get::<String, _>("is_nullable").unwrap_or_default() == "YES",
                "default": row.try_get::<Option<String>, _>("column_default").unwrap_or_default(),
                "key": row.try_get::<String, _>("column_key").unwrap_or_default(),
                "extra": row.try_get::<String, _>("extra").unwrap_or_default(),
                "comment": row.try_get::<String, _>("column_comment").unwrap_or_default(),
            })
        })
        .collect();

    let index_info: Vec<Value> = indexes
        .into_iter()
        .map(|row| {
            json!({
                "name": row.try_get::<String, _>("Key_name").unwrap_or_default(),
                "column": row.try_get::<String, _>("Column_name").unwrap_or_default(),
                "unique": row.try_get::<i32, _>("Non_unique").unwrap_or(1) == 0,
                "type": row.try_get::<String, _>("Index_type").unwrap_or_default(),
            })
        })
        .collect();

    Ok(json!({
        "table_name": table_name,
        "columns": column_info,
        "indexes": index_info
    }))
}

async fn get_all_table_schemas(pool: &Pool<MySql>) -> Result<Vec<Value>, DbError> {
    let current_db: Option<String> = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_optional(pool)
        .await?;
    let current_db = current_db.ok_or(DbError::NoDatabaseSelected)?;

    let tables_query = "SELECT table_name FROM information_schema.tables WHERE table_schema = ? AND table_type = 'BASE TABLE'";
    let tables = sqlx::query(tables_query)
        .bind(current_db)
        .fetch_all(pool)
        .await?;

    let mut schemas = Vec::new();
    for table_row in tables {
        let table_name: String = table_row.try_get("table_name")?;
        match get_table_schema(pool, &table_name).await {
            Ok(schema) => schemas.push(schema),
            Err(e) => {
                warn!("Failed to get schema for table {table_name}: {e}");
            }
        }
    }

    Ok(schemas)
}

pub async fn execute_query(
    query: String,
    database: Option<String>,
    pool: &Pool<MySql>,
    allow_dangerous_queries: bool,
    max_rows: usize,
) -> Result<QueryResult, DbError> {
    // Lightweight client-side pre-check (fast-fail, NOT a security boundary)
    if !allow_dangerous_queries {
        let trimmed_upper = query.trim().to_uppercase();
        if !trimmed_upper.starts_with("SELECT")
            && !trimmed_upper.starts_with("SHOW")
            && !trimmed_upper.starts_with("DESCRIBE")
            && !trimmed_upper.starts_with("EXPLAIN")
        {
            return Err(DbError::ReadOnlyViolation(
                "Only SELECT, SHOW, DESCRIBE, and EXPLAIN queries are allowed. Use --allow-dangerous-queries flag to execute other query types.".to_string(),
            ));
        }
    }

    debug!("Executing query: {}", query);

    let mut conn = pool.acquire().await.map_err(DbError::ConnectionError)?;

    // Set database context if specified
    if let Some(db) = database {
        debug!("Setting database context to: {}", db);
        if !is_valid_identifier(&db) {
            return Err(DbError::InvalidIdentifier(db));
        }
        let use_query = format!("USE `{}`", db);
        sqlx::query(&use_query).execute(&mut *conn).await?;
    }

    // Read-only enforcement via MySQL transaction
    if !allow_dangerous_queries {
        sqlx::query("START TRANSACTION READ ONLY")
            .execute(&mut *conn)
            .await?;
    }

    let result = sqlx::query(&query).fetch_all(&mut *conn).await;

    // Always rollback the read-only transaction (whether query succeeded or failed)
    if !allow_dangerous_queries {
        let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
    }

    let rows = result?;

    let total_count = rows.len();
    let truncated = total_count > max_rows;
    let rows_to_process = if truncated { &rows[..max_rows] } else { &rows[..] };

    let mut results = Vec::with_capacity(rows_to_process.len());

    for row in rows_to_process {
        let mut row_data = serde_json::Map::new();

        for (i, column) in row.columns().iter().enumerate() {
            let column_name = column.name();
            let type_name = column.type_info().name();

            let value_json = match type_name {
                "BOOLEAN" | "TINYINT" => {
                    if let Ok(v) = row.try_get::<Option<bool>, _>(i) {
                        json!(v)
                    } else {
                        json!(row.try_get::<Option<i64>, _>(i).unwrap_or(None))
                    }
                }
                "SMALLINT" | "INT" | "INTEGER" | "BIGINT" => {
                    json!(row.try_get::<Option<i64>, _>(i).unwrap_or(None))
                }
                "FLOAT" | "DOUBLE" | "REAL" => {
                    json!(row.try_get::<Option<f64>, _>(i).unwrap_or(None))
                }
                "DECIMAL" | "NUMERIC" => {
                    if let Ok(v) = row.try_get::<Option<sqlx::types::BigDecimal>, _>(i) {
                        json!(v.map(|d| d.to_string()))
                    } else {
                        json!(null)
                    }
                }
                "DATE" | "TIME" | "DATETIME" | "TIMESTAMP" => {
                    json!(row.try_get::<Option<String>, _>(i).unwrap_or(None))
                }
                _ => {
                    json!(row.try_get::<Option<String>, _>(i).unwrap_or(None))
                }
            };

            row_data.insert(column_name.to_string(), value_json);
        }

        results.push(json!(row_data));
    }

    Ok(QueryResult {
        row_count: results.len(),
        rows: results,
        truncated,
    })
}

pub async fn insert_data(
    table_name: String,
    data: Value,
    pool: &Pool<MySql>,
) -> Result<InsertResult, DbError> {
    let mut conn = pool.acquire().await.map_err(DbError::ConnectionError)?;

    if !is_valid_identifier(&table_name) {
        return Err(DbError::InvalidIdentifier(table_name));
    }

    let data_map = data
        .as_object()
        .ok_or_else(|| DbError::InvalidInput("Data must be an object".to_string()))?;

    if data_map.is_empty() {
        return Err(DbError::InvalidInput("Data object is empty".to_string()));
    }

    let columns: Vec<String> = data_map.keys().cloned().collect();
    for col in &columns {
        if !is_valid_identifier(col) {
            return Err(DbError::InvalidIdentifier(col.clone()));
        }
    }

    let placeholders: Vec<String> = (0..columns.len()).map(|_| "?".to_string()).collect();
    let query = format!(
        "INSERT INTO `{}` ({}) VALUES ({})",
        table_name,
        columns.iter().map(|c| format!("`{}`", c)).collect::<Vec<_>>().join(", "),
        placeholders.join(", ")
    );

    let mut query_builder = sqlx::query(&query);
    for column in &columns {
        if let Some(value) = data_map.get(column) {
            query_builder = query_builder.bind(value);
        }
    }

    debug!("Executing insert query: {}", query);
    query_builder.execute(&mut *conn).await?;

    let last_id: u64 = sqlx::query_scalar("SELECT LAST_INSERT_ID()")
        .fetch_one(&mut *conn)
        .await
        .unwrap_or(0);

    Ok(InsertResult { last_insert_id: last_id })
}

pub async fn update_data(
    table_name: String,
    data: Value,
    conditions: Value,
    pool: &Pool<MySql>,
) -> Result<MutationResult, DbError> {
    let mut conn = pool.acquire().await.map_err(DbError::ConnectionError)?;

    if !is_valid_identifier(&table_name) {
        return Err(DbError::InvalidIdentifier(table_name));
    }

    let data_map = data
        .as_object()
        .ok_or_else(|| DbError::InvalidInput("Data must be an object".to_string()))?;

    let conditions_map = conditions
        .as_object()
        .ok_or_else(|| DbError::InvalidInput("Conditions must be an object".to_string()))?;

    if data_map.is_empty() {
        return Err(DbError::InvalidInput("Data object is empty".to_string()));
    }

    if conditions_map.is_empty() {
        return Err(DbError::InvalidInput("Conditions object is empty".to_string()));
    }

    for k in data_map.keys().chain(conditions_map.keys()) {
        if !is_valid_identifier(k) {
            return Err(DbError::InvalidIdentifier(k.clone()));
        }
    }

    let set_clause: Vec<String> = data_map.keys().map(|k| format!("`{}` = ?", k)).collect();
    let where_clause: Vec<String> = conditions_map.keys().map(|k| format!("`{}` = ?", k)).collect();
    let query = format!(
        "UPDATE `{}` SET {} WHERE {}",
        table_name,
        set_clause.join(", "),
        where_clause.join(" AND ")
    );

    let mut query_builder = sqlx::query(&query);
    for key in data_map.keys() {
        if let Some(value) = data_map.get(key) {
            query_builder = query_builder.bind(value);
        }
    }
    for key in conditions_map.keys() {
        if let Some(value) = conditions_map.get(key) {
            query_builder = query_builder.bind(value);
        }
    }

    debug!("Executing update query: {}", query);
    let result = query_builder.execute(&mut *conn).await?;

    Ok(MutationResult {
        affected_rows: result.rows_affected(),
    })
}

pub async fn delete_data(
    table_name: String,
    conditions: Value,
    pool: &Pool<MySql>,
) -> Result<MutationResult, DbError> {
    let mut conn = pool.acquire().await.map_err(DbError::ConnectionError)?;

    if !is_valid_identifier(&table_name) {
        return Err(DbError::InvalidIdentifier(table_name));
    }

    let conditions_map = conditions
        .as_object()
        .ok_or_else(|| DbError::InvalidInput("Conditions must be an object".to_string()))?;

    if conditions_map.is_empty() {
        return Err(DbError::InvalidInput("Conditions object is empty".to_string()));
    }

    for k in conditions_map.keys() {
        if !is_valid_identifier(k) {
            return Err(DbError::InvalidIdentifier(k.clone()));
        }
    }

    let where_clause: Vec<String> = conditions_map.keys().map(|k| format!("`{}` = ?", k)).collect();
    let query = format!(
        "DELETE FROM `{}` WHERE {}",
        table_name,
        where_clause.join(" AND ")
    );

    let mut query_builder = sqlx::query(&query);
    for key in conditions_map.keys() {
        if let Some(value) = conditions_map.get(key) {
            query_builder = query_builder.bind(value);
        }
    }

    debug!("Executing delete query: {}", query);
    let result = query_builder.execute(&mut *conn).await?;

    Ok(MutationResult {
        affected_rows: result.rows_affected(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_identifier() {
        // Valid identifiers
        assert!(is_valid_identifier("users"));
        assert!(is_valid_identifier("my_table_123"));
        assert!(is_valid_identifier("_hidden"));
        assert!(is_valid_identifier("CamelCase"));
        assert!(is_valid_identifier("123"));
        assert!(is_valid_identifier("table-with-dashes"));
        assert!(is_valid_identifier("table with spaces"));
        assert!(is_valid_identifier("my.table"));

        // Invalid identifiers
        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("table`; DROP TABLE users"));
        assert!(!is_valid_identifier("name\0injection"));
        assert!(!is_valid_identifier(&"a".repeat(65)));

        // 64 chars is OK
        assert!(is_valid_identifier(&"a".repeat(64)));
    }
}
