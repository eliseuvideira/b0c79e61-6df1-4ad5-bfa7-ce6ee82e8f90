use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct JobMessage {
    pub job_id: Uuid,
    pub registry: String,
    pub package_name: String,
}

pub trait Cursor {
    fn cursor(&self) -> String;
}
