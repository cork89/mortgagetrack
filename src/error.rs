use askama::Error as AskamaError;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Database(String),

    #[error(transparent)]
    Template(#[from] AskamaError),

    #[error("{0}")]
    BadRequest(String),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    Conflict(String),

    #[error("{0}")]
    TooManyRequests(String),

    #[error("{0}")]
    Internal(String),
}

impl From<libsql::Error> for AppError {
    fn from(err: libsql::Error) -> Self {
        Self::Database(err.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::TooManyRequests(msg) => (StatusCode::TOO_MANY_REQUESTS, msg.clone()),
            AppError::Internal(msg) => {
                tracing::error!(error = %msg, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            AppError::Database(err) => {
                tracing::error!(error = %err, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error".to_string(),
                )
            }
            AppError::Template(err) => {
                tracing::error!(error = %err, "template error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Template error".to_string(),
                )
            }
        };

        (status, message).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
