use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, sqlx::Type, Serialize, Deserialize, Clone, PartialEq)]
#[sqlx(type_name = "enum_users_role")]
pub enum Role {
    Administrador,
    Empleado,
}

#[derive(Debug, FromRow)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub role: Role,
    pub password: String, // hash, nunca se serializa al cliente
    #[allow(dead_code)]
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[allow(dead_code)]
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserPublic {
    pub id: i32,
    pub username: String,
    pub role: Role,
}

#[derive(Debug, Deserialize)]
pub struct RegisterUser {
    pub username: String,
    pub password: String,
    pub role: Role,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
}
