use log::{debug, error, info, warn};
use serde_json::json;
use sqlx::{MySql, Pool};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::config::Args;
use crate::db::{connect_with_retry, create_error_response, execute_query, get_schema, insert_data, update_data, delete_data};
use crate::rpc::{
    InitializeParams,
    InitializeResult,
    JsonRpcError,
    JsonRpcRequest,
    JsonRpcResponse,
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

pub async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let allow_dangerous_queries = args.allow_dangerous_queries;
    
    // Defer database connection until initialize request is received
    let mut pool: Option<Pool<MySql>> = None;

    // Set up stdio
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    // Send logs to stderr to avoid interfering with JSON-RPC communication
    info!("MCP MySQL Server started and ready to accept connections");
    info!("Server args: host={}, port={}, username={}, database={}", 
              args.host, args.port, args.username, args.database);
    info!("Server PID: {}", std::process::id());
    debug!("Environment variables:");
    for (key, value) in std::env::vars() {
        if key.contains("MYSQL") || key.contains("DATABASE") || key.contains("MCP") {
            debug!("  {key}: {value}");
        }
    }
    debug!("Current working directory: {:?}", std::env::current_dir());

    // Process incoming messages
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                if line.trim().is_empty() {
                    continue;
                }

                debug!("Received message (len={}): {}", line.len(), line);
                debug!("Message bytes: {:?}", line.as_bytes());
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
                                    // Continue processing other requests
                                }
                            }
                            Err(e) => {
                                error!("Failed to serialize response: {e}");
                                // Send a generic error response
                                let error_response = create_error_response(None, -32603, "Internal error");
                                if let Ok(error_str) = serde_json::to_string(&error_response) {
                                    let _ = write_response(&mut stdout, &error_str).await;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse request: {e}");
                        let error_response = create_error_response(None, -32700, "Parse error");
                        if let Ok(response_str) = serde_json::to_string(&error_response) {
                            let _ = write_response(&mut stdout, &response_str).await;
                        }
                    }
                }
            }
            Ok(None) => {
                // stdin closed, this is normal when client disconnects
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
    args: &Args,
    allow_dangerous_queries: bool,
) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => {
            debug!("Handling initialize request with params: {:?}", request.params);

            // Extract database_url from initializationOptions, fallback to args
            let db_url_from_opts = request
                .params
                .as_ref()
                .and_then(|params| serde_json::from_value::<InitializeParams>(params.clone()).ok())
                .and_then(|opts| opts.initialization_options)
                .and_then(|init_opts| init_opts.settings)
                .and_then(|settings| settings.database_url);

            let database_url = match db_url_from_opts {
                Some(url) => {
                    info!("Using database_url from initializationOptions: {url}");
                    url
                }
                None => {
                    let url = format!(
                        "mysql://{}:{}@{}:{}/{}",
                        args.username, args.password, args.host, args.port, args.database
                    );
                    info!("Using database_url from command-line arguments: mysql://{}:***@{}:{}/{}", 
                             args.username, args.host, args.port, args.database);
                    url
                }
            };

            info!("Attempting database connection...");
            match connect_with_retry(&database_url).await {
                Ok(new_pool) => {
                    info!("Database connection successful!");
                    *pool = Some(new_pool);
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: Some(json! (InitializeResult {
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
                        })),
                        error: None,
                    }
                }
                Err(e) => {
                    error!("Database connection failed: {e}");
                    create_error_response(
                        request.id,
                        -32001,
                        &format!("Database connection failed: {e}"),
                    )
                }
            }
        }
        "notifications/initialized" | "initialized" => {
            info!("Client initialized");
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(json!({})) ,
                error: None,
            }
        }
        "tools/list" => {
            debug!("Listing available tools");
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(json!(ToolsList {
                    tools: vec![
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
                        Tool {
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
                        },
                        Tool {
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
                        },
                        Tool {
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
                        },
                    ],
                })),
                error: None,
            }
        }
        "tools/call" => {
            let current_pool = match pool.as_ref() {
                Some(p) => p,
                None => {
                    return create_error_response(request.id, -32002, "Server not initialized");
                }
            };
            debug!("Handling tool call");
            match request.params {
                Some(params) => match serde_json::from_value::<ToolCallParams>(params) {
                    Ok(tool_params) => {
                        match tool_params.name.as_str() {
                            "mysql" => {
                                match serde_json::from_value::<SchemaArguments>(tool_params.arguments) {
                                    Ok(schema_args) => {
                                        get_schema(request.id, schema_args.table_name, current_pool).await
                                    }
                                    Err(e) => JsonRpcResponse {
                                        jsonrpc: "2.0".to_string(),
                                        id: request.id,
                                        result: None,
                                        error: Some(JsonRpcError {
                                            code: -32602,
                                            message: format!("Invalid query arguments: {e}"),
                                            data: None,
                                        }),
                                    },
                                }
                            }
                            "query" => {
                                match serde_json::from_value::<QueryArguments>(tool_params.arguments) {
                                    Ok(query_args) => {
                                        execute_query(request.id.clone().unwrap_or(json!(null)), query_args.query, query_args.database, current_pool, allow_dangerous_queries).await
                                    }
                                    Err(e) => JsonRpcResponse {
                                        jsonrpc: "2.0".to_string(),
                                        id: request.id,
                                        result: None,
                                        error: Some(JsonRpcError {
                                            code: -32602,
                                            message: format!("Invalid query arguments: {e}"),
                                            data: None,
                                        }),
                                    },
                                }
                            }
                            "insert" => {
                                match serde_json::from_value::<InsertArguments>(tool_params.arguments) {
                                    Ok(insert_args) => {
                                        insert_data(request.id.clone().unwrap_or(json!(null)), insert_args.table_name, insert_args.data, current_pool).await
                                    }
                                    Err(e) => JsonRpcResponse {
                                        jsonrpc: "2.0".to_string(),
                                        id: request.id,
                                        result: None,
                                        error: Some(JsonRpcError {
                                            code: -32602,
                                            message: format!("Invalid insert arguments: {e}"),
                                            data: None,
                                        }),
                                    },
                                }
                            }
                            "update" => {
                                match serde_json::from_value::<UpdateArguments>(tool_params.arguments) {
                                    Ok(update_args) => {
                                        update_data(request.id.clone().unwrap_or(json!(null)), update_args.table_name, update_args.data, update_args.conditions, current_pool).await
                                    }
                                    Err(e) => JsonRpcResponse {
                                        jsonrpc: "2.0".to_string(),
                                        id: request.id,
                                        result: None,
                                        error: Some(JsonRpcError {
                                            code: -32602,
                                            message: format!("Invalid update arguments: {e}"),
                                            data: None,
                                        }),
                                    },
                                }
                            }
                            "delete" => {
                                match serde_json::from_value::<DeleteArguments>(tool_params.arguments) {
                                    Ok(delete_args) => {
                                        delete_data(request.id.clone().unwrap_or(json!(null)), delete_args.table_name, delete_args.conditions, current_pool).await
                                    }
                                    Err(e) => JsonRpcResponse {
                                        jsonrpc: "2.0".to_string(),
                                        id: request.id,
                                        result: None,
                                        error: Some(JsonRpcError {
                                            code: -32602,
                                            message: format!("Invalid delete arguments: {e}"),
                                            data: None,
                                        }),
                                    },
                                }
                            }
                            _ => JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: request.id,
                                result: None,
                                error: Some(JsonRpcError {
                                    code: -32601,
                                    message: format!("Unknown tool: {}", tool_params.name),
                                    data: None,
                                }),
                            }
                        }
                    }
                    Err(e) => JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32602,
                            message: format!("Invalid tool call parameters: {e}"),
                            data: None,
                        }),
                    },
                },
                None => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: "Missing parameters".to_string(),
                        data: None,
                    }),
                },
            }
        }
        _ => {
            warn!("Unknown method: {}", request.method);
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", request.method),
                    data: None,
                }),
            }
        }
    }
}
