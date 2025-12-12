use log::{debug, error, info, warn};
use serde_json::{json, Value};
use sqlx::{Column, MySql, Pool, Row, TypeInfo};
use std::time::Duration;
use crate::rpc::{JsonRpcResponse, JsonRpcError};

/// Validates that an identifier (table name, database name) contains only alphanumeric characters or underscores.
/// This is crucial for preventing SQL injection in statements where parameters cannot be used (e.g. USE, FROM).
pub fn is_valid_identifier(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_')
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

pub fn create_error_response(id: Option<Value>, code: i32, message: &str) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message: message.to_string(),
            data: None,
        }),
    }
}

pub async fn get_schema(
    id: Option<Value>,
    table_name: String,
    pool: &Pool<MySql>,
) -> JsonRpcResponse {
    debug!("Getting schema for: {table_name}");
    
    if table_name == "all-tables" {
        // Get all table schemas
        match get_all_table_schemas(pool).await {
            Ok(schemas) => {
                info!("Successfully retrieved schemas for {} tables", schemas.len());
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({
                        "content": [{
                            "type": "text",
                            "text": format!("Retrieved schemas for {} tables.", schemas.len())
                        }],
                        "schemas": schemas
                    })),
                    error: None,
                }
            }
            Err(e) => {
                error!("Database error getting all table schemas: {e}");
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32603,
                        message: format!("Failed to get table schemas: {e}"),
                        data: None,
                    }),
                }
            }
        }
    } else {
        // Get single table schema
        if !is_valid_identifier(&table_name) {
             return create_error_response(id, -32602, "Invalid table name: must contain only alphanumeric characters and underscores");
        }

        match get_table_schema(pool, &table_name).await {
            Ok(schema) => {
                info!("Successfully retrieved schema for table '{table_name}'");
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({
                        "content": [{
                            "type": "text",
                            "text": format!("Retrieved schema for table '{}'.", table_name)
                        }],
                        "schema": schema
                    })),
                    error: None,
                }
            }
            Err(e) => {
                error!("Database error getting schema for table '{table_name}': {e}");
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32603,
                        message: format!("Failed to get schema for table '{table_name}': {e}"),
                        data: None,
                    }),
                }
            }
        }
    }
}

