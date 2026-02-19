use log::{debug, error, info, warn};
use serde_json::json;
use sqlx::{MySql, Pool};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::config::Config;
use crate::db::{connect_with_retry, execute_query, get_schema, insert_data, update_data, delete_data};
use crate::error::DbError;
use crate::rpc::{
    InitializeParams,
    InitializeResult,
    JsonRpcResponse,
    JsonRpcRequest,
    ServerCapabilities,
    ServerInfo,
    Tool,
    ToolsCapability,
    ToolsList,
    ToolCallParams,
    SchemaArguments,
    QueryArguments,
    InsertArguments,
    UpdateArguments,
    DeleteArguments,
};

/// Redact password from a database URL for safe logging.
fn redact_url(url: &str) -> String {
    // Pattern: mysql://user:password@host:port/db
    // Replace password portion between first ':' after '://' and '@'
    if let Some(scheme_end) = url.find("://") {
        let after_scheme = &url[scheme_end + 3..];
        if let Some(at_pos) = after_scheme.find('@') {
            let user_pass = &after_scheme[..at_pos];
            if let Some(colon_pos) = user_pass.find(':') {
                let user = &user_pass[..colon_pos];
                let after_at = &after_scheme[at_pos..];
                return format!("{}://{}:***{}", &url[..scheme_end], user, after_at);
            }
        }
    }
    url.to_string()
}

/// Map a DbError to a JSON-RPC error response with appropriate error codes.
fn db_error_to_response(id: Option<serde_json::Value>, err: DbError) -> JsonRpcResponse {
    let (code, message) = match &err {
        DbError::InvalidIdentifier(_) => (-32602, err.to_string()),
        DbError::InvalidInput(_) => (-32602, err.to_string()),
        DbError::ReadOnlyViolation(_) => (-32602, err.to_string()),
        DbError::ConnectionError(_) => (-32003, err.to_string()),
        DbError::SqlError(_) => (-32004, err.to_string()),
        DbError::NotFound(_) => (-32604, err.to_string()),
        DbError::NoDatabaseSelected => (-32005, err.to_string()),
    };
    JsonRpcResponse::error(id, code, message)
}

macro_rules! dispatch_tool {
    ($id:expr, $arguments:expr, $args_type:ty, $handler:expr, $to_result:expr) => {{
        let args: $args_type = match serde_json::from_value($arguments) {
            Ok(a) => a,
            Err(e) => return JsonRpcResponse::error($id, -32602, format!("Invalid arguments: {e}")),
        };
        match $handler(args).await {
            Ok(result) => JsonRpcResponse::success($id, $to_result(result)),
            Err(e) => db_error_to_response($id, e),
        }
    }};
}

