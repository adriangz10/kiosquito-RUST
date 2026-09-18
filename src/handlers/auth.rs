use crate::auth::{create_jwt, hash_password, verify_password};
use crate::errors::AppError;
use crate::models::user::{LoginRequest, LoginResponse, RegisterUser, User, UserPublic};
use axum::{extract::State, Json};
use sqlx::PgPool;

pub async fn register(
    State(pool): State<PgPool>,
    Json(payload): Json<RegisterUser>,
) -> Result<Json<UserPublic>, AppError> {
    let hashed = hash_password(&payload.password).map_err(|_| AppError::Internal)?;

    let user = sqlx::query_as::<_, User>(
        r#"INSERT INTO users (username, role, password, "createdAt", "updatedAt")
           VALUES ($1, $2, $3, now(), now())
           RETURNING id, username, role, password, "createdAt", "updatedAt""#,
    )
    .bind(payload.username)
    .bind(payload.role)
    .bind(hashed)
    .fetch_one(&pool)
    .await?;

    Ok(Json(UserPublic {
        id: user.id,
        username: user.username,
        role: user.role,
    }))
}

pub async fn login(
    State(pool): State<PgPool>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let user = sqlx::query_as::<_, User>(
        r#"SELECT id, username, role, password, "createdAt", "updatedAt"
           FROM users WHERE username = $1"#,
    )
    .bind(&payload.username)
    .fetch_optional(&pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    if !verify_password(&payload.password, &user.password) {
        return Err(AppError::Unauthorized);
    }

    let token = create_jwt(user.id, &user.username, user.role).map_err(|_| AppError::Internal)?;

    Ok(Json(LoginResponse { token }))
}
