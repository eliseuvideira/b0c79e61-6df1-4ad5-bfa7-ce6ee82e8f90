use std::sync::Arc;

use anyhow::{Context, Result};
use aws_sdk_s3::Client as MinioClient;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::{from_fn, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post, Router},
    serve::Serve,
    Json,
};
use axum_tracing_opentelemetry::{
    middleware::{OtelAxumLayer, OtelInResponseLayer},
    tracing_opentelemetry_instrumentation_sdk::find_current_trace_id,
};
use futures_lite::stream::StreamExt;
use lapin::{
    message::Delivery,
    options::{BasicAckOptions, BasicNackOptions},
    Channel,
};
use metrics_exporter_prometheus::PrometheusHandle;
use opentelemetry::{
    global::{self},
    propagation::Extractor,
};
use scalar_doc::Documentation;
use serde::{Deserialize, Serialize};
use sqlx::{
    postgres::PgPoolOptions,
    types::chrono::{DateTime, Utc},
    Executor, Pool, Postgres, Transaction,
};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{debug_span, info_span, instrument, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

use crate::{
    config::{DatabaseSettings, Settings},
    error::Error,
    minio, rabbitmq,
};

pub struct Application {
    port: u16,
    server: Serve<Router, Router>,
    minio_client: MinioClient,
    db_pool: Pool<Postgres>,
    channel: Channel,
}

#[derive(Debug)]
struct AppState {
    db: Pool<Postgres>,
    channel: Channel,
    exchange_name: String,
}

impl Application {
    pub async fn build(configuration: Settings, metrics_handle: PrometheusHandle) -> Result<Self> {
        let db_pool = get_db_pool(&configuration.database);

        let (_, channel) = rabbitmq::connect(&configuration.rabbitmq).await?;

        rabbitmq::declare_exchange(&channel, "default_exchange").await?;

        let minio_client = minio::create_minio_client(&configuration.minio).await?;

        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );
        let listener = TcpListener::bind(&address)
            .await
            .context("Failed to bind address")?;
        let port = listener
            .local_addr()
            .context("Failed to get local address")?
            .port();

        let app_state = AppState {
            db: db_pool.clone(),
            channel: channel.clone(),
            exchange_name: configuration.rabbitmq.exchange_name,
        };
        let app_state = Arc::new(app_state);

        let prometheus_layer = axum_prometheus::PrometheusMetricLayer::default();

        let router = Router::new()
            .route("/scrapper-jobs", post(create_scrapper_job))
            .route("/", get(index))
            .route("/openapi", get(openapi))
            .layer(TraceLayer::new_for_http())
            .layer(from_fn(attach_trace_id))
            .layer(OtelInResponseLayer)
            .layer(OtelAxumLayer::default())
            .layer(prometheus_layer)
            .with_state(app_state)
            .route(
                "/metrics",
                get(move || std::future::ready(metrics_handle.render())),
            )
            .route("/health", get(health_check))
            .fallback(not_found);

        let server = axum::serve(listener, router);

        Ok(Self {
            port,
            server,
            minio_client,
            db_pool,
            channel,
        })
    }

    pub async fn run_until_stopped(self) -> Result<()> {
        let _ = tokio::spawn(async move {
            let mut consumer = rabbitmq::create_consumer(&self.channel, "output_parser")
                .await
                .expect("Cannot create consumer");

            while let Some(Ok(delivery)) = consumer.next().await {
                handle_new_message(&delivery, self.minio_client.clone(), self.db_pool.clone())
                    .await;
            }
        });

        self.server.await?;

        Ok(())
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

async fn health_check() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

async fn attach_trace_id(req: Request, next: Next) -> Response {
    let trace_id = find_current_trace_id();

    next.run(req)
        .instrument(debug_span!(
            "trace_id",
            trace_id = ?trace_id,
        ))
        .await
}

#[derive(Debug, Deserialize)]
struct CreateScrapperJobPayload {
    registry_name: String,
    package_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ScrapperJobMessage {
    job_id: Uuid,
    package_name: String,
}

#[instrument(name = "create_scrapper_job")]
async fn create_scrapper_job(
    State(app_state): State<Arc<AppState>>,
    Json(payload): Json<CreateScrapperJobPayload>,
) -> Result<impl IntoResponse, Error> {
    let id = Uuid::now_v7();
    let registry_name = payload.registry_name;
    let package_name = payload.package_name;
    let trace_id = find_current_trace_id();

    let mut transaction = app_state.db.begin().await?;

    let scrapper_job = insert_scrapper_job(
        &mut transaction,
        CreateScrapperJob {
            id,
            registry_name,
            package_name,
            trace_id: trace_id.clone(),
            created_at: Utc::now(),
        },
    )
    .await?;

    transaction.commit().await?;

    let message = ScrapperJobMessage {
        job_id: scrapper_job.id,
        package_name: scrapper_job.package_name.clone(),
    };

    rabbitmq::publish_message(
        &app_state.channel,
        &app_state.exchange_name,
        &scrapper_job.registry_name,
        &message,
    )
    .await?;

    Ok((StatusCode::CREATED, Json(scrapper_job)))
}

#[derive(Debug)]
struct CreateScrapperJob {
    id: Uuid,
    registry_name: String,
    package_name: String,
    trace_id: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ScrapperJob {
    id: Uuid,
    registry_name: String,
    package_name: String,
    status: String,
    trace_id: Option<String>,
    created_at: DateTime<Utc>,
}

#[instrument(name = "insert_scrapper_job", skip(transaction))]
async fn insert_scrapper_job(
    transaction: &mut Transaction<'static, Postgres>,
    create_scrapper_job: CreateScrapperJob,
) -> Result<ScrapperJob> {
    let query = sqlx::query!(
        r#"INSERT INTO scrapper_jobs (id, registry_name, package_name, status, trace_id, created_at)
        VALUES ($1, $2, $3, 'processing', $4, $5);
        "#,
        create_scrapper_job.id,
        create_scrapper_job.registry_name,
        create_scrapper_job.package_name,
        create_scrapper_job.trace_id,
        create_scrapper_job.created_at,
    );

    let result = transaction
        .execute(query)
        .instrument(instrument_query(
            "INSERT",
            "INSERT.integrations.scrapper_jobs",
        ))
        .await?;

    let rows_affected = result.rows_affected();
    if rows_affected != 1 {
        anyhow::bail!("Expected 1 row to be affected, got {}", rows_affected);
    }

    let scrapper_job = sqlx::query_as!(
        ScrapperJob,
        r#"SELECT id, registry_name, package_name, status, trace_id, created_at FROM scrapper_jobs WHERE id = $1"#,
        create_scrapper_job.id
    ).fetch_one(&mut **transaction).await?;

    Ok(scrapper_job)
}

fn instrument_query(operation: &str, name: &str) -> tracing::Span {
    debug_span!(
        "db_query",
        db.system = "postgres",
        db.operation = operation,
        otel.name = name,
        otel.kind = "CLIENT",
        otel.status_code = tracing::field::Empty,
    )
}

pub fn get_db_pool(settings: &DatabaseSettings) -> Pool<Postgres> {
    PgPoolOptions::new().connect_lazy_with(settings.connect_options())
}

#[derive(Debug, Deserialize)]
pub struct PackageOutput {
    id: String,
    name: String,
    version: String,
    downloads: u64,
}

#[instrument(name = "consume_message", skip_all)]
pub async fn consume_message(
    delivery: &Delivery,
    minio_client: MinioClient,
    db_pool: Pool<Postgres>,
) -> Result<()> {
    let message = serde_json::from_slice::<ScrapperJobMessage>(&delivery.data)?;

    let response = minio_client
        .get_object()
        .bucket("outputs")
        .key(&format!("outputs/{}.json", message.package_name))
        .send()
        .await?;

    let data = response.body.collect().await?;
    let json_data = serde_json::from_slice::<PackageOutput>(&data.into_bytes())?;

    let mut transaction = db_pool.begin().await?;

    let package = sqlx::query!(
        r#"SELECT id FROM packages WHERE id = $1 FOR UPDATE;"#,
        json_data.id
    )
    .fetch_optional(&mut *transaction)
    .await?;

    if package.is_some() {
        update_package_by_id(&mut transaction, json_data).await?;
    } else {
        insert_package(&mut transaction, json_data).await?;
    }

    complete_scrapper_job(&mut transaction, message.job_id).await?;

    transaction.commit().await?;
    Ok(())
}

struct HeaderExtractor<'a>(&'a Delivery);

impl<'a> Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        if let Some(headers) = self.0.properties.headers() {
            if let Some(value) = headers.inner().get(key) {
                // Convert the AMQP value to a string
                return match value {
                    lapin::types::AMQPValue::LongString(s) => {
                        std::str::from_utf8(s.as_bytes()).ok()
                    }
                    _ => None,
                };
            }
        }
        None
    }

    fn keys(&self) -> Vec<&str> {
        if let Some(headers) = self.0.properties.headers() {
            return headers.inner().keys().map(|k| k.as_str()).collect();
        }
        vec![]
    }
}

