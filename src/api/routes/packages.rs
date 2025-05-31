use std::sync::Arc;

use anyhow::Context;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use tracing::instrument;
use uuid::Uuid;

use crate::{
    api::types::{ApiResponse, ApiResponseList, AppState, Limit, PaginationQuery},
    db,
    error::Error,
};

pub fn create_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/packages", get(get_packages))
        .route("/packages/:id", get(get_package_by_id))
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

#[instrument(name = "get_package_by_id", skip(app_state))]
pub async fn get_package_by_id(
    Path(id): Path<String>,
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, Error> {
    let id = Uuid::parse_str(&id).map_err(|_| Error::InvalidInput("Invalid package ID".to_string()))?;

    let mut conn = app_state.db_pool.acquire().await?;
    let Some(package) = db::get_package_by_id(&mut conn, id).await? else {
        return Err(Error::NotFound(format!("Package with id {} not found", id)));
    };

    Ok(Json(ApiResponse::new(package)))
}
