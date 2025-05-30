use std::{collections::HashMap, sync::Arc};

use lapin::Connection;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::{db, types::Cursor};

#[derive(Debug, Serialize)]
pub struct ApiResponseList<T> {
    pub data: Vec<T>,
    pub next_cursor: Option<String>,
}

impl<T> ApiResponseList<T>
where
    T: Serialize + Cursor,
{
    pub fn new(items: Vec<T>, limit: Limit) -> Self {
        let mut data = items;
        let limit: u64 = limit.into();
        let has_more = data.len() > limit as usize;

        data.truncate(limit as usize);

        let next_cursor = if has_more {
            data.last().map(|item| item.cursor())
        } else {
            None
        };

        Self { data, next_cursor }
    }
}

#[derive(Debug, Serialize, Deserialize)]
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
    pub after: Option<Uuid>,
    #[serde(default)]
    pub order: Order,
}

#[derive(Debug, Deserialize, PartialEq, Copy, Clone)]
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

pub struct Limit(u64);

impl From<Limit> for u64 {
    fn from(limit: Limit) -> Self {
        limit.0
    }
}

impl Limit {
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for Limit {
    type Error = crate::error::Error;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        if value > 100 {
            Err(crate::error::Error::InvalidInput(
                "Limit must be less than 100".to_string(),
            ))
        } else {
            Ok(Limit(value))
        }
    }
}

pub struct AppState {
    pub db_pool: Pool<Postgres>,
    pub rabbitmq_connection: Arc<Connection>,
    pub integration_queues: HashMap<String, String>,
    pub exchange_name: String,
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use uuid::Uuid;

    use crate::{
        api::types::{ApiResponse, ApiResponseList, Limit, Order},
        db,
        types::Cursor,
    };

    #[derive(Serialize)]
    struct Item {
        id: String,
    }
    impl Cursor for Item {
        fn cursor(&self) -> String {
            self.id.to_string()
        }
    }

    #[test]
    fn test_api_response_list_returns_no_next_cursor() {
        // Arrange
        let items: Vec<Item> = (0..100)
            .map(|i| Item {
                id: Uuid::from_u64_pair(i as u64, 0).to_string(),
            })
            .collect();
        assert_eq!(items.len(), 100);

        // Act
        let list = ApiResponseList::new(items, Limit(100));

        // Assert
        assert_eq!(list.next_cursor, None);
        assert_eq!(list.data.len(), 100);
    }

    #[test]
    fn test_api_response_list_returns_next_cursor() {
        // Arrange
        let items: Vec<Item> = (0..=100)
            .map(|i| Item {
                id: Uuid::from_u64_pair(i as u64, 0).to_string(),
            })
            .collect();
        assert_eq!(items.len(), 101);

        // Act
        let list = ApiResponseList::new(items, Limit(100));

        // Assert
        assert_eq!(list.data.len(), 100);
        assert_eq!(
            list.next_cursor,
            Some(Uuid::from_u64_pair(99, 0).to_string())
        );
    }

    #[test]
    fn test_api_response_wraps_data_in_json() {
        // Arrange
        let uuid = Uuid::from_u64_pair(0, 0);
        let item: Item = Item {
            id: uuid.to_string(),
        };

        // Act
        let list = ApiResponse::new(item);

        // Assert
        let json = serde_json::to_string(&list).unwrap();
        assert_eq!(
            json,
            "{\"data\":{\"id\":\"00000000-0000-0000-0000-000000000000\"}}"
        );
    }

    #[test]
    fn test_order_is_default_to_desc() {
        assert_eq!(Order::default(), Order::Desc);
    }

    #[test]
    fn test_order_converts_to_db_order_for_asc() {
        let api_order = Order::Asc;
        let db_order = db::Order::from(api_order);
        assert_eq!(db_order, db::Order::Asc);

        let db_order: db::Order = api_order.into();
        assert_eq!(db_order, db::Order::Asc);
    }

    #[test]
    fn test_order_converts_to_db_order_for_desc() {
        let api_order = Order::Desc;
        let db_order = db::Order::from(api_order);
        assert_eq!(db_order, db::Order::Desc);

        let db_order: db::Order = api_order.into();
        assert_eq!(db_order, db::Order::Desc);
    }
}
