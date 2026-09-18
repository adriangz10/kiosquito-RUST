use crate::errors::AppError;
use crate::models::provider::{CreateProvider, Provider, UpdateProvider};
use axum::{
    extract::{Path, State},
    Json,
};
use sqlx::PgPool;

pub async fn list(State(pool): State<PgPool>) -> Result<Json<Vec<Provider>>, AppError> {
    let providers = sqlx::query_as::<_, Provider>(
        r#"SELECT id, name, phone, direction, "createdAt", "updatedAt"
           FROM providers ORDER BY id"#,
    )
    .fetch_all(&pool)
    .await?;

    Ok(Json(providers))
}

pub async fn get_one(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
) -> Result<Json<Provider>, AppError> {
    let provider = sqlx::query_as::<_, Provider>(
        r#"SELECT id, name, phone, direction, "createdAt", "updatedAt"
           FROM providers WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(provider))
}

pub async fn create(
    State(pool): State<PgPool>,
    Json(payload): Json<CreateProvider>,
) -> Result<Json<Provider>, AppError> {
    let provider = sqlx::query_as::<_, Provider>(
        r#"INSERT INTO providers (name, phone, direction, "createdAt", "updatedAt")
           VALUES ($1, $2, $3, now(), now())
           RETURNING id, name, phone, direction, "createdAt", "updatedAt""#,
    )
    .bind(payload.name)
    .bind(payload.phone)
    .bind(payload.direction)
    .fetch_one(&pool)
    .await?;

    Ok(Json(provider))
}

pub async fn update(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
    Json(payload): Json<UpdateProvider>,
) -> Result<Json<Provider>, AppError> {
    let provider = sqlx::query_as::<_, Provider>(
        r#"UPDATE providers SET
             name = COALESCE($1, name),
             phone = COALESCE($2, phone),
             direction = COALESCE($3, direction),
             "updatedAt" = now()
           WHERE id = $4
           RETURNING id, name, phone, direction, "createdAt", "updatedAt""#,
    )
    .bind(payload.name)
    .bind(payload.phone)
    .bind(payload.direction)
    .bind(id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(provider))
}

pub async fn delete(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
) -> Result<axum::http::StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM providers WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}
