use std::sync::Arc;

use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use tracing::instrument;

use crate::{
    api::types::{ApiResponseList, AppState, Limit, PaginationQuery},
    db,
    error::Error,
};

pub fn create_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/packages", get(get_packages))
        .with_state(app_state)
}

#[instrument(name = "get_packages", skip(app_state))]
pub async fn get_packages(
    Query(query): Query<PaginationQuery>,
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, Error> {
    let limit: Limit = query.limit.unwrap_or(100).try_into()?;
    let after = query.after;
    let order = query.order.into();

    let mut conn = app_state.db_pool.acquire().await?;
    let packages = db::get_packages(&mut conn, limit.as_u64() + 1, after, order).await?;

    Ok(Json(ApiResponseList::new(packages, limit)))
}
