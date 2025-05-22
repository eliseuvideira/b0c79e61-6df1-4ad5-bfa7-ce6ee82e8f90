use std::fmt::Display;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
            _ => {
                tracing::warn!(status = s, "Invalid scrapper job status");
                ScrapperJobStatus::Processing
            }
        }
    }
}

impl Display for ScrapperJobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScrapperJobStatus::Processing => write!(f, "pending"),
            ScrapperJobStatus::Completed => write!(f, "completed"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ScrapperJob {
    pub id: Uuid,
    pub registry_name: String,
    pub package_name: String,
    pub status: ScrapperJobStatus,
    pub trace_id: Option<String>,
    pub created_at: DateTime<Utc>,
}
