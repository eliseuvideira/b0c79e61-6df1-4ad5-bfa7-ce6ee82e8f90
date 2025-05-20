use anyhow::{Context, Result};
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::{from_fn, Next},
    response::{IntoResponse, Response},
    routing::{get, post, Router},
    serve::Serve,
    Json,
};
use axum_tracing_opentelemetry::{
    middleware::{OtelAxumLayer, OtelInResponseLayer},
    tracing_opentelemetry_instrumentation_sdk::find_current_trace_id,
};
use metrics_exporter_prometheus::PrometheusHandle;
use serde::{Deserialize, Serialize};
use sqlx::{
    postgres::PgPoolOptions,
    types::chrono::{DateTime, Utc},
    Executor, Pool, Postgres, Transaction,
};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{debug_span, instrument, Instrument};
use uuid::Uuid;

use crate::{
    config::{DatabaseSettings, Settings},
    error::Error,
};

pub struct Application {
    port: u16,
    server: Serve<Router, Router>,
}

impl Application {
    pub async fn build(configuration: Settings, metrics_handle: PrometheusHandle) -> Result<Self> {
        let db_pool = get_db_pool(&configuration.database);

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

        let router = Router::new()
            .route("/scrapper-jobs", post(create_scrapper_job))
            .layer(TraceLayer::new_for_http())
            .layer(from_fn(attach_trace_id))
            .layer(OtelInResponseLayer)
            .layer(OtelAxumLayer::default())
            .with_state(db_pool)
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

#[instrument(name = "create_scrapper_job")]
async fn create_scrapper_job(
    State(db_pool): State<Pool<Postgres>>,
    Json(payload): Json<CreateScrapperJobPayload>,
) -> Result<impl IntoResponse, Error> {
    let id = Uuid::now_v7();
    let registry_name = payload.registry_name;
    let package_name = payload.package_name;
    let trace_id = find_current_trace_id();

    let mut transaction = db_pool.begin().await?;

    let scrapper_job = insert_scrapper_job(
        &mut transaction,
        CreateScrapperJob {
            id,
            registry_name,
            package_name,
            trace_id,
            created_at: Utc::now(),
        },
    )
    .await?;

    transaction.commit().await?;

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
    transaction: &mut Transaction<'_, Postgres>,
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
