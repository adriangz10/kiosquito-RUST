use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

pub enum AppError {
    NotFound,
    Unauthorized,
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
            AppError::NotFound => (StatusCode::NOT_FOUND, "No encontrado"),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "No autorizado"),
            AppError::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "Error interno"),
            AppError::Database(e) => {
                tracing::error!("DB error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Error interno")
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
