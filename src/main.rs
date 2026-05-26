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
mod error;
mod rpc;
mod server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "--help" | "-h" => {
                print_help(&args[0]);
                return Ok(());
            }
            "--version" | "-V" => {
                println!("mcp-server-mysql {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => {}
        }
    }

    let config = config::load()?;
    server::run(config).await
}

fn print_help(program: &str) {
    println!(
        "MCP MySQL Server — Model Context Protocol server for MySQL/MariaDB\n\n\
         Usage:\n  {program}                    Run using config from ~/.config/mcp-server-mysql/config.toml\n\
         {program} --help               Show this help\n\
         {program} --version            Print version\n\n\
         The server communicates over stdio using JSON-RPC 2.0 (for MCP clients).\n\
         Configuration (TOML):\n\
           host = \"localhost\"\n\
           port = 3306\n\
           username = \"admin\"\n\
           password = \"\"\n\
           database = \"mydb\"            # optional; omit or leave empty to connect without a default DB\n\
           allow_dangerous_queries = false\n\
           max_rows = 1000\n"
    );
}
