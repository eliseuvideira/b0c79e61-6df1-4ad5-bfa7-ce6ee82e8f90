use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub version: String,
    pub downloads: i64,
}
