use axum::response::{Html, IntoResponse};
use scalar_doc::Documentation;

pub async fn index() -> impl IntoResponse {
    Html(
        Documentation::new("Integrations API", "/openapi")
            .build()
            .unwrap(),
    )
}

pub async fn openapi() -> &'static str {
    include_str!("../../../openapi.json")
}
