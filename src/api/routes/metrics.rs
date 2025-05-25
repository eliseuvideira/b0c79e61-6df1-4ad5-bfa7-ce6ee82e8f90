use std::sync::Arc;

use axum::{extract::State, response::IntoResponse, routing::get, Router};
use prometheus::{Encoder, TextEncoder};

use crate::telemetry::Metrics;

pub fn create_router(metrics: Arc<Metrics>) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(metrics)
}

async fn metrics_handler(State(metrics): State<Arc<Metrics>>) -> impl IntoResponse {
    let metrics = metrics.registry.gather();
    let encoder = TextEncoder::new();
    let mut buffer = Vec::new();
    encoder.encode(&metrics, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
