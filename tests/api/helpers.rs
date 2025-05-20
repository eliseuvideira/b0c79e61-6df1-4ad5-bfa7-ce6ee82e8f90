use anyhow::{Context, Result};
use integrations_api::{
    app::Application,
    config::{DatabaseSettings, Settings},
};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle, PrometheusRecorder};
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

pub fn init_metrics() -> (PrometheusHandle, PrometheusRecorder) {
    let builder = PrometheusBuilder::new();
    let recorder = builder.build_recorder();
    let handle = recorder.handle();

    (handle, recorder)
}

pub async fn spawn_app() -> Result<TestApp> {
    dotenvy::dotenv().ok();

    let configuration = {
        let mut configuration = Settings::build()?;
        configuration.database.database_name = Uuid::new_v4().to_string();
        configuration.application.host = "127.0.0.1".to_string();
        configuration.application.port = 0;
        configuration
    };

    let db_pool = configure_database(&configuration.database).await?;

    let (metrics_handle, _) = init_metrics();

    let application = Application::build(configuration, metrics_handle)
        .await
        .expect("Failed to build application.");
    let port = application.port();
    let address = format!("http://127.0.0.1:{}", port);
    let _ = tokio::spawn(application.run_until_stopped());

    Ok(TestApp { address, db_pool })
}

async fn configure_database(config: &DatabaseSettings) -> Result<PgPool> {
    let mut connection = PgConnection::connect_with(&config.connect_options_root())
        .await
        .context(format!(
            "Failed to connect to Postgres with username: {} and password: {} and host: {} and port: {}",
            config.username, config.password, config.host, config.port
        ))?;

    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await?;

    let db_pool = PgPool::connect_with(config.connect_options())
        .await
        .context("Failed to connect to Postgres.")?;

    sqlx::migrate!("./migrations").run(&db_pool).await?;

    Ok(db_pool)
}