async fn get_table_schema(pool: &Pool<MySql>, table_name: &str) -> Result<Value, sqlx::Error> {
    // Note: table_name should be validated before calling this
    let current_db: Option<String> = sqlx::query_scalar("SELECT DATABASE()").fetch_optional(pool).await?;
    if current_db.is_none() {
        return Err(sqlx::Error::Configuration("No database selected. Please specify a database to use.".into()));
    }
    let current_db = current_db.unwrap();
    
    // Get table information
    // We use parameters for the WHERE clause values, which is safe from injection.
    // However, table_name and current_db in the logic below (if used in identifiers) would need care.
    // Here we are selecting * values * from information_schema, so binding is correct and sufficient.
    let table_info_query = "SELECT * FROM information_schema.tables WHERE table_name = ? AND table_schema = ?";
    let table_info = sqlx::query(table_info_query)
        .bind(table_name)
        .bind(&current_db)
        .fetch_optional(pool)
        .await?;
    
    if table_info.is_none() {
        return Err(sqlx::Error::RowNotFound);
    }
    
    // Get column information
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
    
    // Get indexes
    // SHOW INDEX requires the table name as an identifier.
    // Since we validated table_name with is_valid_identifier, this is safe from injection.
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

async fn get_all_table_schemas(pool: &Pool<MySql>) -> Result<Vec<Value>, sqlx::Error> {
    let current_db: Option<String> = sqlx::query_scalar("SELECT DATABASE()").fetch_optional(pool).await?;
    if current_db.is_none() {
        return Err(sqlx::Error::Configuration("No database selected. Please specify a database to use.".into()));
    }
    let current_db = current_db.unwrap();
    
    // Get all tables in the current database
    let tables_query = "SELECT table_name FROM information_schema.tables WHERE table_schema = ? AND table_type = 'BASE TABLE'";
    let tables = sqlx::query(tables_query)
        .bind(current_db)
        .fetch_all(pool)
        .await?;
    
    let mut schemas = Vec::new();
    for table_row in tables {
        let table_name: String = table_row.try_get("table_name")?;
        // Internal tables from information_schema might have special chars? unlikely for base tables but good to check if we use format! later.
        // However, here we just pass it to get_table_schema which validates or binds it.
        // Actually, get_table_schema performs SHOW INDEX which uses format!.
        // We should trust the database's own table names, but purely for safety we can skip weird ones if needed.
        // For now, we assume database table names are valid enough to be quoted with backticks.
        match get_table_schema(pool, &table_name).await {
            Ok(schema) => schemas.push(schema),
            Err(e) => {
                // Log but continue
                warn!("Failed to get schema for table {table_name}: {e}");
            }
        }
    }
    
    Ok(schemas)
}

pub async fn execute_query(
    id: serde_json::Value,
    query: String,
    database: Option<String>,
    pool: &Pool<MySql>,
    allow_dangerous_queries: bool,
) -> JsonRpcResponse {
    // Validate queries unless dangerous queries are allowed
    if !allow_dangerous_queries {
        let trimmed_query = query.trim();
        if !trimmed_query.to_uppercase().starts_with("SELECT") {
            return create_error_response(Some(id), -32602, "Only SELECT queries are allowed. Use --allow-dangerous-queries flag to execute other query types.");
        }
        
        let dangerous_keywords = ["INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER", "TRUNCATE", "GRANT", "REVOKE"];
        let query_upper = trimmed_query.to_uppercase();
        for keyword in &dangerous_keywords {
            if query_upper.contains(keyword) {
                return create_error_response(Some(id), -32602, &format!("Query contains forbidden keyword: {}. Use --allow-dangerous-queries flag to allow such queries.", keyword));
            }
        }
    }

    debug!("Executing query: {}", query);
    
    let result = if let Some(db) = database {
        debug!("Setting database context to: {}", db);
        if !is_valid_identifier(&db) {
             return create_error_response(Some(id), -32602, "Invalid database name");
        }
        
        let mut conn = match pool.acquire().await {
            Ok(conn) => conn,
            Err(e) => {
                error!("Failed to acquire connection: {}", e);
                return create_error_response(Some(id), -32005, &format!("Failed to acquire database connection: {}", e));
            }
        };
        
        // Safe now because we validated `db`
        let use_query = format!("USE `{}`", db); 
        if let Err(e) = sqlx::query(&use_query).execute(&mut *conn).await {
            error!("Failed to set database context to '{}': {}", db, e);
            return create_error_response(Some(id), -32006, &format!("Failed to set database context to '{}': {}", db, e));
        }
        
        sqlx::query(&query).fetch_all(&mut *conn).await
    } else {
        sqlx::query(&query).fetch_all(pool).await
    };
    
    match result {
        Ok(rows) => {
            let mut results = Vec::new();
            
            for row in rows {
                let mut row_data = serde_json::Map::new();
                
                for (i, column) in row.columns().iter().enumerate() {
                    let column_name = column.name();
                    let type_name = column.type_info().name();
                    
                    let value_json = match type_name {
                        "BOOLEAN" | "TINYINT" => {
                            // Map tinyint(1) to bool if possible, else int
                            if let Ok(v) = row.try_get::<Option<bool>, _>(i) {
                                json!(v)
                            } else {
                                json!(row.try_get::<Option<i64>, _>(i).unwrap_or(None))
                            }
                        },
                        "SMALLINT" | "INT" | "INTEGER" | "BIGINT" => {
                            json!(row.try_get::<Option<i64>, _>(i).unwrap_or(None))
                        },
                        "FLOAT" | "DOUBLE" | "REAL" => {
                            json!(row.try_get::<Option<f64>, _>(i).unwrap_or(None))
                        },
                        "DECIMAL" | "NUMERIC" => {
                             // Serialize BigDecimal as string to preserve precision
                             if let Ok(v) = row.try_get::<Option<sqlx::types::BigDecimal>, _>(i) {
                                 json!(v.map(|d| d.to_string()))
                             } else {
                                 json!(null)
                             }
                        },
                        "DATE" | "TIME" | "DATETIME" | "TIMESTAMP" => {
                             // Determine handling for dates. String is safest for JSON.
                             json!(row.try_get::<Option<String>, _>(i).unwrap_or(None))
                        },
                        _ => {
                            // Fallback to string for everything else (VARCHAR, TEXT, BLOB, JSON, etc)
                            json!(row.try_get::<Option<String>, _>(i).unwrap_or(None))
                        }
                    };
                    
                    row_data.insert(column_name.to_string(), value_json);
                }
                
                results.push(json!(row_data));
            }
            
            let mut content_text = format!("Query executed successfully. Retrieved {} rows.\n\n", results.len());
            
            if !results.is_empty() {
                content_text.push_str("Results:\n");
                content_text.push_str(&serde_json::to_string_pretty(&results).unwrap_or_else(|_| "Error formatting results".to_string()));
            }
            
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(id),
                result: Some(json!({
                    "content": [{
                        "type": "text",
                        "text": content_text
                    }]
                })),
                error: None,
            }
        }
        Err(e) => {
            error!("Query execution failed: {}", e);
            create_error_response(Some(id), -32004, &format!("Query execution failed: {}", e))
        }
    }
}

