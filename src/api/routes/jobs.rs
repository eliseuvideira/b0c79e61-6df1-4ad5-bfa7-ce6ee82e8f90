use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use axum_tracing_opentelemetry::tracing_opentelemetry_instrumentation_sdk::find_current_trace_id;
use chrono::Utc;
use http::StatusCode;
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

use crate::{
    api::types::{ApiResponse, ApiResponseList, AppState, Limit, PaginationQuery},
    db,
    error::Error,
    models::job::{Job, JobStatus},
    services::rabbitmq,
    types::JobMessage,
};

pub fn create_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/jobs", post(create_job))
        .route("/jobs", get(get_jobs))
        .route("/jobs/:id", get(get_job_by_id))
        .with_state(app_state)
}

#[derive(Debug, Deserialize)]
pub struct CreateJobPayload {
    pub registry: String,
    pub package_name: String,
}

#[instrument(name = "create_job", skip(app_state))]
pub async fn create_job(
    State(app_state): State<Arc<AppState>>,
    Json(payload): Json<CreateJobPayload>,
) -> Result<impl IntoResponse, Error> {
    let id = Uuid::now_v7();
    let registry = payload.registry;
    let package_name = payload.package_name;
    let trace_id = find_current_trace_id();

    let mut transaction = app_state.db_pool.begin().await?;

    let routing_key = app_state
        .integration_queues
        .get(&registry)
        .context("Registry not found")?
        .clone();

    let job = db::insert_job(
        &mut transaction,
        Job {
            id,
            registry,
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

#[instrument(name = "get_jobs", skip(app_state))]
pub async fn get_jobs(
    Query(query): Query<PaginationQuery>,
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, Error> {
    let limit: Limit = query.limit.unwrap_or(100).try_into()?;
    let after = query.after;
    let order = query.order.into();

    let mut conn = app_state.db_pool.acquire().await?;
    let jobs = db::get_jobs(&mut conn, limit.as_u64() + 1, after, order).await?;

    Ok(Json(ApiResponseList::new(jobs, limit)))
}

#[instrument(name = "get_job_by_id", skip(app_state))]
pub async fn get_job_by_id(
    Path(id): Path<String>,
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, Error> {
    let id = Uuid::parse_str(&id).context("Invalid job ID")?;

    let mut conn = app_state.db_pool.acquire().await?;
    let Some(job) = db::get_job_by_id(&mut conn, id).await? else {
        return Err(Error::NotFound("Not found".to_string()));
    };

    Ok(Json(ApiResponse::new(job)))
}
