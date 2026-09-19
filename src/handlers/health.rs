// src/handlers/health.rs
use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};
use sqlx::PgPool;

pub async fn check(State(pool): State<PgPool>) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .map(|_| Json(json!({ "status": "ok" })))
        .map_err(|_| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "status": "database_unavailable" })),
            )
        })
}
