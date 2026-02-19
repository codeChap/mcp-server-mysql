use serde::Deserialize;
use std::path::PathBuf;

fn default_host() -> String {
    "localhost".into()
}

fn default_port() -> u16 {
    3306
}

fn default_password() -> String {
    String::new()
}

fn default_max_rows() -> usize {
    1000
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,

    pub username: String,

    #[serde(default = "default_password")]
    pub password: String,

    pub database: String,

    #[serde(default)]
    pub allow_dangerous_queries: bool,

    #[serde(default = "default_max_rows")]
    pub max_rows: usize,
}

pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(home)
        .join(".config")
        .join("mcp-server-mysql")
        .join("config.toml")
}

pub fn load() -> Result<Config, Box<dyn std::error::Error>> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "Failed to read config file: {}\n\
             Create it with your MySQL connection settings.\n\
             Example:\n\n\
             host = \"localhost\"\n\
             port = 3306\n\
             username = \"admin\"\n\
             password = \"\"\n\
             database = \"mydb\"\n\
             allow_dangerous_queries = false\n\
             max_rows = 1000\n\n\
             Error: {e}",
            path.display()
        )
    })?;
    let config: Config = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse {}: {e}", path.display()))?;
    Ok(config)
}
