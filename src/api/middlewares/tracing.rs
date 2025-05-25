use axum::{extract::Request, middleware::Next, response::Response};
use opentelemetry::trace::TraceContextExt;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub async fn attach_trace_id(req: Request, next: Next) -> Response {
    let span = Span::current();
    let context = span.context();
    let otel_context = context.span().span_context().clone();
    if otel_context.is_valid() {
        let trace_id = otel_context.trace_id().to_string();
        span.record("trace_id", trace_id);
    }

    next.run(req).await
}