pub async fn insert_data(
    id: serde_json::Value,
    table_name: String,
    data: serde_json::Value,
    pool: &Pool<MySql>,
) -> JsonRpcResponse {
    let mut conn = match pool.acquire().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Failed to get connection: {}", e);
            return create_error_response(Some(id), -32003, &format!("Database connection error: {}", e));
        }
    };

    if !is_valid_identifier(&table_name) {
        return create_error_response(Some(id), -32602, "Invalid table name");
    }

    let data_map = match data.as_object() {
        Some(map) => map,
        None => {
            return create_error_response(Some(id), -32602, "Data must be an object");
        }
    };

    if data_map.is_empty() {
        return create_error_response(Some(id), -32602, "Data object is empty");
    }

    let columns: Vec<String> = data_map.keys().cloned().collect();
    // Validate column names too!
    for col in &columns {
        if !is_valid_identifier(col) {
            return create_error_response(Some(id), -32602, &format!("Invalid column name: {}", col));
        }
    }

    let placeholders: Vec<String> = (0..columns.len()).map(|_| "?".to_string()).collect();
    let query = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        table_name,
        columns.join(", "),
        placeholders.join(", ")
    );

    let mut query_builder = sqlx::query(&query);
    for column in &columns {
        if let Some(value) = data_map.get(column) {
            query_builder = query_builder.bind(value);
        }
    }

    debug!("Executing insert query: {}", query);
    match query_builder.execute(&mut *conn).await {
        Ok(_) => {
            let last_id: u64 = match sqlx::query_scalar("SELECT LAST_INSERT_ID()")
                .fetch_one(&mut *conn)
                .await
            {
                Ok(id) => id,
                Err(e) => {
                    error!("Failed to get last insert ID: {}", e);
                    0
                }
            };
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(id),
                result: Some(json!({
                    "success": true,
                    "last_insert_id": last_id
                })),
                error: None,
            }
        }
        Err(e) => {
            error!("Insert failed: {}", e);
            create_error_response(Some(id), -32004, &format!("Insert failed: {}", e))
        }
    }
}

