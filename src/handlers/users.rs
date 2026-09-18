use crate::errors::AppError;
use crate::extractors::AdminUser;
use crate::models::user::{User, UserPublic};
use axum::{
    extract::{Path, State},
    Json,
};
use sqlx::PgPool;

// Solo Administrador puede listar usuarios
pub async fn list(
    State(pool): State<PgPool>,
    AdminUser(_claims): AdminUser,
) -> Result<Json<Vec<UserPublic>>, AppError> {
    let users = sqlx::query_as::<_, User>(
        r#"SELECT id, username, role, password, "createdAt", "updatedAt" FROM users ORDER BY id"#,
    )
    .fetch_all(&pool)
    .await?;

    let public: Vec<UserPublic> = users
        .into_iter()
        .map(|u| UserPublic {
            id: u.id,
            username: u.username,
            role: u.role,
        })
        .collect();

    Ok(Json(public))
}

// Solo Administrador puede borrar usuarios
pub async fn delete(
    State(pool): State<PgPool>,
    AdminUser(_claims): AdminUser,
    Path(id): Path<i32>,
) -> Result<axum::http::StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}
