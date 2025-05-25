use std::{collections::HashMap, sync::Arc};

use lapin::Connection;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::{db, types::Cursor};

#[derive(Debug, Serialize)]
pub struct ApiResponsePagination<T> {
    pub data: Vec<T>,
    pub next_cursor: Option<String>,
}

impl<T> ApiResponsePagination<T>
where
    T: Serialize + Cursor,
{
    pub fn new(items: Vec<T>, limit: u64) -> Self {
        let mut data = items;
        let has_more = data.len() > limit as usize;

        let (data, last) = if has_more {
            match data.pop() {
                Some(last) => (data, Some(last)),
                None => (data, None),
            }
        } else {
            (data, None)
        };

        let next_cursor = last.map(|last| last.cursor());

        Self { data, next_cursor }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<u64>,
    pub cursor: Option<Uuid>,
    #[serde(default)]
    pub order: Order,
}

#[derive(Debug, Deserialize)]
pub enum Order {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
}

impl Default for Order {
    fn default() -> Self {
        Self::Desc
    }
}

impl From<Order> for db::Order {
    fn from(order: Order) -> Self {
        match order {
            Order::Asc => db::Order::Asc,
            Order::Desc => db::Order::Desc,
        }
    }
}

pub struct AppState {
    pub db_pool: Pool<Postgres>,
    pub rabbitmq_connection: Arc<Connection>,
    pub integration_queues: HashMap<String, String>,
    pub exchange_name: String,
}
