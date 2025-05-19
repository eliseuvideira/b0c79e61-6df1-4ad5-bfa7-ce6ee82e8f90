use anyhow::{Context, Result};
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::{from_fn, Next},
    response::Response,
    routing::{get, Router},
    serve::Serve,
};
use axum_tracing_opentelemetry::{
    middleware::{OtelAxumLayer, OtelInResponseLayer},
    tracing_opentelemetry_instrumentation_sdk::find_current_trace_id,
};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{debug_span, Instrument};

use crate::{config::Settings, error::Error};

pub struct Application {
    port: u16,
    server: Serve<Router, Router>,
}

impl Application {
    pub async fn build(configuration: Settings) -> Result<Self> {
        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );
        let listener = TcpListener::bind(&address)
            .await
            .context("Failed to bind address")?;
        let port = listener
            .local_addr()
            .context("Failed to get local address")?
            .port();

        let router = Router::new()
            .layer(TraceLayer::new_for_http())
            .layer(from_fn(attach_trace_id))
            .layer(OtelInResponseLayer)
            .layer(OtelAxumLayer::default())
            .route("/health", get(health_check))
            .fallback(not_found);

        let server = axum::serve(listener, router);

        Ok(Self { port, server })
    }

    pub async fn run_until_stopped(self) -> Result<()> {
        self.server.await?;

        Ok(())
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

async fn health_check() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn not_found() -> Error {
    Error::NotFound
}

async fn attach_trace_id(req: Request, next: Next) -> Response {
    let trace_id = find_current_trace_id();

    let response = next
        .run(req)
        .instrument(debug_span!(
            "trace_id",
            trace_id = ?trace_id,
        ))
        .await;

    response
}