pub async fn update_data(
    id: serde_json::Value,
    table_name: String,
    data: serde_json::Value,
    conditions: serde_json::Value,
    pool: &Pool<MySql>,
) -> JsonRpcResponse {
    let mut conn = match pool.acquire().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Failed to get connection: {}", e);
            return create_error_response(Some(id), -32003, &format!("Database connection error: {}", e));
        }
    };

    if !is_valid_identifier(&table_name) {
        return create_error_response(Some(id), -32602, "Invalid table name");
    }

    let data_map = match data.as_object() {
        Some(map) => map,
        None => {
            return create_error_response(Some(id), -32602, "Data must be an object");
        }
    };

    let conditions_map = match conditions.as_object() {
        Some(map) => map,
        None => {
            return create_error_response(Some(id), -32602, "Conditions must be an object");
        }
    };

    if data_map.is_empty() {
        return create_error_response(Some(id), -32602, "Data object is empty");
    }

    if conditions_map.is_empty() {
        return create_error_response(Some(id), -32602, "Conditions object is empty");
    }

    // Validate keys
    for k in data_map.keys().chain(conditions_map.keys()) {
        if !is_valid_identifier(k) {
             return create_error_response(Some(id), -32602, &format!("Invalid column name: {}", k));
        }
    }

    let set_clause: Vec<String> = data_map.keys().map(|k| format!("{} = ?", k)).collect();
    let where_clause: Vec<String> = conditions_map.keys().map(|k| format!("{} = ?", k)).collect();
    let query = format!(
        "UPDATE {} SET {} WHERE {}",
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
    match query_builder.execute(&mut *conn).await {
        Ok(result) => {
            let affected_rows = result.rows_affected();
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(id),
                result: Some(json!({
                    "success": true,
                    "affected_rows": affected_rows
                })),
                error: None,
            }
        }
        Err(e) => {
            error!("Update failed: {}", e);
            create_error_response(Some(id), -32004, &format!("Update failed: {}", e))
        }
    }
}

pub async fn delete_data(
    id: serde_json::Value,
    table_name: String,
    conditions: serde_json::Value,
    pool: &Pool<MySql>,
) -> JsonRpcResponse {
    let mut conn = match pool.acquire().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Failed to get connection: {}", e);
            return create_error_response(Some(id), -32003, &format!("Database connection error: {}", e));
        }
    };

    if !is_valid_identifier(&table_name) {
        return create_error_response(Some(id), -32602, "Invalid table name");
    }

    let conditions_map = match conditions.as_object() {
        Some(map) => map,
        None => {
            return create_error_response(Some(id), -32602, "Conditions must be an object");
        }
    };

    if conditions_map.is_empty() {
        return create_error_response(Some(id), -32602, "Conditions object is empty");
    }

     // Validate keys
    for k in conditions_map.keys() {
        if !is_valid_identifier(k) {
             return create_error_response(Some(id), -32602, &format!("Invalid column name: {}", k));
        }
    }

    let where_clause: Vec<String> = conditions_map.keys().map(|k| format!("{} = ?", k)).collect();
    let query = format!(
        "DELETE FROM {} WHERE {}",
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
    match query_builder.execute(&mut *conn).await {
        Ok(result) => {
            let affected_rows = result.rows_affected();
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(id),
                result: Some(json!({
                    "success": true,
                    "affected_rows": affected_rows
                })),
                error: None,
            }
        }
        Err(e) => {
            error!("Delete failed: {}", e);
            create_error_response(Some(id), -32004, &format!("Delete failed: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_identifier() {
        assert!(is_valid_identifier("users"));
        assert!(is_valid_identifier("my_table_123"));
        assert!(is_valid_identifier("_hidden"));
        assert!(is_valid_identifier("CamelCase"));

        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("users; DROP TABLE users"));
        assert!(!is_valid_identifier("users--"));
        assert!(!is_valid_identifier("table with spaces"));
        assert!(!is_valid_identifier("table-with-dashes"));
        assert!(is_valid_identifier("123")); 
    }
}
