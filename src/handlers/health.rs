// src/handlers/health.rs
use axum::{extract::State, Json};
use serde_json::{json, Value};
use sqlx::PgPool;

pub async fn check(State(_pool): State<PgPool>) -> Json<Value> {
    Json(json!({ "status": "Base de datos conectada" }))
}
