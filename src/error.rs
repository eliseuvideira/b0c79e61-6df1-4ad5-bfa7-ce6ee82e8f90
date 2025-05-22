use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    RabbitMQ(#[from] lapin::Error),
    #[error(transparent)]
    Unknown(#[from] anyhow::Error),
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    message: String,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Error::Io(_) | Error::Unknown(_) | Error::Sqlx(_) | Error::RabbitMQ(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    message: "Internal Server Error".to_string(),
                }),
            )
                .into_response(),
        }
    }
}
