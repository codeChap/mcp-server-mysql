//! MySQL MCP Server
//!
//! A Model Context Protocol (MCP) server implementation for MySQL databases.
//! This server enables AI assistants to interact with MySQL databases through
//! a standardized protocol using JSON-RPC 2.0 over stdio.
//!
//! # Features
//!
//! - Schema inspection and introspection
//! - SQL query execution (with safety controls)
//! - Data manipulation (INSERT, UPDATE, DELETE)
//! - Robust error handling and logging
//! - Configurable security settings

mod config;
mod db;
mod rpc;
mod server;

use clap::Parser;
use config::Args;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger
    env_logger::init();
    
    let args = Args::parse();
    server::run(args).await
}
