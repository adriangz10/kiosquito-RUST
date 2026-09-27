use crate::errors::AppError;
use crate::extractors::{AdminUser, AuthUser, SupervisorUser};
use crate::models::product::{AumentoMasivoRequest, CreateProduct, Product, UpdateProduct};
use crate::outbox::emit_event;
use axum::{
    extract::{Path, State},
    Json,
};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use sqlx::PgPool;

#[derive(sqlx::FromRow)]
struct ProductRelationRow {
    id: i32,
    name: String,
    price: rust_decimal::Decimal,
    stock: Option<rust_decimal::Decimal>,
    category_id: i32,
    provider_id: Option<i32>,
    barcode: Option<String>,
    cost_price: rust_decimal::Decimal,
    min_margin: Option<rust_decimal::Decimal>,
    brand: Option<String>,
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
                  p.min_margin, p.brand,
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
        "min_margin": product.min_margin.unwrap_or(Decimal::ZERO),
        "brand": product.brand,
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
                  p.min_margin, p.brand,
                  p."createdAt", p."updatedAt", c.name AS category_name, pr.name AS provider_name, pr.phone AS provider_phone
           FROM products p JOIN categories c ON c.id=p.category_id LEFT JOIN providers pr ON pr.id=p.provider_id WHERE p.id=$1"#,
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(
        json!({
            "id": product.id, "name": product.name, "price": product.price, "stock": product.stock,
            "category_id": product.category_id, "provider_id": product.provider_id,
            "barcode": product.barcode, "cost_price": product.cost_price,
            "min_margin": product.min_margin.unwrap_or(Decimal::ZERO),
            "brand": product.brand,
            "createdAt": product.created_at, "updatedAt": product.updated_at,
            "category": {"id": product.category_id, "name": product.category_name},
            "provider": product.provider_name.map(|name| json!({"id": product.provider_id, "name": name, "phone": product.provider_phone}))
        }),
    ))
}

