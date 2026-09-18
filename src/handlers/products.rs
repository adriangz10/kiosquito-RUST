use crate::errors::AppError;
use crate::models::product::{CreateProduct, Product, UpdateProduct};
use axum::{
    extract::{Path, State},
    Json,
};
use sqlx::PgPool;

pub async fn list(State(pool): State<PgPool>) -> Result<Json<Vec<Product>>, AppError> {
    let products = sqlx::query_as::<_, Product>(
        r#"SELECT id, name, price, stock, category_id, provider_id, barcode, cost_price,
                  "createdAt", "updatedAt"
           FROM products ORDER BY id"#,
    )
    .fetch_all(&pool)
    .await?;

    Ok(Json(products))
}

pub async fn get_one(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
) -> Result<Json<Product>, AppError> {
    let product = sqlx::query_as::<_, Product>(
        r#"SELECT id, name, price, stock, category_id, provider_id, barcode, cost_price,
                  "createdAt", "updatedAt"
           FROM products WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(product))
}

pub async fn create(
    State(pool): State<PgPool>,
    Json(payload): Json<CreateProduct>,
) -> Result<Json<Product>, AppError> {
    let product = sqlx::query_as::<_, Product>(
        r#"INSERT INTO products (name, price, stock, category_id, provider_id, barcode, cost_price, "createdAt", "updatedAt")
           VALUES ($1, $2, $3, $4, $5, $6, $7, now(), now())
           RETURNING id, name, price, stock, category_id, provider_id, barcode, cost_price,
                     "createdAt", "updatedAt""#
    )
    .bind(payload.name)
    .bind(payload.price)
    .bind(payload.stock)
    .bind(payload.category_id)
    .bind(payload.provider_id)
    .bind(payload.barcode)
    .bind(payload.cost_price)
    .fetch_one(&pool)
    .await?;

    Ok(Json(product))
}

pub async fn update(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
    Json(payload): Json<UpdateProduct>,
) -> Result<Json<Product>, AppError> {
    let product = sqlx::query_as::<_, Product>(
        r#"UPDATE products SET
             name = COALESCE($1, name),
             price = COALESCE($2, price),
             stock = COALESCE($3, stock),
             category_id = COALESCE($4, category_id),
             provider_id = COALESCE($5, provider_id),
             barcode = COALESCE($6, barcode),
             cost_price = COALESCE($7, cost_price),
             "updatedAt" = now()
           WHERE id = $8
           RETURNING id, name, price, stock, category_id, provider_id, barcode, cost_price,
                     "createdAt", "updatedAt""#,
    )
    .bind(payload.name)
    .bind(payload.price)
    .bind(payload.stock)
    .bind(payload.category_id)
    .bind(payload.provider_id)
    .bind(payload.barcode)
    .bind(payload.cost_price)
    .bind(id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(product))
}

pub async fn delete(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
) -> Result<axum::http::StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM products WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}
