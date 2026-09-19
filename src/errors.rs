use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

pub enum AppError {
    NotFound,
    #[allow(dead_code)]
    Unauthorized,
    BadRequest(String),
    TooManyRequests(String),
    Internal,
    Database(sqlx::Error),
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::NotFound,
            other => AppError::Database(other),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "No encontrado".to_string()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "No autorizado".to_string()),
            AppError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            AppError::TooManyRequests(message) => (StatusCode::TOO_MANY_REQUESTS, message),
            AppError::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error interno".to_string(),
            ),
            AppError::Database(e) => {
                tracing::error!("DB error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Error interno".to_string(),
                )
            }
        };
        (status, Json(json!({ "message": message }))).into_response()
    }
}
