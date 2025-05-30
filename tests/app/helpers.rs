use std::collections::HashMap;

use anyhow::{Context, Result};
use fake::{faker::name::en::Name, Fake};
use integrations_api::{
    api::types::ApiResponse,
    app::Application,
    config::{Config, DatabaseConfig},
    models::job::Job,
    services::rabbitmq,
    telemetry::Metrics,
};
use lapin::Channel;
use reqwest::Client;
use serde_json::json;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
    pub channel: Channel,
    pub integration_queues: HashMap<String, String>,
}

impl TestApp {
    pub fn registry_queue(&self) -> Result<(String, String)> {
        let (registry, queue) = self
            .integration_queues
            .iter()
            .next()
            .ok_or(anyhow::anyhow!("No registry queue"))?;
        Ok((registry.clone(), queue.clone()))
    }

    pub async fn mock_create_job(
        &self,
        client: &Client,
        registry: &str,
    ) -> Result<ApiResponse<Job>> {
        let package_name: String = Name().fake();
        let url = format!("{}/jobs", self.address);
        let response = client
            .post(url)
            .json(&json!({
                "registry": registry,
                "package_name": package_name,
            }))
            .send()
            .await?;

        let body: ApiResponse<Job> = response.json().await?;

        Ok(body)
    }

    pub async fn mock_create_jobs(
        &self,
        client: &Client,
        registry: &str,
        count: u64,
    ) -> Result<Vec<ApiResponse<Job>>> {
        let mut jobs = Vec::new();
        for _ in 0..count {
            let response = self.mock_create_job(client, registry).await?;
            jobs.push(response);
        }

        Ok(jobs)
    }
}

pub async fn spawn_app() -> Result<TestApp> {
    dotenvy::dotenv().ok();

    let exchange_name = Uuid::new_v4().to_string();
    let queues = vec![Uuid::new_v4().to_string()];
    let registry_queues: Vec<(String, String)> = queues
        .clone()
        .into_iter()
        .map(|queue| (Uuid::new_v4().to_string(), queue))
        .collect();
    let queue_consumer = Uuid::new_v4().to_string();
    let configuration = {
        let mut configuration = Config::build()?;
        configuration.database.database_name = Uuid::new_v4().to_string();
        configuration.application.host = "127.0.0.1".to_string();
        configuration.application.port = 0;
        configuration.rabbitmq.exchange_name = exchange_name.clone();
        configuration.rabbitmq.queues = queues;
        configuration.rabbitmq.queue_consumer = queue_consumer.clone();
        configuration.rabbitmq.registry_queues = registry_queues.clone();
        configuration.minio.bucket_name = Uuid::new_v4().to_string();
        configuration
    };

    let db_pool = configure_database(&configuration.database).await?;

    let rabbitmq_connection = rabbitmq::connect(&configuration.rabbitmq).await?;
    let channel = rabbitmq_connection.create_channel().await?;

    let integration_queues: HashMap<String, String> = registry_queues.into_iter().collect();

    let metrics = Metrics::build()?;
    let application = Application::build(configuration, metrics)
        .await
        .context("Failed to build application.")?;
    let port = application.api.port();
    let address = format!("http://127.0.0.1:{}", port);
    let _ = tokio::spawn(application.run_until_stopped());

    Ok(TestApp {
        address,
        db_pool,
        channel,
        integration_queues,
    })
}

async fn configure_database(config: &DatabaseConfig) -> Result<PgPool> {
    let mut connection = PgConnection::connect_with(&config.connect_options_root())
        .await
        .context("Failed to connect to Postgres.")?;

    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await?;

    let db_pool = PgPool::connect_with(config.connect_options())
        .await
        .context("Failed to connect to Postgres pool.")?;

    sqlx::migrate!("./migrations").run(&db_pool).await?;

    Ok(db_pool)
}
