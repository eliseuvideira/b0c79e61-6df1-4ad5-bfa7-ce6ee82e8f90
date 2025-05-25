use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Result};
use axum::{
    extract::{Query, Request, State},
    middleware::{from_fn, Next},
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
use metrics_exporter_prometheus::PrometheusHandle;
use opentelemetry::trace::TraceContextExt;
use reqwest::StatusCode;
use scalar_doc::Documentation;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

use crate::{
    config::Settings,
    db,
    error::Error,
    models::job::{Job, JobStatus},
    services::rabbitmq,
    types::JobMessage,
};

pub struct Api {
    port: u16,
    server: Serve<Router, Router>,
}

impl Api {
    pub async fn build(
        configuration: &Settings,
        db_pool: Pool<Postgres>,
        rabbitmq_connection: Arc<Connection>,
        integration_queues: HashMap<String, String>,
        metrics_handle: PrometheusHandle,
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
            db: db_pool.clone(),
            rabbitmq_connection: rabbitmq_connection.clone(),
            integration_queues,
            exchange_name: configuration.rabbitmq.exchange_name.clone(),
        });

        let prometheus_layer = axum_prometheus::PrometheusMetricLayer::default();

        let router = Router::new()
            .route("/jobs", post(create_job))
            .route("/jobs", get(get_jobs))
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
    include_str!("../openapi.json")
}

#[derive(Debug, Deserialize)]
struct PaginationQuery {
    limit: Option<u64>,
    after: Option<Uuid>,
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

#[derive(Debug, Serialize)]
struct WithPagination<T> {
    data: Vec<T>,
    has_more: bool,
}

async fn get_jobs(
    Query(query): Query<PaginationQuery>,
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, Error> {
    let limit = query.limit.unwrap_or(100);
    let after = query.after;
    let order = query.order.into();

    let mut conn = app_state.db.acquire().await?;
    let jobs = db::get_jobs(&mut conn, limit, after, order).await?;

    Ok(Json(paginate(jobs, limit)))
}

fn paginate<T>(items: Vec<T>, limit: u64) -> WithPagination<T>
where
    T: Serialize,
{
    let has_more = items.len() > limit as usize;
    let data = if has_more {
        items.into_iter().take(limit as usize).collect()
    } else {
        items
    };

    WithPagination { data, has_more }
}

struct AppState {
    db: Pool<Postgres>,
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