pub async fn run(args: Config) -> Result<(), Box<dyn std::error::Error>> {
    let allow_dangerous_queries = args.allow_dangerous_queries;

    let mut pool: Option<Pool<MySql>> = None;

    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    info!("MCP MySQL Server started and ready to accept connections");
    info!("Server config: host={}, port={}, username={}, database={}",
              args.host, args.port, args.username, args.database);
    info!("Server PID: {}", std::process::id());
    debug!("Environment variables:");
    for (key, value) in std::env::vars() {
        if key.contains("MYSQL") || key.contains("DATABASE") || key.contains("MCP") {
            // Redact potentially sensitive env vars
            if key.to_uppercase().contains("PASSWORD") || key.to_uppercase().contains("SECRET") {
                debug!("  {key}: ***");
            } else {
                debug!("  {key}: {value}");
            }
        }
    }
    debug!("Current working directory: {:?}", std::env::current_dir());

    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                if line.trim().is_empty() {
                    continue;
                }

                debug!("Received message (len={}): {}", line.len(), line);
                match serde_json::from_str::<JsonRpcRequest>(&line) {
                    Ok(request) => {
                        debug!("Parsed request: method={}, id={:?}", request.method, request.id);
                        // Handle notifications (no response needed)
                        if request.method == "notifications/initialized" || request.method == "initialized" {
                            debug!("Received initialization notification: {}", request.method);
                            continue;
                        }

                        let response = handle_request(request, &mut pool, &args, allow_dangerous_queries).await;
                        match serde_json::to_string(&response) {
                            Ok(response_str) => {
                                if let Err(e) = write_response(&mut stdout, &response_str).await {
                                    error!("Failed to write response: {e}");
                                }
                            }
                            Err(e) => {
                                error!("Failed to serialize response: {e}");
                                let error_response = JsonRpcResponse::error(None, -32603, "Internal error".to_string());
                                if let Ok(error_str) = serde_json::to_string(&error_response) {
                                    let _ = write_response(&mut stdout, &error_str).await;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse request: {e}");
                        let error_response = JsonRpcResponse::error(None, -32700, "Parse error".to_string());
                        if let Ok(response_str) = serde_json::to_string(&error_response) {
                            let _ = write_response(&mut stdout, &response_str).await;
                        }
                    }
                }
            }
            Ok(None) => {
                info!("stdin closed - client disconnected, shutting down server");
                break;
            }
            Err(e) => {
                warn!("Error reading from stdin: {e} (error kind: {:?})", e.kind());
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    info!("Unexpected EOF - client may have terminated");
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }

    info!("MCP MySQL Server shutdown complete");
    Ok(())
}

async fn write_response(stdout: &mut tokio::io::Stdout, response: &str) -> Result<(), Box<dyn std::error::Error>> {
    stdout.write_all(response.as_bytes()).await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;
    Ok(())
}

async fn handle_request(
    request: JsonRpcRequest,
    pool: &mut Option<Pool<MySql>>,
    args: &Config,
    allow_dangerous_queries: bool,
) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => {
            debug!("Handling initialize request with params: {:?}", request.params);

            let db_url_from_opts = request
                .params
                .as_ref()
                .and_then(|params| serde_json::from_value::<InitializeParams>(params.clone()).ok())
                .and_then(|opts| opts.initialization_options)
                .and_then(|init_opts| init_opts.settings)
                .and_then(|settings| settings.database_url);

            let database_url = match db_url_from_opts {
                Some(url) => {
                    info!("Using database_url from initializationOptions: {}", redact_url(&url));
                    url
                }
                None => {
                    let url = format!(
                        "mysql://{}:{}@{}:{}/{}",
                        args.username, args.password, args.host, args.port, args.database
                    );
                    info!("Using database_url from config: mysql://{}:***@{}:{}/{}",
                             args.username, args.host, args.port, args.database);
                    url
                }
            };

            info!("Attempting database connection...");
            match connect_with_retry(&database_url).await {
                Ok(new_pool) => {
                    info!("Database connection successful!");
                    *pool = Some(new_pool);
                    JsonRpcResponse::success(request.id, json!(InitializeResult {
                        protocol_version: "2025-03-26".to_string(),
                        capabilities: ServerCapabilities {
                            tools: Some(ToolsCapability {
                                list_changed: true,
                            }),
                        },
                        server_info: ServerInfo {
                            name: "mcp-server-mysql".to_string(),
                            version: "0.1.0".to_string(),
                        },
                    }))
                }
                Err(e) => {
                    error!("Database connection failed: {e}");
                    JsonRpcResponse::error(
                        request.id,
                        -32001,
                        format!("Database connection failed: {e}"),
                    )
                }
            }
        }
        "tools/list" => {
            debug!("Listing available tools");
            let mut tools = vec![
                Tool {
                    name: "mysql".to_string(),
                    description: "Retrieve MySQL database schema information for tables".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "table_name": {
                                "type": "string",
                                "description": "Name of the table to inspect, or 'all-tables' to get all table schemas"
                            }
                        },
                        "required": ["table_name"]
                    }),
                },
                Tool {
                    name: "query".to_string(),
                    description: if allow_dangerous_queries {
                        "Execute any SQL query on the database (unrestricted)".to_string()
                    } else {
                        "Execute a SELECT query on the database (read-only)".to_string()
                    },
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": if allow_dangerous_queries {
                                    "SQL query to execute"
                                } else {
                                    "SELECT query to execute"
                                }
                            },
                            "database": {
                                "type": "string",
                                "description": "Optional database name to use for this query. If specified, the query will be executed in the context of this database."
                            }
                        },
                        "required": ["query"]
                    }),
                },
            ];

            if allow_dangerous_queries {
                tools.push(Tool {
                    name: "insert".to_string(),
                    description: "Insert data into a specified table".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "table_name": {
                                "type": "string",
                                "description": "Name of the table to insert data into"
                            },
                            "data": {
                                "type": "object",
                                "description": "Data to insert as key-value pairs"
                            }
                        },
                        "required": ["table_name", "data"]
                    }),
                });
                tools.push(Tool {
                    name: "update".to_string(),
                    description: "Update data in a specified table based on conditions".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "table_name": {
                                "type": "string",
                                "description": "Name of the table to update data in"
                            },
                            "data": {
                                "type": "object",
                                "description": "Data to update as key-value pairs"
                            },
                            "conditions": {
                                "type": "object",
                                "description": "Conditions for update as key-value pairs"
                            }
                        },
                        "required": ["table_name", "data", "conditions"]
                    }),
                });
                tools.push(Tool {
                    name: "delete".to_string(),
                    description: "Delete data from a specified table based on conditions".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "table_name": {
                                "type": "string",
                                "description": "Name of the table to delete data from"
                            },
                            "conditions": {
                                "type": "object",
                                "description": "Conditions for deletion as key-value pairs"
                            }
                        },
                        "required": ["table_name", "conditions"]
                    }),
                });
            }

            JsonRpcResponse::success(request.id, json!(ToolsList { tools }))
        }
        "tools/call" => {
            let current_pool = match pool.as_ref() {
                Some(p) => p,
                None => {
                    return JsonRpcResponse::error(request.id, -32002, "Server not initialized".to_string());
                }
            };
            debug!("Handling tool call");
            match request.params {
                Some(params) => match serde_json::from_value::<ToolCallParams>(params) {
                    Ok(tool_params) => {
                        let id = request.id;
                        let max_rows = args.max_rows;
                        match tool_params.name.as_str() {
                            "mysql" => {
                                dispatch_tool!(id, tool_params.arguments, SchemaArguments,
                                    |args: SchemaArguments| get_schema(args.table_name, current_pool),
                                    |result: crate::db::SchemaResult| {
                                        if result.schemas.len() == 1 {
                                            json!({
                                                "content": [{
                                                    "type": "text",
                                                    "text": result.description
                                                }],
                                                "schema": result.schemas[0]
                                            })
                                        } else {
                                            json!({
                                                "content": [{
                                                    "type": "text",
                                                    "text": result.description
                                                }],
                                                "schemas": result.schemas
                                            })
                                        }
                                    }
                                )
                            }
                            "query" => {
                                dispatch_tool!(id, tool_params.arguments, QueryArguments,
                                    |args: QueryArguments| execute_query(args.query, args.database, current_pool, allow_dangerous_queries, max_rows),
                                    |result: crate::db::QueryResult| {
                                        let mut content_text = format!("Query executed successfully. Retrieved {} rows.", result.row_count);
                                        if result.truncated {
                                            content_text.push_str(&format!(" (truncated from more than {} rows)", max_rows));
                                        }
                                        if !result.rows.is_empty() {
                                            content_text.push_str("\n\nResults:\n");
                                            content_text.push_str(&serde_json::to_string_pretty(&result.rows).unwrap_or_else(|_| "Error formatting results".to_string()));
                                        }
                                        json!({
                                            "content": [{
                                                "type": "text",
                                                "text": content_text
                                            }]
                                        })
                                    }
                                )
                            }
                            "insert" | "update" | "delete" if !allow_dangerous_queries => {
                                JsonRpcResponse::error(id, -32601, format!("Tool '{}' is not available in read-only mode. Set allow_dangerous_queries = true in config.toml.", tool_params.name))
                            }
                            "insert" => {
                                dispatch_tool!(id, tool_params.arguments, InsertArguments,
                                    |args: InsertArguments| insert_data(args.table_name, args.data, current_pool),
                                    |result: crate::db::InsertResult| {
                                        json!({
                                            "content": [{
                                                "type": "text",
                                                "text": format!("Insert successful. Last insert ID: {}", result.last_insert_id)
                                            }]
                                        })
                                    }
                                )
                            }
                            "update" => {
                                dispatch_tool!(id, tool_params.arguments, UpdateArguments,
                                    |args: UpdateArguments| update_data(args.table_name, args.data, args.conditions, current_pool),
                                    |result: crate::db::MutationResult| {
                                        json!({
                                            "content": [{
                                                "type": "text",
                                                "text": format!("Update successful. Affected rows: {}", result.affected_rows)
                                            }]
                                        })
                                    }
                                )
                            }
                            "delete" => {
                                dispatch_tool!(id, tool_params.arguments, DeleteArguments,
                                    |args: DeleteArguments| delete_data(args.table_name, args.conditions, current_pool),
                                    |result: crate::db::MutationResult| {
                                        json!({
                                            "content": [{
                                                "type": "text",
                                                "text": format!("Delete successful. Affected rows: {}", result.affected_rows)
                                            }]
                                        })
                                    }
                                )
                            }
                            _ => JsonRpcResponse::error(id, -32601, format!("Unknown tool: {}", tool_params.name)),
                        }
                    }
                    Err(e) => JsonRpcResponse::error(request.id, -32602, format!("Invalid tool call parameters: {e}")),
                },
                None => JsonRpcResponse::error(request.id, -32602, "Missing parameters".to_string()),
            }
        }
        _ => {
            warn!("Unknown method: {}", request.method);
            JsonRpcResponse::error(request.id, -32601, format!("Method not found: {}", request.method))
        }
    }
}
