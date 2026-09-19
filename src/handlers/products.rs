use crate::errors::AppError;
use crate::extractors::{AdminUser, AuthUser};
use crate::models::product::{CreateProduct, Product, UpdateProduct};
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};
use sqlx::PgPool;

#[derive(sqlx::FromRow)]
struct ProductRelationRow {
    id: i32,
    name: String,
    price: rust_decimal::Decimal,
    stock: Option<i32>,
    category_id: i32,
    provider_id: Option<i32>,
    barcode: Option<String>,
    cost_price: rust_decimal::Decimal,
    #[sqlx(rename = "createdAt")]
    created_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(rename = "updatedAt")]
    updated_at: chrono::DateTime<chrono::Utc>,
    category_name: String,
    provider_name: Option<String>,
    provider_phone: Option<String>,
}

pub async fn list(
    State(pool): State<PgPool>,
    AuthUser(_user): AuthUser,
) -> Result<Json<Vec<Value>>, AppError> {
    let products = sqlx::query_as::<_, ProductRelationRow>(
        r#"SELECT p.id, p.name, p.price, p.stock, p.category_id, p.provider_id, p.barcode, p.cost_price,
                  p."createdAt", p."updatedAt", c.name AS category_name, pr.name AS provider_name, pr.phone AS provider_phone
           FROM products p JOIN categories c ON c.id = p.category_id
           LEFT JOIN providers pr ON pr.id = p.provider_id ORDER BY p.name"#,
    )
    .fetch_all(&pool)
    .await?;

    Ok(Json(products.into_iter().map(|product| json!({
        "id": product.id, "name": product.name, "price": product.price, "stock": product.stock,
        "category_id": product.category_id, "provider_id": product.provider_id,
        "barcode": product.barcode, "cost_price": product.cost_price,
        "createdAt": product.created_at, "updatedAt": product.updated_at,
        "category": {"id": product.category_id, "name": product.category_name},
        "provider": product.provider_name.map(|name| json!({"id": product.provider_id, "name": name, "phone": product.provider_phone}))
    })).collect()))
}

pub async fn get_one(
    State(pool): State<PgPool>,
    AuthUser(_user): AuthUser,
    Path(id): Path<i32>,
) -> Result<Json<Value>, AppError> {
    let product = sqlx::query_as::<_, ProductRelationRow>(
        r#"SELECT p.id, p.name, p.price, p.stock, p.category_id, p.provider_id, p.barcode, p.cost_price,
                  p."createdAt", p."updatedAt", c.name AS category_name, pr.name AS provider_name, pr.phone AS provider_phone
           FROM products p JOIN categories c ON c.id=p.category_id LEFT JOIN providers pr ON pr.id=p.provider_id WHERE p.id=$1"#,
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(
        json!({"id":product.id,"name":product.name,"price":product.price,"stock":product.stock,"category_id":product.category_id,"provider_id":product.provider_id,"barcode":product.barcode,"cost_price":product.cost_price,"createdAt":product.created_at,"updatedAt":product.updated_at,"category":{"id":product.category_id,"name":product.category_name},"provider":product.provider_name.map(|name| json!({"id":product.provider_id,"name":name,"phone":product.provider_phone}))}),
    ))
}

pub async fn get_by_barcode(
    State(pool): State<PgPool>,
    AuthUser(_user): AuthUser,
    Path(barcode): Path<String>,
) -> Result<Json<Value>, AppError> {
    let product = sqlx::query_as::<_, ProductRelationRow>(
        r#"SELECT p.id, p.name, p.price, p.stock, p.category_id, p.provider_id, p.barcode, p.cost_price,
                  p."createdAt", p."updatedAt", c.name AS category_name, pr.name AS provider_name, pr.phone AS provider_phone
           FROM products p JOIN categories c ON c.id=p.category_id LEFT JOIN providers pr ON pr.id=p.provider_id WHERE p.barcode=$1"#,
    )
    .bind(barcode)
    .fetch_one(&pool)
    .await?;

    Ok(Json(
        json!({"id":product.id,"name":product.name,"price":product.price,"stock":product.stock,"category_id":product.category_id,"provider_id":product.provider_id,"barcode":product.barcode,"cost_price":product.cost_price,"createdAt":product.created_at,"updatedAt":product.updated_at,"category":{"id":product.category_id,"name":product.category_name},"provider":product.provider_name.map(|name| json!({"id":product.provider_id,"name":name,"phone":product.provider_phone}))}),
    ))
}

pub async fn create(
    State(pool): State<PgPool>,
    AdminUser(_user): AdminUser,
    Json(payload): Json<CreateProduct>,
) -> Result<(axum::http::StatusCode, Json<Product>), AppError> {
    if payload.price.is_sign_negative()
        || payload
            .cost_price
            .is_some_and(|cost| cost.is_sign_negative())
        || payload.stock.is_some_and(|stock| stock < 0)
    {
        return Err(AppError::BadRequest(
            "Los precios y el stock no pueden ser negativos".into(),
        ));
    }

    let product = sqlx::query_as::<_, Product>(
        r#"INSERT INTO products (name, price, stock, category_id, provider_id, barcode, cost_price, "createdAt", "updatedAt")
           VALUES ($1, $2, COALESCE($3, 0), $4, $5, $6, COALESCE($7, 0), now(), now())
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

    Ok((axum::http::StatusCode::CREATED, Json(product)))
}

pub async fn update(
    State(pool): State<PgPool>,
    AdminUser(_user): AdminUser,
    Path(id): Path<i32>,
    Json(payload): Json<UpdateProduct>,
) -> Result<Json<serde_json::Value>, AppError> {
    if payload.price.is_some_and(|price| price.is_sign_negative())
        || payload
            .cost_price
            .is_some_and(|cost| cost.is_sign_negative())
        || payload.stock.is_some_and(|stock| stock < 0)
    {
        return Err(AppError::BadRequest(
            "Los precios y el stock no pueden ser negativos".into(),
        ));
    }

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

    Ok(Json(json!({
        "message": "Producto actualizado con éxito.",
        "product": product
    })))
}

pub async fn delete(
    State(pool): State<PgPool>,
    AdminUser(_user): AdminUser,
    Path(id): Path<i32>,
) -> Result<(axum::http::StatusCode, Json<serde_json::Value>), AppError> {
    let result = sqlx::query("DELETE FROM products WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok((
        axum::http::StatusCode::OK,
        Json(json!({"message": "Producto eliminado correctamente."})),
    ))
}
