use clap::Parser;

// Command line arguments
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// MySQL host
    #[arg(long, default_value = "localhost")]
    pub host: String,
    
    /// MySQL port
    #[arg(long, default_value = "3306")]
    pub port: u16,
    
    /// MySQL username
    #[arg(long)]
    pub username: String,
    
    /// MySQL password
    #[arg(long, default_value = "")]
    pub password: String,
    
    /// MySQL database name
    #[arg(long)]
    pub database: String,
    
    /// Allow dangerous SQL keywords in queries (INSERT, UPDATE, DELETE, etc.)
    #[arg(long, default_value = "false")]
    pub allow_dangerous_queries: bool,
}