#[instrument(name = "handle_new_message", skip_all)]
pub async fn handle_new_message(
    delivery: &Delivery,
    minio_client: MinioClient,
    db_pool: Pool<Postgres>,
) -> () {
    let extractor = HeaderExtractor(delivery);

    let context = global::get_text_map_propagator(|prop| prop.extract(&extractor));

    let span = info_span!("consume_message");

    span.set_parent(context);

    match consume_message(&delivery, minio_client, db_pool)
        .instrument(span)
        .await
    {
        Ok(_) => delivery.ack(BasicAckOptions::default()).await.expect("ack"),
        Err(err) => {
            tracing::error!("Failed to consume message: {:?}", err);
            delivery
                .nack(BasicNackOptions {
                    multiple: false,
                    requeue: false,
                })
                .await
                .expect("nack");
        }
    }
}

#[instrument(name = "update_package_by_id", skip_all)]
async fn update_package_by_id(
    transaction: &mut Transaction<'static, Postgres>,
    package: PackageOutput,
) -> Result<()> {
    let query = sqlx::query!(
        r#"UPDATE packages SET name = $1, version = $2, downloads = $3 WHERE id = $4;"#,
        package.name,
        package.version,
        package.downloads as i64,
        package.id,
    );

    let result = transaction.execute(query).await?;
    let rows_affected = result.rows_affected();
    if rows_affected != 1 {
        anyhow::bail!("Expected 1 row to be affected, got {}", rows_affected);
    }

    Ok(())
}

