use anyhow::{Context, Result};
use aws_sdk_s3::config::Credentials;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde_aux::prelude::*;
use sqlx::postgres::{PgConnectOptions, PgSslMode};

#[derive(Deserialize)]
pub struct Config {
    pub application: ApplicationConfig,
    pub database: DatabaseConfig,
    pub rabbitmq: RabbitMQConfig,
    pub minio: MinioConfig,
}

#[derive(Deserialize)]
pub struct ApplicationConfig {
    pub name: String,
    pub version: String,
    pub host: String,
    pub port: u16,
}

#[derive(Deserialize)]
pub struct DatabaseConfig {
    pub host: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub username: SecretString,
    pub password: SecretString,
    pub database_name: String,
    pub require_ssl: bool,
}

#[derive(Deserialize)]
pub struct RabbitMQConfig {
    pub url: String,
    pub exchange_name: String,
    pub queues: Vec<String>,
    pub registry_queues: Vec<(String, String)>,
    pub queue_consumer: String,
}

#[derive(Deserialize)]
pub struct MinioConfig {
    pub url: String,
    pub username: SecretString,
    pub password: SecretString,
    pub bucket_name: String,
}

impl MinioConfig {
    pub fn credentials(&self) -> Credentials {
        Credentials::new(
            self.username.expose_secret(),
            self.password.expose_secret(),
            None,
            None,
            "minio0",
        )
    }
}

impl DatabaseConfig {
    pub fn connect_options(&self) -> PgConnectOptions {
        let ssl_mode = if self.require_ssl {
            PgSslMode::Require
        } else {
            PgSslMode::Prefer
        };

        PgConnectOptions::new()
            .host(&self.host)
            .port(self.port)
            .username(self.username.expose_secret())
            .password(self.password.expose_secret())
            .database(&self.database_name)
            .ssl_mode(ssl_mode)
    }

    pub fn connect_options_root(&self) -> PgConnectOptions {
        PgConnectOptions::new()
            .host(&self.host)
            .port(self.port)
            .username(self.username.expose_secret())
            .password(self.password.expose_secret())
    }
}

impl Config {
    pub fn build() -> Result<Self> {
        let base_path = std::env::current_dir().context("Failed to determine current directory")?;
        let configuration_directory = base_path.join("configs");

        let environment: Environment = std::env::var("APP_ENVIRONMENT")
            .unwrap_or_else(|_| "dev".into())
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

        if let Some(host) = get_env_var("POSTGRES_HOST") {
            settings = settings.set_override("database.host", host)?;
        }
        if let Some(port) = get_env_var("POSTGRES_PORT") {
            settings = settings.set_override("database.port", port)?;
        }
        if let Some(username) = get_env_var("POSTGRES_USER") {
            settings = settings.set_override("database.username", username)?;
        }
        if let Some(password) = get_env_var("POSTGRES_PASSWORD") {
            settings = settings.set_override("database.password", password)?;
        }
        if let Some(database_name) = get_env_var("POSTGRES_DB") {
            settings = settings.set_override("database.database_name", database_name)?;
        }
        if let Some(require_ssl) = get_env_var("POSTGRES_REQUIRE_SSL") {
            settings = settings.set_override("database.require_ssl", require_ssl)?;
        }
        if let Some(url) = get_env_var("RABBITMQ_URL") {
            settings = settings.set_override("rabbitmq.url", url)?;
        }
        if let Some(exchange_name) = get_env_var("RABBITMQ_EXCHANGE_NAME") {
            settings = settings.set_override("rabbitmq.exchange_name", exchange_name)?;
        }

        let settings = settings.build().context("Failed to build configuration")?;

        settings
            .try_deserialize::<Config>()
            .context("Failed to deserialize configuration")
    }
}

fn get_env_var(name: &str) -> Option<String> {
    let var = std::env::var(name).ok()?;
    if var.is_empty() {
        return None;
    }
    Some(var)
}

pub enum Environment {
    Development,
    Production,
    Staging,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Development => "dev",
            Environment::Production => "production",
            Environment::Staging => "staging",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "dev" => Ok(Environment::Development),
            "production" => Ok(Environment::Production),
            "staging" => Ok(Environment::Staging),
            other => Err(format!("{} is not a valid environment", other)),
        }
    }
}
