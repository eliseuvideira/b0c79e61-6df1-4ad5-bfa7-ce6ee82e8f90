use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize)]
pub struct Package {
    pub id: Uuid,
    pub registry: String,
    pub name: String,
    pub version: String,
    pub downloads: i64,
}
