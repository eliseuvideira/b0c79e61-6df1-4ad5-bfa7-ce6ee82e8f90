use std::{collections::HashMap, sync::Arc};

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
use lapin::{
    message::DeliveryResult,
    options::{BasicAckOptions, BasicNackOptions},
    types::{AMQPValue, FieldTable, ShortString},
    Connection,
};
use metrics_exporter_prometheus::PrometheusHandle;
use opentelemetry::{
    global::{self},
    propagation::Extractor,
    trace::TraceContextExt,
};
use scalar_doc::Documentation;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, types::chrono::Utc, Pool, Postgres};
use tokio::{net::TcpListener, try_join};
use tower_http::trace::TraceLayer;
use tracing::{info_span, instrument, Instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

use crate::{
    config::{DatabaseSettings, Settings},
    db,
    error::Error,
    models::{
        package::Package,
        job::{Job, JobStatus},
    },
    services::{minio, rabbitmq},
};

pub struct Application {
    port: u16,
    server: Serve<Router, Router>,
    minio_client: MinioClient,
    db_pool: Pool<Postgres>,
    rabbitmq_connection: Arc<Connection>,
    queue_consumer: String,
}

struct AppState {
    db: Pool<Postgres>,
    rabbitmq_connection: Arc<Connection>,
    integration_queues: HashMap<String, String>,
    exchange_name: String,
}

impl Application {
    pub async fn build(configuration: Settings, metrics_handle: PrometheusHandle) -> Result<Self> {
        let db_pool = get_db_pool(&configuration.database);

        let rabbitmq_connection = rabbitmq::connect(&configuration.rabbitmq).await?;
        let rabbitmq_connection = Arc::new(rabbitmq_connection);
        let channel = rabbitmq_connection.create_channel().await?;

        rabbitmq::declare_exchange(&channel, &configuration.rabbitmq.exchange_name).await?;
        for queue in configuration.rabbitmq.queues.iter() {
            rabbitmq::declare_and_bind_queue(
                &channel,
                queue,
                &configuration.rabbitmq.exchange_name,
            )
            .await?;
        }
        rabbitmq::declare_and_bind_queue(
            &channel,
            &configuration.rabbitmq.queue_consumer,
            &configuration.rabbitmq.exchange_name,
        )
        .await?;

        let queue_consumer = configuration.rabbitmq.queue_consumer;

        let minio_client = minio::create_client(&configuration.minio).await?;

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

        let integration_queues: HashMap<String, String> =
            configuration.rabbitmq.registry_queues.into_iter().collect();

        let app_state = AppState {
            db: db_pool.clone(),
            rabbitmq_connection: rabbitmq_connection.clone(),
            integration_queues,
            exchange_name: configuration.rabbitmq.exchange_name,
        };
        let app_state = Arc::new(app_state);

        let prometheus_layer = axum_prometheus::PrometheusMetricLayer::default();

        let router = Router::new()
            .route("/jobs", post(create_job))
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
            rabbitmq_connection,
            queue_consumer,
        })
    }

    pub async fn run_until_stopped(self) -> Result<()> {
        let consumer_handle = Application::run_consumer(
            self.rabbitmq_connection,
            self.minio_client,
            self.db_pool,
            self.queue_consumer,
        );
        let server_handle = Application::run_server(self.server);

        try_join!(consumer_handle, server_handle)?;

        Ok(())
    }

    async fn run_server(server: Serve<Router, Router>) -> Result<()> {
        server.await.context("Server failed to start")
    }

    async fn run_consumer(
        rabbitmq_connection: Arc<Connection>,
        minio_client: MinioClient,
        db_pool: Pool<Postgres>,
        consumer_queue: String,
    ) -> Result<()> {
        let channel = rabbitmq_connection.create_channel().await?;
        let consumer = rabbitmq::create_consumer(&channel, &consumer_queue).await?;

        consumer.set_delegate(move |delivery: DeliveryResult| {
            let minio_client_for_task = minio_client.clone();
            let db_pool_for_task = db_pool.clone();

            async move {
                match delivery {
                    Ok(Some(delivery)) => {
                        match parse_and_run_consume(
                            &delivery.data,
                            delivery.properties.headers(),
                            minio_client_for_task,
                            db_pool_for_task,
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
    let span = Span::current();
    let context = span.context();
    let otel_context = context.span().span_context().clone();
    if otel_context.is_valid() {
        let trace_id = otel_context.trace_id().to_string();
        span.record("trace_id", trace_id);
    }

    next.run(req).await
}

#[derive(Debug, Deserialize)]
struct CreateJobPayload {
    registry_name: String,
    package_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct JobMessage {
    job_id: Uuid,
    package_name: String,
}

#[instrument(name = "create_job", skip(app_state))]
async fn create_job(
    State(app_state): State<Arc<AppState>>,
    Json(payload): Json<CreateJobPayload>,
) -> Result<impl IntoResponse, Error> {
    let id = Uuid::now_v7();
    let registry_name = payload.registry_name;
    let package_name = payload.package_name;
    let trace_id = find_current_trace_id();

    let mut transaction = app_state.db.begin().await?;

    let routing_key = app_state
        .integration_queues
        .get(&registry_name)
        .context("Registry not found")?
        .clone();

    let job = db::insert_job(
        &mut transaction,
        Job {
            id,
            registry_name,
            package_name,
            status: JobStatus::Processing,
            trace_id: trace_id.clone(),
            created_at: Utc::now(),
        },
    )
    .await?;

    transaction.commit().await?;

    let message = JobMessage {
        job_id: job.id,
        package_name: job.package_name.clone(),
    };
    let channel = app_state.rabbitmq_connection.create_channel().await?;

    rabbitmq::publish_message(&channel, &app_state.exchange_name, &routing_key, &message).await?;

    Ok((StatusCode::CREATED, Json(job)))
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
    message: JobMessage,
    minio_client: MinioClient,
    db_pool: Pool<Postgres>,
) -> Result<()> {
    let response = minio_client
        .get_object()
        .bucket("outputs")
        .key(format!("outputs/{}.json", message.package_name))
        .send()
        .await?;

    let data = response.body.collect().await?;
    let json_data = serde_json::from_slice::<PackageOutput>(&data.into_bytes())?;

    let mut transaction = db_pool.begin().await?;

    let package = Package {
        id: json_data.id,
        name: json_data.name,
        version: json_data.version,
        downloads: json_data.downloads as i64,
    };

    db::upsert_package(&mut transaction, package).await?;
    db::complete_job(&mut transaction, message.job_id).await?;

    transaction.commit().await?;

    Ok(())
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

async fn parse_and_run_consume(
    data: &[u8],
    headers: &Option<FieldTable>,
    minio_client: MinioClient,
    db_pool: Pool<Postgres>,
) -> Result<()> {
    let message = serde_json::from_slice::<JobMessage>(data)?;
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

    consume_message(message, minio_client, db_pool)
        .instrument(span)
        .await
}
