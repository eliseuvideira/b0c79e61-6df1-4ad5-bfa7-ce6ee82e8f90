use std::fmt::Display;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::Cursor;

#[derive(Debug, Deserialize, Serialize)]
pub enum JobStatus {
    #[serde(rename = "processing")]
    Processing,
    #[serde(rename = "completed")]
    Completed,
}

impl From<String> for JobStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "processing" => JobStatus::Processing,
            "completed" => JobStatus::Completed,
            _ => {
                tracing::warn!(status = s, "Invalid job status");
                JobStatus::Processing
            }
        }
    }
}

impl Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Processing => write!(f, "processing"),
            JobStatus::Completed => write!(f, "completed"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Job {
    pub id: Uuid,
    pub registry_name: String,
    pub package_name: String,
    pub status: JobStatus,
    pub trace_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Cursor for Job {
    fn cursor(&self) -> String {
        self.id.to_string()
    }
}
