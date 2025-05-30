use std::sync::Arc;

use anyhow::Result;
use aws_sdk_s3::Client;
use lapin::{
    message::DeliveryResult,
    options::{BasicAckOptions, BasicNackOptions},
    types::{AMQPValue, FieldTable, ShortString},
    Connection,
};
use opentelemetry::{global, propagation::Extractor};
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use tracing::{info_span, instrument, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

use crate::{
    db,
    models::package::Package,
    services::rabbitmq,
    types::{self, JobMessage},
};

pub struct Worker {
    rabbitmq_connection: Arc<Connection>,
    consumer_queue: String,
    minio_client: Client,
    bucket_name: Arc<String>,
    db_pool: Pool<Postgres>,
}

impl Worker {
    pub async fn build(
        rabbitmq_connection: Arc<Connection>,
        consumer_queue: String,
        minio_client: Client,
        bucket_name: String,
        db_pool: Pool<Postgres>,
    ) -> Result<Self> {
        Ok(Self {
            rabbitmq_connection,
            consumer_queue,
            minio_client,
            bucket_name: Arc::new(bucket_name),
            db_pool,
        })
    }

    pub async fn run_until_stopped(self) -> Result<()> {
        let channel = self.rabbitmq_connection.create_channel().await?;
        let consumer = rabbitmq::create_consumer(&channel, &self.consumer_queue).await?;

        consumer.set_delegate(move |delivery: DeliveryResult| {
            let minio_client = self.minio_client.clone();
            let db_pool = self.db_pool.clone();
            let bucket_name = self.bucket_name.clone();

            async move {
                match delivery {
                    Ok(Some(delivery)) => {
                        match parse_and_run_consume(
                            &delivery.data,
                            delivery.properties.headers(),
                            minio_client,
                            &bucket_name,
                            db_pool,
                        )
                        .await
                        {
                            Ok(_) => delivery
                                .ack(BasicAckOptions::default())
                                .await
                                .expect("Failed to ack message"),
                            Err(err) => {
                                tracing::error!("Failed to process message: {:?}", err);
                                delivery
                                    .nack(BasicNackOptions {
                                        multiple: false,
                                        requeue: false,
                                    })
                                    .await
                                    .expect("Failed to nack message");
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        tracing::error!("Failed to consume queue message {}:", error);
                    }
                }
            }
        });

        std::future::pending::<()>().await;

        Ok(())
    }
}

async fn parse_and_run_consume(
    data: &[u8],
    headers: &Option<FieldTable>,
    minio_client: Client,
    bucket_name: &str,
    db_pool: Pool<Postgres>,
) -> Result<()> {
    let message = serde_json::from_slice::<types::JobMessage>(data)?;
    let span = if let Some(headers) = headers {
        let extractor = FieldTableExtractor(headers);
        let context = global::get_text_map_propagator(|prop| prop.extract(&extractor));

        let span = info_span!("consumer");
        span.set_parent(context);
        span
    } else {
        info_span!("consumer")
    };
    let _ = span.enter();

    consume_message(message, minio_client, bucket_name, db_pool)
        .instrument(span)
        .await
}

pub struct FieldTableExtractor<'a>(&'a FieldTable);

impl<'a> Extractor for FieldTableExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        let key = ShortString::from(key.to_string());
        self.0.inner().get(&key).and_then(|value| match value {
            AMQPValue::LongString(s) => std::str::from_utf8(s.as_bytes()).ok(),
            _ => None,
        })
    }

    fn keys(&self) -> Vec<&str> {
        self.0.inner().keys().map(|k| k.as_str()).collect()
    }
}

#[instrument(name = "consume_message", skip_all)]
pub async fn consume_message(
    message: JobMessage,
    minio_client: Client,
    bucket_name: &str,
    db_pool: Pool<Postgres>,
) -> Result<()> {
    let response = minio_client
        .get_object()
        .bucket(bucket_name)
        .key(format!("outputs/{}.json", message.package_name))
        .send()
        .await?;

    let data = response.body.collect().await?;
    let json_data = serde_json::from_slice::<PackageOutput>(&data.into_bytes())?;

    let mut transaction = db_pool.begin().await?;

    let package = Package {
        id: json_data.id,
        registry: json_data.registry,
        name: json_data.name,
        version: json_data.version,
        downloads: json_data.downloads as i64,
    };

    db::upsert_package(&mut transaction, package).await?;
    db::complete_job(&mut transaction, message.job_id).await?;

    transaction.commit().await?;

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct PackageOutput {
    id: Uuid,
    registry: String,
    name: String,
    version: String,
    downloads: u64,
}
