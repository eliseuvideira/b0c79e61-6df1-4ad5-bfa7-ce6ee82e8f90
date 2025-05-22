use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{
    types::chrono::{DateTime, Utc},
    PgConnection,
};
use tracing::{instrument, Instrument};
use uuid::Uuid;

use crate::telemetry::{instrument_query, Operation};

#[derive(Debug, Deserialize, Serialize)]
pub enum ScrapperJobStatus {
    #[serde(rename = "pending")]
    Processing,
    #[serde(rename = "completed")]
    Completed,
}

impl From<String> for ScrapperJobStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "pending" => ScrapperJobStatus::Processing,
            "completed" => ScrapperJobStatus::Completed,
            _ => ScrapperJobStatus::Processing,
        }
    }
}

impl ScrapperJobStatus {
    pub fn to_string(&self) -> String {
        match self {
            ScrapperJobStatus::Processing => "pending".to_string(),
            ScrapperJobStatus::Completed => "completed".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ScrapperJob {
    pub id: Uuid,
    pub registry_name: String,
    pub package_name: String,
    pub status: String,
    pub trace_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[instrument(name = "insert_scrapper_job", skip(conn))]
pub async fn insert_scrapper_job(
    conn: &mut PgConnection,
    scrapper_job: ScrapperJob,
) -> Result<ScrapperJob> {
    let result = sqlx::query_as!(
        ScrapperJob,
        "INSERT INTO scrapper_jobs (id, registry_name, package_name, status, trace_id, created_at) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *;",
        scrapper_job.id,
        scrapper_job.registry_name,
        scrapper_job.package_name,
        scrapper_job.status.to_string(),
        scrapper_job.trace_id,
        scrapper_job.created_at,
    )
    .fetch_one(&mut *conn)
    .instrument(instrument_query(Operation::Insert, "scrapper_jobs"))
    .await?;

    Ok(result)
}

#[instrument(name = "complete_scrapper_job", skip(conn))]
pub async fn complete_scrapper_job(conn: &mut PgConnection, id: Uuid) -> Result<ScrapperJob> {
    let scrapper_job = sqlx::query_as!(
        ScrapperJob,
        "UPDATE scrapper_jobs SET status = 'completed' WHERE id = $1 RETURNING *;",
        id,
    )
    .fetch_one(&mut *conn)
    .instrument(instrument_query(Operation::Update, "scrapper_jobs"))
    .await?;

    Ok(scrapper_job)
}
