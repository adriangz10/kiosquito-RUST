use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct Product {
    pub id: i32,
    pub name: String,
    pub price: Decimal,
    pub stock: Option<i32>,
    pub category_id: i32,
    pub provider_id: Option<i32>,
    pub barcode: Option<String>,
    pub cost_price: Option<Decimal>,
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
    pub stock: Option<i32>,
    pub category_id: i32,
    pub provider_id: Option<i32>,
    pub barcode: Option<String>,
    pub cost_price: Option<Decimal>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProduct {
    pub name: Option<String>,
    pub price: Option<Decimal>,
    pub stock: Option<i32>,
    pub category_id: Option<i32>,
    pub provider_id: Option<i32>,
    pub barcode: Option<String>,
    pub cost_price: Option<Decimal>,
}
