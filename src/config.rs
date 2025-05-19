use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Settings {
    pub application: ApplicationSettings,
}

#[derive(Deserialize)]
pub struct ApplicationSettings {
    pub name: String,
    pub version: String,
    pub host: String,
    pub port: u16,
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

        let settings = config::Config::builder()
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
            .set_override("application.version", env!("CARGO_PKG_VERSION"))?
            .build()
            .context("Failed to build configuration")?;

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
