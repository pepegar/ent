use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize)]
pub struct JwtConfig {
    pub public_key_path: String,
    pub issuer: String,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: u32,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub timeout_seconds: u64,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub jwt: JwtConfig,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        Self::new_from_folder(".".into())
    }

    pub fn new_from_folder(prefix: String) -> Result<Self, ConfigError> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

        let s = Config::builder()
            // Start with default settings
            .add_source(File::with_name(&format!("{}/config/default", prefix)).required(true))
            // Add environment specific settings
            .add_source(File::with_name(&format!("{}/config/{}", prefix, run_mode)).required(false))
            // Add local overrides
            .add_source(File::with_name(&format!("{}/config/local", prefix)).required(false))
            // Add environment variables with prefix "ENT_"
            .add_source(Environment::with_prefix("ENT").separator("_"))
            .build()?;

        s.try_deserialize()
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
}
