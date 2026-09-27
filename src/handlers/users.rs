use crate::auth::{hash_password, verify_password};
use crate::errors::AppError;
use crate::extractors::AdminUser;
use crate::models::user::{RegisterUser, Role, UpdateUser};
use crate::models::user::{User, UserPublic};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;

#[derive(Debug, Deserialize)]
pub struct VerifyPinRequest {
    pub pin: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyPinResponse {
    pub valid: bool,
    pub supervisor_id: Option<i32>,
    pub supervisor_name: Option<String>,
}

// Solo Administrador o Dueño puede listar usuarios
pub async fn list(
    State(pool): State<PgPool>,
    AdminUser(_claims): AdminUser,
) -> Result<Json<Vec<UserPublic>>, AppError> {
    let users = sqlx::query_as::<_, User>(
        r#"SELECT id, username, role, password, pin_supervisor, "createdAt", "updatedAt" FROM users ORDER BY id"#,
    )
    .fetch_all(&pool)
    .await?;

    let public: Vec<UserPublic> = users
        .into_iter()
        .map(|u| UserPublic {
            id: u.id,
            username: u.username,
            role: u.role,
            has_supervisor_pin: u.pin_supervisor.is_some(),
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
    let pin_hash = if let Some(pin) = payload.pin_supervisor {
        if !pin.trim().is_empty() {
            Some(hash_password(pin.trim()).map_err(|_| AppError::Internal)?)
        } else {
            None
        }
    } else {
        None
    };

    let user = sqlx::query_as::<_, User>(
        r#"INSERT INTO users (username, role, password, pin_supervisor, "createdAt", "updatedAt")
           VALUES ($1, $2, $3, $4, now(), now())
           RETURNING id, username, role, password, pin_supervisor, "createdAt", "updatedAt""#,
    )
    .bind(payload.username)
    .bind(payload.role)
    .bind(password)
    .bind(pin_hash)
    .fetch_one(&pool)
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(json!({"message":"Usuario creado","user": UserPublic {
            id: user.id,
            username: user.username,
            role: user.role,
            has_supervisor_pin: user.pin_supervisor.is_some(),
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
    if claims.sub == id && payload.role.as_ref().is_some_and(|r| !r.is_admin_or_owner()) {
        return Err(AppError::BadRequest(
            "No puedes degradar tu propia cuenta".into(),
        ));
    }
    if payload.role.as_ref().is_some_and(|r| !r.is_admin_or_owner()) {
        let target_role: Option<Role> = sqlx::query_scalar("SELECT role FROM users WHERE id=$1")
            .bind(id)
            .fetch_optional(&pool)
            .await?;
        let admins: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role='Administrador' OR role='Dueño'")
                .fetch_one(&pool)
                .await?;
        if target_role.as_ref().is_some_and(|r| r.is_admin_or_owner()) && admins <= 1 {
            return Err(AppError::BadRequest(
                "Debe existir al menos un administrador o dueño".into(),
            ));
        }
    }

    let password = payload
        .password
        .map(|value| hash_password(&value))
        .transpose()
        .map_err(|_| AppError::Internal)?;

    let pin_hash = if let Some(pin) = payload.pin_supervisor {
        if !pin.trim().is_empty() {
            Some(hash_password(pin.trim()).map_err(|_| AppError::Internal)?)
        } else {
            None
        }
    } else {
        None
    };

    let user = sqlx::query_as::<_, User>(
        r#"UPDATE users SET
             username = COALESCE($1, username),
             role = COALESCE($2, role),
             password = COALESCE($3, password),
             pin_supervisor = COALESCE($4, pin_supervisor),
             "updatedAt" = now()
           WHERE id = $5
           RETURNING id, username, role, password, pin_supervisor, "createdAt", "updatedAt""#,
    )
    .bind(payload.username)
    .bind(payload.role)
    .bind(password)
    .bind(pin_hash)
    .bind(id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(
        json!({"message":"Usuario actualizado","user": UserPublic {
            id: user.id,
            username: user.username,
            role: user.role,
            has_supervisor_pin: user.pin_supervisor.is_some(),
        }}),
    ))
}

// Solo Administrador o Dueño puede borrar usuarios
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

    let admins: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role = 'Administrador' OR role = 'Dueño'")
        .fetch_one(&pool)
        .await?;
    let target_role: Option<Role> = sqlx::query_scalar("SELECT role FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await?;
    if target_role.as_ref().is_some_and(|r| r.is_admin_or_owner()) && admins <= 1 {
        return Err(AppError::BadRequest(
            "Debe existir al menos un administrador o dueño".into(),
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

// Endpoint para validar PIN de supervisor en POS para anulaciones (BR-POS-003)
pub async fn verify_supervisor_pin(
    State(pool): State<PgPool>,
    Json(payload): Json<VerifyPinRequest>,
) -> Result<Json<VerifyPinResponse>, AppError> {
    let pin = payload.pin.trim();
    if pin.is_empty() {
        return Ok(Json(VerifyPinResponse {
            valid: false,
            supervisor_id: None,
            supervisor_name: None,
        }));
    }

    let supervisors = sqlx::query_as::<_, (i32, String, String)>(
        r#"SELECT id, username, pin_supervisor FROM users 
           WHERE (role = 'Administrador' OR role = 'Dueño' OR role = 'Encargado') 
             AND pin_supervisor IS NOT NULL"#,
    )
    .fetch_all(&pool)
    .await?;

    for (id, username, hash) in supervisors {
        if verify_password(pin, &hash) {
            return Ok(Json(VerifyPinResponse {
                valid: true,
                supervisor_id: Some(id),
                supervisor_name: Some(username),
            }));
        }
    }

    Ok(Json(VerifyPinResponse {
        valid: false,
        supervisor_id: None,
        supervisor_name: None,
    }))
}
