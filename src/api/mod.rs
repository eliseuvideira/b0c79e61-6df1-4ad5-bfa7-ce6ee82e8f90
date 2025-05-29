use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Result};
use axum::{
    middleware::{from_fn, from_fn_with_state},
    routing::get,
    serve::Serve,
    Router,
};
use axum_tracing_opentelemetry::middleware::{OtelAxumLayer, OtelInResponseLayer};
use lapin::Connection;
use reqwest::StatusCode;
use sqlx::{Pool, Postgres};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use types::AppState;

use crate::{config::Config, telemetry::Metrics};

mod middlewares;
mod routes;
pub mod types;

pub struct Api {
    port: u16,
    server: Serve<Router, Router>,
}

impl Api {
    pub async fn build(
        configuration: &Config,
        db_pool: Pool<Postgres>,
        rabbitmq_connection: Arc<Connection>,
        integration_queues: HashMap<String, String>,
        metrics: Arc<Metrics>,
    ) -> Result<Self> {
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

        let app_state = Arc::new(AppState {
            db_pool: db_pool.clone(),
            rabbitmq_connection,
            integration_queues,
            exchange_name: configuration.rabbitmq.exchange_name.clone(),
        });

        let router = Router::new()
            .merge(routes::jobs::create_router(app_state.clone()))
            .merge(routes::openapi::create_router())
            .layer(TraceLayer::new_for_http())
            .layer(from_fn(middlewares::tracing::attach_trace_id))
            .layer(from_fn_with_state(
                metrics.clone(),
                middlewares::record_metrics,
            ))
            .layer(OtelInResponseLayer)
            .layer(OtelAxumLayer::default())
            .merge(routes::metrics::create_router(metrics.clone()))
            .route("/health", get(health_check))
            .fallback(not_found);

        let server = axum::serve(listener, router);

        Ok(Self { port, server })
    }

    pub async fn run_until_stopped(self) -> Result<()> {
        self.server.await.context("Server failed to start")
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

async fn not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

async fn health_check() -> StatusCode {
    StatusCode::NO_CONTENT
}
