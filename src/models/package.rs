use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::Cursor;

#[derive(Debug, Deserialize, Serialize)]
pub struct Package {
    pub id: Uuid,
    pub registry: String,
    pub name: String,
    pub version: String,
    pub downloads: i64,
}

impl Cursor for Package {
    fn cursor(&self) -> String {
        self.id.to_string()
    }
}
