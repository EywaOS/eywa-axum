//! Unified configuration loading for EYWA services.

use config::{Config, Environment, File, FileFormat};
use serde::Deserialize;
use tracing::info;

use crate::Result;

/// Wrapper for service configuration.
///
/// Automatically loads configuration from:
/// 1. `config/default.toml`
/// 2. `config/{env}.toml` (where {env} is RUN_MODE, defaults to "development")
/// 3. Environment variables (prefixed with APP_)
/// 4. `.env` file
pub struct EywaConfig;

impl EywaConfig {
    /// Load configuration into a struct that implements `Deserialize`.
    pub fn load<T>() -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        // Load .env file if it exists
        dotenvy::dotenv().ok();

        let run_mode = std::env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
        info!("Loading configuration for environment: {}", run_mode);

        let settings = Config::builder()
            // Start with default defaults
            .add_source(File::new("config/default", FileFormat::Toml).required(false))
            // Add environment specific config
            .add_source(
                File::new(&format!("config/{}", run_mode), FileFormat::Toml).required(false),
            )
            // Add local config (gitignored)
            .add_source(File::new("config/local", FileFormat::Toml).required(false))
            // Add environment variables
            .add_source(Environment::default().separator("__"))
            .build()
            .map_err(|e| eywa_errors::AppError::ConfigError(e.to_string()))?;

        settings
            .try_deserialize()
            .map_err(|e| eywa_errors::AppError::ConfigError(e.to_string()))
    }
}
