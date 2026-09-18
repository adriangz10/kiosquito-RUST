use crate::errors::AppError;
use crate::models::category::{Category, CreateCategory, UpdateCategory};
use axum::{
    extract::{Path, State},
    Json,
};
use sqlx::PgPool;

pub async fn list(State(pool): State<PgPool>) -> Result<Json<Vec<Category>>, AppError> {
    let categories = sqlx::query_as::<_, Category>(
        r#"SELECT id, name, "createdAt", "updatedAt" FROM categories ORDER BY id"#,
    )
    .fetch_all(&pool)
    .await?;

    Ok(Json(categories))
}

pub async fn get_one(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
) -> Result<Json<Category>, AppError> {
    let category = sqlx::query_as::<_, Category>(
        r#"SELECT id, name, "createdAt", "updatedAt" FROM categories WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(category))
}

pub async fn create(
    State(pool): State<PgPool>,
    Json(payload): Json<CreateCategory>,
) -> Result<Json<Category>, AppError> {
    let category = sqlx::query_as::<_, Category>(
        r#"INSERT INTO categories (name, "createdAt", "updatedAt")
           VALUES ($1, now(), now())
           RETURNING id, name, "createdAt", "updatedAt""#,
    )
    .bind(payload.name)
    .fetch_one(&pool)
    .await?;

    Ok(Json(category))
}

pub async fn update(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
    Json(payload): Json<UpdateCategory>,
) -> Result<Json<Category>, AppError> {
    let category = sqlx::query_as::<_, Category>(
        r#"UPDATE categories SET
             name = COALESCE($1, name),
             "updatedAt" = now()
           WHERE id = $2
           RETURNING id, name, "createdAt", "updatedAt""#,
    )
    .bind(payload.name)
    .bind(id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(category))
}

pub async fn delete(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
) -> Result<axum::http::StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM categories WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}
