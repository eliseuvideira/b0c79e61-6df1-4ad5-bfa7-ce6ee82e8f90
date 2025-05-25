use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct JobMessage {
    pub job_id: Uuid,
    pub package_name: String,
}
