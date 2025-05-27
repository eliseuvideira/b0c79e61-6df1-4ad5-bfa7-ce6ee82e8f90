use std::sync::Arc;

use axum::{
    extract::{MatchedPath, Request, State},
    middleware::Next,
    response::Response,
};
use http::Method;
use tokio::time::Instant;

use crate::telemetry::Metrics;

fn get_method(req: &Request) -> &'static str {
    let method = req.method();
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

fn get_endpoint(req: &Request) -> String {
    let exact_endpoint = req.uri().path().to_string();
    let endpoint = req
        .extensions()
        .get::<MatchedPath>()
        .map_or(exact_endpoint, |matched_path| {
            matched_path.as_str().to_string()
        });

    endpoint
}

pub async fn record_metrics(
    State(metrics): State<Arc<Metrics>>,
    req: Request,
    next: Next,
) -> Response {
    let method = get_method(&req);
    let endpoint = get_endpoint(&req);

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

#[cfg(test)]
mod tests {
    use axum::body::Body;

    use super::*;

    #[tokio::test]
    async fn test_get_method_returns_method() {
        for (method, expected) in [
            (Method::GET, "GET"),
            (Method::POST, "POST"),
            (Method::PUT, "PUT"),
            (Method::DELETE, "DELETE"),
            (Method::HEAD, "HEAD"),
            (Method::OPTIONS, "OPTIONS"),
        ] {
            let req = Request::builder()
                .method(method)
                .body(Body::from(()))
                .unwrap();
            assert_eq!(get_method(&req), expected);
        }
    }
}
