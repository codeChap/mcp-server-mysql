use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub initialization_options: Option<InitializationOptions>,
}

#[derive(Debug, Deserialize)]
pub struct InitializationOptions {
    pub settings: Option<ServerSettings>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerSettings {
    pub database_url: Option<String>,
}

// MCP specific structures
#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
}

#[derive(Debug, Serialize)]
pub struct ServerCapabilities {
    pub tools: Option<ToolsCapability>,
}

#[derive(Debug, Serialize)]
pub struct ToolsCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

#[derive(Debug, Serialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Serialize)]
pub struct ToolsList {
    pub tools: Vec<Tool>,
}

#[derive(Debug, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Deserialize)]
pub struct SchemaArguments {
    pub table_name: String,
}

#[derive(Debug, Deserialize)]
pub struct QueryArguments {
    pub query: String,
    pub database: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct InsertArguments {
    pub table_name: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct UpdateArguments {
    pub table_name: String,
    pub data: serde_json::Value,
    pub conditions: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct DeleteArguments {
    pub table_name: String,
    pub conditions: serde_json::Value,
}