#[instrument(name = "insert_package", skip_all)]
async fn insert_package(
    transaction: &mut Transaction<'static, Postgres>,
    package: PackageOutput,
) -> Result<()> {
    let query = sqlx::query!(
        r#"INSERT INTO packages(id, name, version, downloads)
        VALUES ($1, $2, $3, $4);
        "#,
        package.id,
        package.name,
        package.version,
        package.downloads as i64,
    );

    let result = transaction.execute(query).await?;
    let rows_affected = result.rows_affected();
    if rows_affected != 1 {
        anyhow::bail!("Expected 1 row to be affected, got {}", rows_affected);
    }

    Ok(())
}

#[instrument(name = "complete_scrapper_job", skip(transaction))]
async fn complete_scrapper_job(
    transaction: &mut Transaction<'static, Postgres>,
    job_id: Uuid,
) -> Result<()> {
    let query = sqlx::query!(
        r#"UPDATE scrapper_jobs SET status = 'completed' WHERE id = $1;"#,
        job_id
    );
    let result = transaction.execute(query).await?;
    let rows_affected = result.rows_affected();
    if rows_affected != 1 {
        anyhow::bail!("Expected 1 row to be affected, got {}", rows_affected);
    }

    Ok(())
}

async fn index() -> impl IntoResponse {
    Html(
        Documentation::new("Api Documentation title", "/openapi")
            .build()
            .unwrap(),
    )
}

async fn openapi() -> &'static str {
    include_str!("../openapi.json")
}
