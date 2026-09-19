use crate::auth::hash_password;
use crate::errors::AppError;
use crate::extractors::AdminUser;
use crate::models::user::{RegisterUser, Role, UpdateUser};
use crate::models::user::{User, UserPublic};
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};
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

pub async fn create(
    State(pool): State<PgPool>,
    AdminUser(_claims): AdminUser,
    Json(payload): Json<RegisterUser>,
) -> Result<(axum::http::StatusCode, Json<Value>), AppError> {
    if payload.password.chars().count() < 8 || payload.username.trim().is_empty() {
        return Err(AppError::BadRequest(
            "El usuario requiere nombre y password de al menos 8 caracteres".into(),
        ));
    }

    let password = hash_password(&payload.password).map_err(|_| AppError::Internal)?;
    let user = sqlx::query_as::<_, User>(
        r#"INSERT INTO users (username, role, password, "createdAt", "updatedAt")
           VALUES ($1, $2, $3, now(), now())
           RETURNING id, username, role, password, "createdAt", "updatedAt""#,
    )
    .bind(payload.username)
    .bind(payload.role)
    .bind(password)
    .fetch_one(&pool)
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(json!({"message":"Usuario creado","user": UserPublic {
            id: user.id,
            username: user.username,
            role: user.role,
        }})),
    ))
}

pub async fn update(
    State(pool): State<PgPool>,
    AdminUser(claims): AdminUser,
    Path(id): Path<i32>,
    Json(payload): Json<UpdateUser>,
) -> Result<Json<Value>, AppError> {
    if payload
        .password
        .as_ref()
        .is_some_and(|password| password.chars().count() < 8)
    {
        return Err(AppError::BadRequest(
            "El password debe tener al menos 8 caracteres".into(),
        ));
    }
    if claims.sub == id && payload.role == Some(Role::Empleado) {
        return Err(AppError::BadRequest(
            "No puedes degradar tu propia cuenta".into(),
        ));
    }
    if payload.role == Some(Role::Empleado) {
        let target_role: Option<Role> = sqlx::query_scalar("SELECT role FROM users WHERE id=$1")
            .bind(id)
            .fetch_optional(&pool)
            .await?;
        let admins: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role='Administrador'")
                .fetch_one(&pool)
                .await?;
        if target_role == Some(Role::Administrador) && admins <= 1 {
            return Err(AppError::BadRequest(
                "Debe existir al menos un administrador".into(),
            ));
        }
    }

    let password = payload
        .password
        .map(|value| hash_password(&value))
        .transpose()
        .map_err(|_| AppError::Internal)?;
    let user = sqlx::query_as::<_, User>(
        r#"UPDATE users SET
             username = COALESCE($1, username),
             role = COALESCE($2, role),
             password = COALESCE($3, password),
             "updatedAt" = now()
           WHERE id = $4
           RETURNING id, username, role, password, "createdAt", "updatedAt""#,
    )
    .bind(payload.username)
    .bind(payload.role)
    .bind(password)
    .bind(id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(
        json!({"message":"Usuario actualizado","user": UserPublic {
            id: user.id,
            username: user.username,
            role: user.role,
        }}),
    ))
}

// Solo Administrador puede borrar usuarios
pub async fn delete(
    State(pool): State<PgPool>,
    AdminUser(claims): AdminUser,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, AppError> {
    if claims.sub == id {
        return Err(AppError::BadRequest(
            "No puedes eliminar tu propia cuenta".into(),
        ));
    }

    let admins: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role = 'Administrador'")
        .fetch_one(&pool)
        .await?;
    let target_role: Option<Role> = sqlx::query_scalar("SELECT role FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await?;
    if target_role == Some(Role::Administrador) && admins <= 1 {
        return Err(AppError::BadRequest(
            "Debe existir al menos un administrador".into(),
        ));
    }

    let associated: i64 = sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM sales WHERE user_id=$1)+(SELECT COUNT(*) FROM purchases WHERE user_id=$1)+(SELECT COUNT(*) FROM expenses WHERE user_id=$1)+(SELECT COUNT(*) FROM cash_shifts WHERE user_id=$1)+(SELECT COUNT(*) FROM customer_accounts WHERE user_id=$1)")
        .bind(id).fetch_one(&pool).await?;
    if associated > 0 {
        return Err(AppError::BadRequest(
            "No se puede eliminar un usuario con operaciones asociadas".into(),
        ));
    }
    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(json!({ "message": "Usuario eliminado." })))
}