pub async fn get_by_barcode(
    State(pool): State<PgPool>,
    AuthUser(_user): AuthUser,
    Path(barcode): Path<String>,
) -> Result<Json<Value>, AppError> {
    let product = sqlx::query_as::<_, ProductRelationRow>(
        r#"SELECT p.id, p.name, p.price, p.stock, p.category_id, p.provider_id, p.barcode, p.cost_price,
                  p.min_margin, p.brand,
                  p."createdAt", p."updatedAt", c.name AS category_name, pr.name AS provider_name, pr.phone AS provider_phone
           FROM products p JOIN categories c ON c.id=p.category_id LEFT JOIN providers pr ON pr.id=p.provider_id WHERE p.barcode=$1"#,
    )
    .bind(barcode)
    .fetch_one(&pool)
    .await?;

    Ok(Json(
        json!({
            "id": product.id, "name": product.name, "price": product.price, "stock": product.stock,
            "category_id": product.category_id, "provider_id": product.provider_id,
            "barcode": product.barcode, "cost_price": product.cost_price,
            "min_margin": product.min_margin.unwrap_or(Decimal::ZERO),
            "brand": product.brand,
            "createdAt": product.created_at, "updatedAt": product.updated_at,
            "category": {"id": product.category_id, "name": product.category_name},
            "provider": product.provider_name.map(|name| json!({"id": product.provider_id, "name": name, "phone": product.provider_phone}))
        }),
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
    {
        return Err(AppError::BadRequest(
            "Los precios no pueden ser negativos".into(),
        ));
    }

    let product = sqlx::query_as::<_, Product>(
        r#"INSERT INTO products (name, price, stock, category_id, provider_id, barcode, cost_price, min_margin, brand, "createdAt", "updatedAt")
           VALUES ($1, $2, COALESCE($3, 0), $4, $5, $6, COALESCE($7, 0), COALESCE($8, 0), $9, now(), now())
           RETURNING id, name, price, stock, category_id, provider_id, barcode, cost_price, min_margin, brand,
                     "createdAt", "updatedAt""#
    )
    .bind(payload.name)
    .bind(payload.price)
    .bind(payload.stock)
    .bind(payload.category_id)
    .bind(payload.provider_id)
    .bind(payload.barcode)
    .bind(payload.cost_price)
    .bind(payload.min_margin)
    .bind(payload.brand)
    .fetch_one(&pool)
    .await?;

    Ok((axum::http::StatusCode::CREATED, Json(product)))
}

pub async fn update(
    State(pool): State<PgPool>,
    AdminUser(claims): AdminUser,
    Path(id): Path<i32>,
    Json(payload): Json<UpdateProduct>,
) -> Result<Json<serde_json::Value>, AppError> {
    if payload.price.is_some_and(|price| price.is_sign_negative())
        || payload
            .cost_price
            .is_some_and(|cost| cost.is_sign_negative())
    {
        return Err(AppError::BadRequest(
            "Los precios no pueden ser negativos".into(),
        ));
    }

    let prev = sqlx::query_as::<_, (Decimal, Decimal)>(
        "SELECT cost_price, price FROM products WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let product = sqlx::query_as::<_, Product>(
        r#"UPDATE products SET
             name = COALESCE($1, name),
             price = COALESCE($2, price),
             stock = COALESCE($3, stock),
             category_id = COALESCE($4, category_id),
             provider_id = COALESCE($5, provider_id),
             barcode = COALESCE($6, barcode),
             cost_price = COALESCE($7, cost_price),
             min_margin = COALESCE($8, min_margin),
             brand = COALESCE($9, brand),
             "updatedAt" = now()
           WHERE id = $10
           RETURNING id, name, price, stock, category_id, provider_id, barcode, cost_price, min_margin, brand,
                     "createdAt", "updatedAt""#,
    )
    .bind(payload.name)
    .bind(payload.price)
    .bind(payload.stock)
    .bind(payload.category_id)
    .bind(payload.provider_id)
    .bind(payload.barcode)
    .bind(payload.cost_price)
    .bind(payload.min_margin)
    .bind(payload.brand)
    .bind(id)
    .fetch_one(&pool)
    .await?;

    // Si cambió el precio o costo, registrar en histórico (BR-PRE-001 y auditoría)
    if product.cost_price != Some(prev.0) || product.price != prev.1 {
        let _ = sqlx::query(
            r#"INSERT INTO historico_precios_costo 
               (producto_id, precio_costo_anterior, precio_costo_nuevo, precio_venta_anterior, precio_venta_nuevo, motivo, user_id, "createdAt")
               VALUES ($1, $2, $3, $4, $5, 'Actualización individual de producto', $6, now())"#
        )
        .bind(id)
        .bind(prev.0)
        .bind(product.cost_price.unwrap_or(prev.0))
        .bind(prev.1)
        .bind(product.price)
        .bind(claims.sub)
        .execute(&pool)
        .await;
    }

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

/// Contrato 3 de Especificación (Fase 7.1) y BR-PRE-001:
/// PUT /api/v1/productos/aumento-masivo (y /api/products/aumento-masivo)
pub async fn aumento_masivo(
    State(pool): State<PgPool>,
    SupervisorUser(claims): SupervisorUser,
    Json(payload): Json<AumentoMasivoRequest>,
) -> Result<Json<Value>, AppError> {
    if payload.porcentaje_aumento_costo.is_sign_negative() {
        return Err(AppError::BadRequest(
            "El porcentaje de aumento no puede ser negativo".into(),
        ));
    }

    let mut tx = pool.begin().await?;

    // Obtener los productos según el criterio
    let rows: Vec<(i32, String, Decimal, Decimal, Option<Decimal>)> = match payload.criterio.to_uppercase().as_str() {
        "POR_PROVEEDOR" => {
            let pid = payload.proveedor_id.ok_or_else(|| {
                AppError::BadRequest("Debe especificar proveedor_id para el criterio POR_PROVEEDOR".into())
            })?;
            sqlx::query_as("SELECT id, name, cost_price, price, min_margin FROM products WHERE provider_id = $1 FOR UPDATE")
                .bind(pid)
                .fetch_all(&mut *tx)
                .await?
        }
        "POR_CATEGORIA" => {
            let cid = payload.categoria_id.ok_or_else(|| {
                AppError::BadRequest("Debe especificar categoria_id para el criterio POR_CATEGORIA".into())
            })?;
            sqlx::query_as("SELECT id, name, cost_price, price, min_margin FROM products WHERE category_id = $1 FOR UPDATE")
                .bind(cid)
                .fetch_all(&mut *tx)
                .await?
        }
        "POR_MARCA" => {
            let marca = payload.marca.as_deref().unwrap_or("").trim();
            if marca.is_empty() {
                return Err(AppError::BadRequest("Debe especificar marca para el criterio POR_MARCA".into()));
            }
            sqlx::query_as("SELECT id, name, cost_price, price, min_margin FROM products WHERE brand = $1 FOR UPDATE")
                .bind(marca)
                .fetch_all(&mut *tx)
                .await?
        }
        "TODOS" => {
            sqlx::query_as("SELECT id, name, cost_price, price, min_margin FROM products FOR UPDATE")
                .fetch_all(&mut *tx)
                .await?
        }
        _ => {
            return Err(AppError::BadRequest(
                "Criterio inválido. Use POR_PROVEEDOR, POR_CATEGORIA, POR_MARCA o TODOS".into(),
            ));
        }
    };

    let count = rows.len();
    let factor = Decimal::ONE + (payload.porcentaje_aumento_costo / Decimal::from(100));
    let mut updated_items = Vec::new();

    for (product_id, name, old_cost, old_price, min_margin_opt) in rows {
        let new_cost = (old_cost * factor).round_dp(2);
        let min_margin = min_margin_opt.unwrap_or(Decimal::ZERO);

        let new_price = if payload.actualizar_precio_venta {
            let proposed_price = (old_price * factor).round_dp(2);
            if payload.respetar_margen_minimo && min_margin > Decimal::ZERO {
                let min_required_price = (new_cost * (Decimal::ONE + (min_margin / Decimal::from(100)))).round_dp(2);
                proposed_price.max(min_required_price)
            } else {
                proposed_price
            }
        } else {
            old_price
        };

        sqlx::query(
            r#"UPDATE products 
               SET cost_price = $1, price = $2, "updatedAt" = now() 
               WHERE id = $3"#
        )
        .bind(new_cost)
        .bind(new_price)
        .bind(product_id)
        .execute(&mut *tx)
        .await?;

        // Registrar en historico_precios_costo para trazabilidad completa
        sqlx::query(
            r#"INSERT INTO historico_precios_costo 
               (producto_id, precio_costo_anterior, precio_costo_nuevo, precio_venta_anterior, precio_venta_nuevo, motivo, user_id, "createdAt")
               VALUES ($1, $2, $3, $4, $5, $6, $7, now())"#
        )
        .bind(product_id)
        .bind(old_cost)
        .bind(new_cost)
        .bind(old_price)
        .bind(new_price)
        .bind(format!("Aumento masivo {}%: criterio={}", payload.porcentaje_aumento_costo, payload.criterio))
        .bind(claims.sub)
        .execute(&mut *tx)
        .await?;

        updated_items.push(json!({
            "id": product_id,
            "nombre": name,
            "costo_anterior": old_cost,
            "costo_nuevo": new_cost,
            "precio_anterior": old_price,
            "precio_nuevo": new_price
        }));
    }

    let payload_val = json!({
        "criterio": payload.criterio,
        "porcentaje": payload.porcentaje_aumento_costo,
        "actualizados": count
    });
    emit_event(&mut *tx, "PRODUCTOS", "MASIVO", "AUMENTO_PRECIOS_MASIVO", &payload_val).await?;

    tx.commit().await?;

    Ok(Json(json!({
        "message": format!("Aumento masivo aplicado con éxito a {} producto(s).", count),
        "productos_actualizados": count,
        "porcentaje_aplicado": payload.porcentaje_aumento_costo,
        "detalles": updated_items
    })))
}
