use anyhow::Result;
use sqlx::PgConnection;
use tracing::{instrument, Instrument};
use uuid::Uuid;

use crate::{
    models::job::Job,
    telemetry::{instrument_query, Operation},
};

#[instrument(name = "insert_job", skip(conn))]
pub async fn insert_job(
    conn: &mut PgConnection,
    job: Job,
) -> Result<Job> {
    let result = sqlx::query_as!(
        Job,
        "INSERT INTO jobs (id, registry_name, package_name, status, trace_id, created_at) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *;",
        job.id,
        job.registry_name,
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
