use anyhow::Result;
use sqlx::PgConnection;
use tracing::{instrument, Instrument};
use uuid::Uuid;

use crate::{
    models::job::Job,
    telemetry::{instrument_query, Operation},
};

use super::types::Order;

#[instrument(name = "insert_job", skip(conn))]
pub async fn insert_job(conn: &mut PgConnection, job: Job) -> Result<Job> {
    let result = sqlx::query_as!(
        Job,
        "INSERT INTO jobs (id, registry, package_name, status, trace_id, created_at) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *;",
        job.id,
        job.registry,
        job.package_name,
        job.status.to_string(),
        job.trace_id,
        job.created_at,
    )
    .fetch_one(&mut *conn)
    .instrument(instrument_query(Operation::Insert, "jobs"))
    .await?;

    Ok(result)
}

#[instrument(name = "complete_job", skip(conn))]
pub async fn complete_job(conn: &mut PgConnection, id: Uuid) -> Result<Job> {
    let job = sqlx::query_as!(
        Job,
        "UPDATE jobs SET status = 'completed' WHERE id = $1 RETURNING *;",
        id,
    )
    .fetch_one(&mut *conn)
    .instrument(instrument_query(Operation::Update, "jobs"))
    .await?;

    Ok(job)
}

#[instrument(name = "get_jobs", skip(conn))]
pub async fn get_jobs(
    conn: &mut PgConnection,
    limit: u64,
    after: Option<Uuid>,
    order: Order,
) -> Result<Vec<Job>> {
    match after {
        Some(after) => get_jobs_with_limit_after(conn, limit, after, order).await,
        None => get_jobs_with_limit(conn, limit, order).await,
    }
}

async fn get_jobs_with_limit(
    conn: &mut PgConnection,
    limit: u64,
    order: Order,
) -> Result<Vec<Job>> {
    match order {
        Order::Asc => get_jobs_with_limit_asc(conn, limit).await,
        Order::Desc => get_jobs_with_limit_desc(conn, limit).await,
    }
}

async fn get_jobs_with_limit_asc(conn: &mut PgConnection, limit: u64) -> Result<Vec<Job>> {
    let jobs = sqlx::query_as!(
        Job,
        "SELECT * FROM jobs ORDER BY id ASC LIMIT $1;",
        limit as i64
    )
    .fetch_all(conn)
    .instrument(instrument_query(Operation::Select, "jobs"))
    .await?;
    Ok(jobs)
}

async fn get_jobs_with_limit_desc(conn: &mut PgConnection, limit: u64) -> Result<Vec<Job>> {
    let jobs = sqlx::query_as!(
        Job,
        "SELECT * FROM jobs ORDER BY id DESC LIMIT $1;",
        limit as i64
    )
    .fetch_all(conn)
    .instrument(instrument_query(Operation::Select, "jobs"))
    .await?;
    Ok(jobs)
}

async fn get_jobs_with_limit_after(
    conn: &mut PgConnection,
    limit: u64,
    after: Uuid,
    order: Order,
) -> Result<Vec<Job>> {
    match order {
        Order::Asc => get_jobs_with_limit_after_asc(conn, limit, after).await,
        Order::Desc => get_jobs_with_limit_after_desc(conn, limit, after).await,
    }
}

async fn get_jobs_with_limit_after_asc(
    conn: &mut PgConnection,
    limit: u64,
    after: Uuid,
) -> Result<Vec<Job>> {
    let jobs = sqlx::query_as!(
        Job,
        "SELECT * FROM jobs WHERE id > $1 ORDER BY id ASC LIMIT $2;",
        after,
        limit as i64,
    )
    .fetch_all(conn)
    .instrument(instrument_query(Operation::Select, "jobs"))
    .await?;
    Ok(jobs)
}

async fn get_jobs_with_limit_after_desc(
    conn: &mut PgConnection,
    limit: u64,
    after: Uuid,
) -> Result<Vec<Job>> {
    let jobs = sqlx::query_as!(
        Job,
        "SELECT * FROM jobs WHERE id < $1 ORDER BY id DESC LIMIT $2;",
        after,
        limit as i64,
    )
    .fetch_all(conn)
    .instrument(instrument_query(Operation::Select, "jobs"))
    .await?;
    Ok(jobs)
}

#[instrument(name = "get_one", skip(conn))]
pub async fn get_job_by_id(conn: &mut PgConnection, id: Uuid) -> Result<Option<Job>> {
    let job = sqlx::query_as!(Job, "SELECT * FROM jobs WHERE id = $1;", id)
        .fetch_optional(conn)
        .instrument(instrument_query(Operation::Select, "jobs"))
        .await?;

    Ok(job)
}
