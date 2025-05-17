use anyhow::{Context, Result};
use axum::{
    http::StatusCode,
    routing::{get, Router},
    serve::Serve,
};
use tokio::net::TcpListener;

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
