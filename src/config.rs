use anyhow::{Context, Result};
use serde::Deserialize;
use serde_aux::prelude::*;
use sqlx::postgres::{PgConnectOptions, PgSslMode};

#[derive(Deserialize)]
pub struct Settings {
    pub application: ApplicationSettings,
    pub database: DatabaseSettings,
}

#[derive(Deserialize)]
pub struct ApplicationSettings {
    pub name: String,
    pub version: String,
    pub host: String,
    pub port: u16,
}

#[derive(Deserialize)]
pub struct DatabaseSettings {
    pub host: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database_name: String,
    pub require_ssl: bool,
}

impl DatabaseSettings {
    pub fn connect_options(&self) -> PgConnectOptions {
        let ssl_mode = if self.require_ssl {
            PgSslMode::Require
        } else {
            PgSslMode::Prefer
        };

        PgConnectOptions::new()
            .host(&self.host)
            .port(self.port)
            .username(&self.username)
            .password(&self.password)
            .database(&self.database_name)
            .ssl_mode(ssl_mode)
    }

    pub fn connect_options_root(&self) -> PgConnectOptions {
        PgConnectOptions::new()
            .host(&self.host)
            .port(self.port)
            .username(&self.username)
            .password(&self.password)
    }
}

impl Settings {
    pub fn build() -> Result<Self> {
        let base_path = std::env::current_dir().context("Failed to determine current directory")?;
        let configuration_directory = base_path.join("configs");

        let environment: Environment = std::env::var("APP_ENVIRONMENT")
            .unwrap_or_else(|_| "local".into())
            .try_into()
            .expect("Failed to parse APP_ENVIRONMENT");
        let environment_filename = format!("{}.toml", environment.as_str());

        let mut settings = config::Config::builder()
            .add_source(config::File::from(
                configuration_directory.join("base.toml"),
            ))
            .add_source(config::File::from(
                configuration_directory.join(environment_filename),
            ))
            .add_source(
                config::Environment::with_prefix("APP")
                    .prefix_separator("_")
                    .separator("__"),
            )
            .set_override("application.name", env!("CARGO_PKG_NAME"))?
            .set_override("application.version", env!("CARGO_PKG_VERSION"))?;

        if let Ok(host) = std::env::var("POSTGRES_HOST") {
            settings = settings.set_override("database.host", host)?;
        }
        if let Ok(port) = std::env::var("POSTGRES_PORT") {
            settings = settings.set_override("database.port", port)?;
        }
        if let Ok(username) = std::env::var("POSTGRES_USER") {
            settings = settings.set_override("database.username", username)?;
        }
        if let Ok(password) = std::env::var("POSTGRES_PASSWORD") {
            settings = settings.set_override("database.password", password)?;
        }
        if let Ok(database_name) = std::env::var("POSTGRES_DB") {
            settings = settings.set_override("database.database_name", database_name)?;
        }
        if let Ok(require_ssl) = std::env::var("POSTGRES_REQUIRE_SSL") {
            settings = settings.set_override("database.require_ssl", require_ssl)?;
        }

        let settings = settings.build().context("Failed to build configuration")?;

        settings
            .try_deserialize::<Settings>()
            .context("Failed to deserialize configuration")
    }
}

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

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "local" => Ok(Environment::Local),
            "production" => Ok(Environment::Production),
            other => Err(format!("{} is not a valid environment", other)),
        }
    }
}
