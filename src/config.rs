//! Defines the configuration required to start the server application..
use anyhow::Result;
use serde::Deserialize;
use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::sqlite::SqliteConnectOptions;

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    pub database: DatabaseConfig,
    pub application: ApplicationConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DatabaseConfig {
    pub uri: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApplicationConfig {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
}

/// The possible runtime environment for our application.
pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a supported environment. Use either `local` or `production`.",
                other
            )),
        }
    }
}

pub fn get_config() -> Result<Config, config::ConfigError> {
    let base_path = std::env::current_dir().expect("Unable to determine current directory");
    let config_dir = base_path.join("config");

    // Detect the running environment, default to local if unspecified.
    let env: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT.");

    let config = config::Config::builder()
        // Read the default config
        .add_source(config::File::from(config_dir.join("base")).required(true))
        // Add in the current environment file
        .add_source(config::File::from(config_dir.join(env.as_str())))
        .add_source(
            config::Environment::with_prefix("challenges")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;

    config.try_deserialize()
}

impl DatabaseConfig {
    pub fn with_db(&self) -> SqliteConnectOptions {
        SqliteConnectOptions::new()
            .filename(&self.uri)
            .create_if_missing(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn get_config_env_vars() -> Result<()> {
        std::env::set_var("CHALLENGES_APPLICATION__HOST", "127.0.0.1");
        std::env::set_var("CHALLENGES_APPLICATION__PORT", "8888");
        let config = get_config()?;
        assert_eq!(config.application.host, "127.0.0.1");
        assert_eq!(config.application.port, 8888);

        std::env::set_var("CHALLENGES_DATABASE__URI", "test");
        let config = get_config()?;
        assert_eq!(config.database.uri, "test");
        Ok(())
    }
}
