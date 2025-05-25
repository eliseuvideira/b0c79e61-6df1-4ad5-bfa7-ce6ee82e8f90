use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Result};
use axum::{
    extract::{Query, Request, State},
    middleware::{from_fn, from_fn_with_state, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    serve::Serve,
    Json, Router,
};
use axum_tracing_opentelemetry::{
    middleware::{OtelAxumLayer, OtelInResponseLayer},
    tracing_opentelemetry_instrumentation_sdk::find_current_trace_id,
};
use chrono::Utc;
use lapin::Connection;
use opentelemetry::trace::TraceContextExt;
use prometheus::{Encoder, TextEncoder};
use reqwest::StatusCode;
use scalar_doc::Documentation;
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use types::{ApiResponse, ApiResponsePagination};
use uuid::Uuid;

use crate::{
    config::Config,
    db,
    error::Error,
    models::job::{Job, JobStatus},
    services::rabbitmq,
    telemetry::Metrics,
    types::JobMessage,
};

mod middlewares;
mod types;

pub struct Api {
    port: u16,
    server: Serve<Router, Router>,
}

impl Api {
    pub async fn build(
        configuration: &Config,
        db_pool: Pool<Postgres>,
        rabbitmq_connection: Arc<Connection>,
        integration_queues: HashMap<String, String>,
        metrics: Arc<Metrics>,
    ) -> Result<Self> {
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

        let app_state = Arc::new(AppState {
            db_pool: db_pool.clone(),
            rabbitmq_connection,
            integration_queues,
            exchange_name: configuration.rabbitmq.exchange_name.clone(),
        });

        let metrics_router = Router::new()
            .route("/metrics", get(metrics_handler))
            .with_state(metrics.clone());

        let router = Router::new()
            .route("/jobs", post(create_job))
            .route("/jobs", get(get_jobs))
            .route("/", get(index))
            .route("/openapi", get(openapi))
            .layer(TraceLayer::new_for_http())
            .layer(from_fn(attach_trace_id))
            .layer(from_fn_with_state(
                metrics.clone(),
                middlewares::record_metrics,
            ))
            .layer(OtelInResponseLayer)
            .layer(OtelAxumLayer::default())
            .with_state(app_state)
            .merge(metrics_router)
            .route("/health", get(health_check))
            .fallback(not_found);

        let server = axum::serve(listener, router);

        Ok(Self { port, server })
    }

    pub async fn run_until_stopped(self) -> Result<()> {
        self.server.await.context("Server failed to start")
    }

    pub fn port(&self) -> u16 {
        self.port
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
    include_str!("../../openapi.json")
}

#[derive(Debug, Deserialize)]
struct PaginationQuery {
    limit: Option<u64>,
    cursor: Option<Uuid>,
    #[serde(default)]
    order: Order,
}

#[derive(Debug, Deserialize)]
enum Order {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
}

impl Default for Order {
    fn default() -> Self {
        Self::Desc
    }
}

impl From<Order> for db::Order {
    fn from(order: Order) -> Self {
        match order {
            Order::Asc => db::Order::Asc,
            Order::Desc => db::Order::Desc,
        }
    }
}

async fn get_jobs(
    Query(query): Query<PaginationQuery>,
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, Error> {
    let limit = query.limit.unwrap_or(100);
    let cursor = query.cursor;
    let order = query.order.into();

    let mut conn = app_state.db_pool.acquire().await?;
    let jobs = db::get_jobs(&mut conn, limit, cursor, order).await?;

    Ok(Json(ApiResponsePagination::new(jobs, limit)))
}

struct AppState {
    db_pool: Pool<Postgres>,
    rabbitmq_connection: Arc<Connection>,
    integration_queues: HashMap<String, String>,
    exchange_name: String,
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

#[instrument(name = "create_job", skip(app_state))]
async fn create_job(
    State(app_state): State<Arc<AppState>>,
    Json(payload): Json<CreateJobPayload>,
) -> Result<impl IntoResponse, Error> {
    let id = Uuid::now_v7();
    let registry_name = payload.registry_name;
    let package_name = payload.package_name;
    let trace_id = find_current_trace_id();

    let mut transaction = app_state.db_pool.begin().await?;

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

    Ok((StatusCode::CREATED, Json(ApiResponse::new(job))))
}

async fn metrics_handler(State(metrics): State<Arc<Metrics>>) -> impl IntoResponse {
    let metrics = metrics.registry.gather();
    let encoder = TextEncoder::new();
    let mut buffer = Vec::new();
    encoder.encode(&metrics, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
