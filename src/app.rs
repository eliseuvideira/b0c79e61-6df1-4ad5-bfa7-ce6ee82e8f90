use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use tokio::try_join;

use crate::{
    api::Api,
    config::{Config, DatabaseConfig},
    services::{minio, rabbitmq},
    telemetry::Metrics,
    worker::Worker,
};

pub struct Application {
    pub api: Api,
    pub worker: Worker,
}

impl Application {
    pub async fn build(configuration: Config, metrics: Metrics) -> Result<Self> {
        let db_pool = get_db_pool(&configuration.database);

        let rabbitmq_connection = Arc::new(rabbitmq::connect(&configuration.rabbitmq).await?);
        let channel = rabbitmq_connection.create_channel().await?;

        let all_queues: Vec<&str> = configuration
            .rabbitmq
            .queues
            .iter()
            .map(|s| s.as_str())
            .chain(std::iter::once(
                configuration.rabbitmq.queue_consumer.as_str(),
            ))
            .collect();

        rabbitmq::declare_exchange(&channel, &configuration.rabbitmq.exchange_name).await?;
        rabbitmq::declare_and_bind_queues(
            &channel,
            &all_queues,
            &configuration.rabbitmq.exchange_name,
        )
        .await?;

        let queue_consumer = configuration.rabbitmq.queue_consumer.clone();

        let minio_client = minio::create_client(&configuration.minio).await?;

        minio::ensure_bucket(&minio_client, &configuration.minio.bucket_name).await?;

        let integration_queues: HashMap<String, String> = configuration
            .rabbitmq
            .registry_queues
            .iter()
            .cloned()
            .collect();

        let worker = Worker::build(
            rabbitmq_connection.clone(),
            queue_consumer.clone(),
            minio_client.clone(),
            configuration.minio.bucket_name.clone(),
            db_pool.clone(),
        )
        .await?;

        let metrics = Arc::new(metrics);

        let api = Api::build(
            &configuration,
            db_pool,
            rabbitmq_connection.clone(),
            integration_queues,
            metrics,
        )
        .await?;

        Ok(Self { api, worker })
    }

    pub async fn run_until_stopped(self) -> Result<()> {
        try_join!(
            self.worker.run_until_stopped(),
            self.api.run_until_stopped()
        )?;

        Ok(())
    }
}

pub fn get_db_pool(settings: &DatabaseConfig) -> Pool<Postgres> {
    PgPoolOptions::new().connect_lazy_with(settings.connect_options())
}
