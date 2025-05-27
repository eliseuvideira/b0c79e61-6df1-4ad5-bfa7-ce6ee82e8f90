use axum::{extract::Request, middleware::Next, response::Response};

use crate::telemetry::propagate_trace_id;

pub async fn attach_trace_id(req: Request, next: Next) -> Response {
    propagate_trace_id();

    next.run(req).await
}
