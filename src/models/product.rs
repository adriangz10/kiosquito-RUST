use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct Product {
    pub id: i32,
    pub name: String,
    pub price: Decimal,
    pub stock: Option<Decimal>,
    pub category_id: i32,
    pub provider_id: Option<i32>,
    pub barcode: Option<String>,
    pub cost_price: Option<Decimal>,
    pub min_margin: Option<Decimal>,
    pub brand: Option<String>,
    #[serde(rename = "createdAt")]
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProduct {
    pub name: String,
    pub price: Decimal,
    pub stock: Option<Decimal>,
    pub category_id: i32,
    pub provider_id: Option<i32>,
    pub barcode: Option<String>,
    pub cost_price: Option<Decimal>,
    pub min_margin: Option<Decimal>,
    pub brand: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProduct {
    pub name: Option<String>,
    pub price: Option<Decimal>,
    pub stock: Option<Decimal>,
    pub category_id: Option<i32>,
    pub provider_id: Option<i32>,
    pub barcode: Option<String>,
    pub cost_price: Option<Decimal>,
    pub min_margin: Option<Decimal>,
    pub brand: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AumentoMasivoRequest {
    pub criterio: String, // "POR_PROVEEDOR", "POR_CATEGORIA", "TODOS", "POR_MARCA"
    pub proveedor_id: Option<i32>,
    pub categoria_id: Option<i32>,
    pub marca: Option<String>,
    pub porcentaje_aumento_costo: Decimal,
    pub actualizar_precio_venta: bool,
    pub respetar_margen_minimo: bool,
}
