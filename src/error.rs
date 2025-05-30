use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    InvalidInput(String),
    #[error("{0}")]
    NotFound(String),
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
            Error::InvalidInput(message) => {
                (StatusCode::BAD_REQUEST, Json(ErrorResponse { message })).into_response()
            }
            Error::NotFound(message) => {
                (StatusCode::NOT_FOUND, Json(ErrorResponse { message })).into_response()
            }
            Error::Io(_) | Error::Unknown(_) | Error::Sqlx(_) | Error::RabbitMQ(_) => {
                tracing::error!(
                    error = ?self,
                    "API Error"
                );

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        message: "Internal Server Error".to_string(),
                    }),
                )
                    .into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use http::HeaderValue;

    use super::*;

    #[test]
    fn test_error_response() {
        // Arrange
        let error = Error::InvalidInput("Invalid input".to_string());

        // Act
        let response = error.into_response();

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.headers().get("Content-Type"),
            Some(&HeaderValue::from_static("application/json"))
        );
    }

    #[test]
    fn test_error_response_not_found() {
        // Arrange
        let error = Error::NotFound("Not found".to_string());

        // Act
        let response = error.into_response();

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.headers().get("Content-Type"),
            Some(&HeaderValue::from_static("application/json"))
        );
    }

    #[test]
    fn test_error_response_internal_server_error() {
        // Arrange
        let error = Error::Unknown(anyhow::anyhow!("Unknown error"));

        // Act
        let response = error.into_response();

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response.headers().get("Content-Type"),
            Some(&HeaderValue::from_static("application/json"))
        );
    }
}
