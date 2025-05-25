use std::sync::Arc;

use axum::{
    extract::{MatchedPath, Request, State},
    middleware::Next,
    response::Response,
};
use http::Method;
use tokio::time::Instant;

use crate::telemetry::Metrics;

fn get_method(method: &Method) -> &'static str {
    match *method {
        Method::OPTIONS => "OPTIONS",
        Method::GET => "GET",
        Method::POST => "POST",
        Method::PUT => "PUT",
        Method::DELETE => "DELETE",
        Method::HEAD => "HEAD",
        Method::TRACE => "TRACE",
        Method::CONNECT => "CONNECT",
        Method::PATCH => "PATCH",
        _ => "",
    }
}

fn get_labels(req: &Request) -> (&'static str, String) {
    let exact_endpoint = req.uri().path().to_string();
    let endpoint = req
        .extensions()
        .get::<MatchedPath>()
        .map_or(exact_endpoint, |matched_path| {
            matched_path.as_str().to_string()
        });
    let method = get_method(req.method());

    (method, endpoint)
}

pub async fn record_metrics(
    State(metrics): State<Arc<Metrics>>,
    req: Request,
    next: Next,
) -> Response {
    let (method, endpoint) = get_labels(&req);

    metrics.http_requests_pending(method, &endpoint).inc();

    let start = Instant::now();
    let response = next.run(req).await;

    let status_code = response.status().as_u16().to_string();
    let duration_seconds = start.elapsed().as_secs_f64();

    metrics.http_requests_pending(method, &endpoint).dec();
    metrics.http_requests_pending(method, &endpoint).dec();
    metrics
        .http_requests_total(method, &endpoint, &status_code)
        .inc();
    metrics
        .http_requests_duration_seconds(method, &endpoint, &status_code)
        .observe(duration_seconds);

    response
}
