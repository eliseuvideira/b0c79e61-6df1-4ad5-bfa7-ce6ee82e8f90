use axum::{
    body::Body,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use http::header;
use mime;
use scalar_doc::Documentation;

pub fn create_router() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/openapi.json", get(openapi))
}

async fn index() -> impl IntoResponse {
    Html(
        Documentation::new("Integrations API", "/openapi.json")
            .build()
            .unwrap(),
    )
}

async fn openapi() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
        .body(Body::from(include_str!("../../../openapi.json")))
        .unwrap()
}
