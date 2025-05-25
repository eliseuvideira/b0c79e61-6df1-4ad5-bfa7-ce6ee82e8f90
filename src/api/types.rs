use serde::Serialize;

use crate::types::Cursor;

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
