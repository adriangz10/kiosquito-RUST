use crate::{
    errors::AppError,
    extractors::{AdminUser, AuthUser},
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Transaction};

#[derive(Debug, Deserialize)]
pub struct CustomerInput {
    pub name: Option<String>,
    #[serde(rename = "lastName")]
    pub last_name: Option<String>,
    pub phone: Option<String>,
    #[serde(rename = "DNI")]
    pub dni: Option<String>,
    #[serde(rename = "creditMax")]
    pub credit_max: Option<Decimal>,
    pub allow_credit: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub term: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PaymentInput {
    pub customer_id: i32,
    pub amount: Decimal,
    pub description: Option<String>,
    pub payment_method: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpenShiftInput {
    pub initial_cash: Option<Decimal>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CloseShiftInput {
    pub actual_cash: Decimal,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SaleInput {
    pub customer_id: i32,
    pub products: Vec<SaleItem>,
    pub payment_method: String,
    pub discount_amount: Option<Decimal>,
}

#[derive(Debug, Deserialize)]
pub struct SaleItem {
    pub product_id: i32,
    pub quantity: i32,
}

#[derive(Debug, Deserialize)]
pub struct PurchaseInput {
    pub provider_id: i32,
    pub invoice_number: String,
    pub invoice_type: Option<String>,
    pub issue_date: Option<NaiveDate>,
    pub due_date: Option<NaiveDate>,
    pub payment_condition: Option<String>,
    pub payment_method: Option<String>,
    pub products: Vec<PurchaseItem>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PurchaseItem {
    pub product_id: i32,
    pub quantity: i32,
    pub unit_cost: Decimal,
}

#[derive(Debug, Deserialize)]
pub struct ProviderPayment {
    pub provider_id: i32,
    pub amount: Decimal,
    pub description: Option<String>,
    pub payment_method: Option<String>,
    pub due_date: Option<NaiveDate>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct CustomerRow {
    id: i32,
    name: String,
    #[sqlx(rename = "lastName")]
    last_name: Option<String>,
    phone: Option<String>,
    #[sqlx(rename = "DNI")]
    dni: Option<String>,
    #[sqlx(rename = "creditMax")]
    credit_max: Option<Decimal>,
    allow_credit: bool,
    #[sqlx(rename = "createdAt")]
    created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    updated_at: DateTime<Utc>,
}

fn customer_json(row: CustomerRow, balance: Decimal) -> Value {
    json!({
        "id": row.id, "name": row.name, "lastName": row.last_name,
        "phone": row.phone, "DNI": row.dni, "creditMax": row.credit_max,
        "allow_credit": row.allow_credit, "saldo_actual": balance,
        "createdAt": row.created_at, "updatedAt": row.updated_at
    })
}

async fn customer_balance(pool: &PgPool, customer_id: i32) -> Result<Decimal, AppError> {
    Ok(sqlx::query_scalar::<_, Option<Decimal>>(
        "SELECT COALESCE(SUM(CASE WHEN type = 'debit' THEN amount ELSE -amount END), 0) FROM customer_accounts WHERE customer_id = $1",
    )
    .bind(customer_id)
    .fetch_one(pool)
    .await?
    .unwrap_or_default())
}

pub async fn customers(
    State(pool): State<PgPool>,
    AuthUser(_user): AuthUser,
) -> Result<Json<Vec<Value>>, AppError> {
    let rows = sqlx::query_as::<_, CustomerRow>("SELECT id, name, \"lastName\", phone, \"DNI\", \"creditMax\", allow_credit, \"createdAt\", \"updatedAt\" FROM customers ORDER BY name")
        .fetch_all(&pool).await?;
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let balance = customer_balance(&pool, row.id).await?;
        result.push(customer_json(row, balance));
    }
    Ok(Json(result))
}

pub async fn customer_search(
    State(pool): State<PgPool>,
    AuthUser(_user): AuthUser,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<Value>>, AppError> {
    let term = match query.term {
        Some(t) if !t.trim().is_empty() => t.trim().to_string(),
        _ => return Ok(Json(vec![])),
    };
    let pattern = format!("%{}%", term);
    let rows = sqlx::query_as::<_, (i32, String, Option<String>, Option<String>, bool)>(
        r#"
        SELECT id, name, "lastName", "DNI", allow_credit
        FROM customers
        WHERE name ILIKE $1 OR COALESCE("lastName", '') ILIKE $1 OR COALESCE("DNI", '') ILIKE $1
        ORDER BY name
        LIMIT 10
        "#,
    )
    .bind(pattern)
    .fetch_all(&pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, last_name, dni, allow_credit)| {
                json!({
                    "customer_id": id,
                    "nombre": format!("{}{}", name, last_name.map(|v| format!(" {}", v)).unwrap_or_default()),
                    "dni": dni.unwrap_or_else(|| "Sin especificar".to_string()),
                    "permite_credito": allow_credit
                })
            })
            .collect(),
    ))
}

pub async fn customer_create(
    State(pool): State<PgPool>,
    AuthUser(_user): AuthUser,
    Json(payload): Json<CustomerInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let name = payload
        .name
        .ok_or_else(|| AppError::BadRequest("Nombre requerido".into()))?;
    if name.trim().is_empty() || payload.credit_max.is_some_and(|v| v.is_sign_negative()) {
        return Err(AppError::BadRequest(
            "Nombre requerido y creditMax no puede ser negativo".into(),
        ));
    }
    let row = sqlx::query_as::<_, CustomerRow>("INSERT INTO customers (name, \"lastName\", phone, \"DNI\", \"creditMax\", allow_credit, \"createdAt\", \"updatedAt\") VALUES ($1,$2,$3,$4,COALESCE($5,0),COALESCE($6,false),now(),now()) RETURNING id, name, \"lastName\", phone, \"DNI\", \"creditMax\", allow_credit, \"createdAt\", \"updatedAt\"")
        .bind(name).bind(payload.last_name).bind(payload.phone).bind(payload.dni).bind(payload.credit_max).bind(payload.allow_credit).fetch_one(&pool).await?;
    Ok((StatusCode::CREATED, Json(customer_json(row, Decimal::ZERO))))
}

pub async fn customer_get(
    State(pool): State<PgPool>,
    AuthUser(_user): AuthUser,
    Path(id): Path<i32>,
) -> Result<Json<Value>, AppError> {
    let row = sqlx::query_as::<_, CustomerRow>("SELECT id, name, \"lastName\", phone, \"DNI\", \"creditMax\", allow_credit, \"createdAt\", \"updatedAt\" FROM customers WHERE id=$1").bind(id).fetch_one(&pool).await?;
    Ok(Json(customer_json(row, customer_balance(&pool, id).await?)))
}

pub async fn customer_update(
    State(pool): State<PgPool>,
    AuthUser(_user): AuthUser,
    Path(id): Path<i32>,
    Json(payload): Json<CustomerInput>,
) -> Result<Json<Value>, AppError> {
    if payload.credit_max.is_some_and(|v| v.is_sign_negative()) {
        return Err(AppError::BadRequest(
            "creditMax no puede ser negativo".into(),
        ));
    }
    let row = sqlx::query_as::<_, CustomerRow>("UPDATE customers SET name=COALESCE($1,name), \"lastName\"=COALESCE($2,\"lastName\"), phone=COALESCE($3,phone), \"DNI\"=COALESCE($4,\"DNI\"), \"creditMax\"=COALESCE($5,\"creditMax\"), allow_credit=COALESCE($6,allow_credit), \"updatedAt\"=now() WHERE id=$7 RETURNING id, name, \"lastName\", phone, \"DNI\", \"creditMax\", allow_credit, \"createdAt\", \"updatedAt\"").bind(payload.name).bind(payload.last_name).bind(payload.phone).bind(payload.dni).bind(payload.credit_max).bind(payload.allow_credit).bind(id).fetch_one(&pool).await?;
    Ok(Json(customer_json(row, customer_balance(&pool, id).await?)))
}

pub async fn customer_payment(
    State(pool): State<PgPool>,
    AuthUser(claims): AuthUser,
    Json(payload): Json<PaymentInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    validate_positive(payload.amount)?;
    let method = payload
        .payment_method
        .unwrap_or_else(|| "Efectivo".to_string());
    if !["Efectivo", "Transferencia", "Débito", "Tarjeta de crédito"].contains(&method.as_str()) {
        return Err(AppError::BadRequest("Método de pago inválido".into()));
    }
    let mut tx = pool.begin().await?;
    let allow: bool = sqlx::query_scalar("SELECT allow_credit FROM customers WHERE id=$1")
        .bind(payload.customer_id)
        .fetch_one(&mut *tx)
        .await?;
    if !allow {
        return Err(AppError::BadRequest("El cliente no permite crédito".into()));
    }
    let balance: Decimal = sqlx::query_scalar("SELECT COALESCE(SUM(CASE WHEN type='debit' THEN amount ELSE -amount END),0) FROM customer_accounts WHERE customer_id=$1").bind(payload.customer_id).fetch_one(&mut *tx).await?;
    if payload.amount > balance {
        return Err(AppError::BadRequest(
            "El cobro supera el saldo del cliente".into(),
        ));
    }
    let shift: Option<i32> = if method == "Efectivo" {
        sqlx::query_scalar("SELECT id FROM cash_shifts WHERE user_id=$1 AND status='abierta' ORDER BY opened_at DESC LIMIT 1")
            .bind(claims.sub).fetch_optional(&mut *tx).await?
    } else {
        None
    };
    if method == "Efectivo" && shift.is_none() {
        return Err(AppError::BadRequest(
            "Se requiere un turno abierto para cobrar en efectivo".into(),
        ));
    }
    let movement = sqlx::query_as::<_, (i32, i32, Option<i32>, String, Decimal, Option<String>, Option<String>, Option<i32>, Option<i32>, DateTime<Utc>)>(
        r#"INSERT INTO customer_accounts (customer_id,type,amount,description,"createdAt","updatedAt",payment_method,user_id,cash_shift_id)
           VALUES ($1,'credit',$2,COALESCE($3,'Cobro de cuenta corriente'),now(),now(),$4::enum_customer_accounts_payment_method,$5,$6)
           RETURNING id, customer_id, sale_id, type::text, amount, description, payment_method::text, user_id, cash_shift_id, "createdAt""#
    )
        .bind(payload.customer_id).bind(payload.amount).bind(payload.description).bind(&method).bind(claims.sub).bind(shift).fetch_one(&mut *tx).await?;
    if let Some(shift_id) = shift {
        sqlx::query(
            "UPDATE cash_shifts SET expected_cash=expected_cash+$1,\"updatedAt\"=now() WHERE id=$2",
        )
        .bind(payload.amount)
        .bind(shift_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "message": "Pago registrado",
            "movement": {
                "id": movement.0,
                "customer_id": movement.1,
                "sale_id": movement.2,
                "type": movement.3,
                "amount": movement.4,
                "description": movement.5,
                "payment_method": movement.6,
                "user_id": movement.7,
                "cash_shift_id": movement.8,
                "createdAt": movement.9,
            }
        })),
    ))
}

pub async fn customer_balance_endpoint(
    State(pool): State<PgPool>,
    AuthUser(_user): AuthUser,
    Path(id): Path<i32>,
) -> Result<Json<Value>, AppError> {
    let (name, total_bought, total_paid): (String, Decimal, Decimal) = sqlx::query_as(
        r#"SELECT CONCAT(c.name, COALESCE(' ' || c."lastName",'')),
                  COALESCE((SELECT SUM(amount) FROM customer_accounts WHERE customer_id=c.id AND type='debit'),0),
                  COALESCE((SELECT SUM(amount) FROM customer_accounts WHERE customer_id=c.id AND type='credit'),0)
           FROM customers c WHERE c.id=$1"#
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(
        json!({"customer_id":id,"name":name,"total_comprado":total_bought,"total_pagado":total_paid,"saldo_actual":total_bought-total_paid}),
    ))
}

pub async fn customer_history(
    State(pool): State<PgPool>,
    AuthUser(_user): AuthUser,
    Path(id): Path<i32>,
) -> Result<Json<Vec<Value>>, AppError> {
    let rows = sqlx::query_as::<_, (i32, i32, Option<i32>, String, Decimal, Option<String>, Option<String>, DateTime<Utc>)>(
        r#"SELECT id, customer_id, sale_id, type::text, amount, description, payment_method::text, "createdAt"
           FROM customer_accounts WHERE customer_id=$1 ORDER BY "createdAt" DESC"#
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|r| {
                json!({
                    "id": r.0,
                    "customer_id": r.1,
                    "sale_id": r.2,
                    "type": r.3,
                    "amount": r.4,
                    "description": r.5,
                    "payment_method": r.6,
                    "createdAt": r.7
                })
            })
            .collect(),
    ))
}

fn validate_positive(value: Decimal) -> Result<(), AppError> {
    if value <= Decimal::ZERO {
        Err(AppError::BadRequest("El monto debe ser positivo".into()))
    } else {
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct DateRange {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
}

#[derive(Debug, Deserialize)]
pub struct ExpenseInput {
    pub category: String,
    pub description: String,
    pub amount: Decimal,
    pub date: Option<NaiveDate>,
    #[serde(alias = "payment_method", alias = "paymentMethod")]
    pub payment_method: Option<String>,
    pub receipt_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExpenseQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub category: Option<String>,
}

pub async fn expenses(
    State(pool): State<PgPool>,
    AuthUser(_user): AuthUser,
    Query(query): Query<ExpenseQuery>,
) -> Result<Json<Vec<Value>>, AppError> {
    if query.from.zip(query.to).is_some_and(|(from, to)| from > to) {
        return Err(AppError::BadRequest(
            "El rango de fechas es inválido".into(),
        ));
    }
    let rows = sqlx::query_as::<
        _,
        (
            i32,
            String,
            String,
            Decimal,
            NaiveDate,
            String,
            Option<String>,
        ),
    >(
        r#"
        SELECT id, category::text, description, amount, date, "paymentMethod"::text, receipt_url
        FROM expenses
        WHERE ($1::date IS NULL OR date >= $1)
          AND ($2::date IS NULL OR date <= $2)
          AND ($3::text IS NULL OR category::text = $3)
        ORDER BY date DESC, id DESC
        "#,
    )
    .bind(query.from)
    .bind(query.to)
    .bind(query.category)
    .fetch_all(&pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, category, description, amount, date, payment_method, receipt_url)| {
                    json!({
                        "id": id,
                        "category": category,
                        "description": description,
                        "amount": amount,
                        "date": date,
                        "paymentMethod": payment_method,
                        "receipt_url": receipt_url
                    })
                },
            )
            .collect(),
    ))
}

pub async fn expense_create(
    State(pool): State<PgPool>,
    AuthUser(claims): AuthUser,
    Json(payload): Json<ExpenseInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    validate_positive(payload.amount)?;
    if !["Estructura", "Herramientas", "Personal", "Marketing"].contains(&payload.category.as_str())
    {
        return Err(AppError::BadRequest("Categoría de gasto inválida".into()));
    }
    let payment = payload.payment_method.unwrap_or_else(|| "Efectivo".into());
    if ![
        "Efectivo",
        "Transferencia",
        "Tarjeta de débito",
        "Tarjeta de crédito",
    ]
    .contains(&payment.as_str())
    {
        return Err(AppError::BadRequest("Método de pago inválido".into()));
    }
    let shift: Option<(i32, Decimal)> = if payment == "Efectivo" {
        sqlx::query_as(
            "SELECT id, expected_cash FROM cash_shifts WHERE user_id=$1 AND status='abierta' ORDER BY opened_at DESC LIMIT 1",
        )
        .bind(claims.sub)
        .fetch_optional(&pool)
        .await?
    } else {
        None
    };
    let shift_id: Option<i32> = if payment == "Efectivo" {
        let (s_id, expected_cash) = shift.ok_or_else(|| {
            AppError::BadRequest(
                "Se requiere un turno abierto para registrar un gasto en efectivo".into(),
            )
        })?;
        if expected_cash < payload.amount {
            return Err(AppError::BadRequest(
                "El efectivo esperado en caja no alcanza para registrar este gasto. Registra un retiro o usa otro medio de pago.".into(),
            ));
        }
        Some(s_id)
    } else {
        None
    };
    let row = sqlx::query_as::<_, (i32, String, String, Decimal, NaiveDate, String)>(
        r#"
        INSERT INTO expenses (category, description, amount, date, "paymentMethod", receipt_url, cash_shift_id, user_id, "createdAt", "updatedAt")
        VALUES ($1::enum_expenses_category, $2, $3, COALESCE($4, CURRENT_DATE), $5::"enum_expenses_paymentMethod", $6, $7, $8, now(), now())
        RETURNING id, category::text, description, amount, date, "paymentMethod"::text
        "#,
    )
    .bind(payload.category)
    .bind(payload.description)
    .bind(payload.amount)
    .bind(payload.date)
    .bind(&payment)
    .bind(payload.receipt_url)
    .bind(shift_id)
    .bind(claims.sub)
    .fetch_one(&pool)
    .await?;

    if let Some(s_id) = shift_id {
        sqlx::query(
            "UPDATE cash_shifts SET expected_cash=expected_cash-$1, \"updatedAt\"=now() WHERE id=$2",
        )
        .bind(payload.amount)
        .bind(s_id)
        .execute(&pool)
        .await?;
    }
    Ok((
        StatusCode::CREATED,
        Json(
            json!({"message": "Gasto operativo registrado exitosamente.", "expense": {"id": row.0, "category": row.1, "description": row.2, "amount": row.3, "date": row.4, "paymentMethod": row.5}}),
        ),
    ))
}

pub async fn expense_delete(
    State(pool): State<PgPool>,
    AdminUser(_user): AdminUser,
    Path(id): Path<i32>,
) -> Result<Json<Value>, AppError> {
    let expense = sqlx::query_as::<_, (Decimal, Option<i32>)>(
        "SELECT amount, cash_shift_id FROM expenses WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?
    .ok_or(AppError::NotFound)?;

    if let Some(shift_id) = expense.1 {
        sqlx::query(
            "UPDATE cash_shifts SET expected_cash=expected_cash+$1, \"updatedAt\"=now() WHERE id=$2 AND status='abierta'",
        )
        .bind(expense.0)
        .bind(shift_id)
        .execute(&pool)
        .await?;
    }

    sqlx::query("DELETE FROM expenses WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await?;

    Ok(Json(json!({"message": "Gasto eliminado exitosamente."})))
}

pub async fn expense_summary(
    State(pool): State<PgPool>,
    AuthUser(_user): AuthUser,
    Query(range): Query<DateRange>,
) -> Result<Json<Value>, AppError> {
    validate_range(&range)?;
    let total: Decimal = sqlx::query_scalar("SELECT COALESCE(SUM(amount),0) FROM expenses WHERE ($1::date IS NULL OR date >= $1) AND ($2::date IS NULL OR date <= $2)")
        .bind(range.from).bind(range.to).fetch_one(&pool).await?;
    let rows = sqlx::query_as::<_, (String, i64, Decimal)>("SELECT category::text,COUNT(*),COALESCE(SUM(amount),0) FROM expenses WHERE ($1::date IS NULL OR date >= $1) AND ($2::date IS NULL OR date <= $2) GROUP BY category ORDER BY category")
        .bind(range.from).bind(range.to).fetch_all(&pool).await?;
    Ok(Json(
        json!({"totalGeneral":total,"porCategoria":rows.into_iter().map(|(category,count,sum)| json!({"category":category,"cantidad":count,"total":sum})).collect::<Vec<_>>()}),
    ))
}

pub async fn cash_open(
    State(pool): State<PgPool>,
    AuthUser(claims): AuthUser,
    Json(payload): Json<OpenShiftInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let initial = payload.initial_cash.unwrap_or_default();
    if initial.is_sign_negative() {
        return Err(AppError::BadRequest(
            "El fondo inicial no puede ser negativo".into(),
        ));
    }
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM cash_shifts WHERE user_id=$1 AND status='abierta')",
    )
    .bind(claims.sub)
    .fetch_one(&pool)
    .await?;
    if exists {
        return Err(AppError::BadRequest("Ya tienes un turno abierto".into()));
    }
    let shift = sqlx::query_as::<_, (i32, Decimal, Decimal, String, DateTime<Utc>)>("INSERT INTO cash_shifts (user_id,opened_at,initial_cash,expected_cash,status,notes,\"createdAt\",\"updatedAt\") VALUES ($1,now(),$2,$2,'abierta',$3,now(),now()) RETURNING id,initial_cash,expected_cash,status::text,opened_at")
        .bind(claims.sub).bind(initial).bind(payload.notes).fetch_one(&pool).await?;
    Ok((
        StatusCode::CREATED,
        Json(
            json!({"message":"Turno abierto","shift":{"id":shift.0,"initial_cash":shift.1,"expected_cash":shift.2,"status":shift.3,"opened_at":shift.4}}),
        ),
    ))
}

pub async fn cash_current(
    State(pool): State<PgPool>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Value>, AppError> {
    let shift = sqlx::query_as::<_, (i32, Decimal, Decimal, Option<Decimal>, String, DateTime<Utc>, Option<String>)>("SELECT id,initial_cash,expected_cash,actual_cash,status::text,opened_at,notes FROM cash_shifts WHERE user_id=$1 AND status='abierta' ORDER BY opened_at DESC LIMIT 1").bind(claims.sub).fetch_optional(&pool).await?;
    let Some(shift) = shift else {
        return Ok(Json(json!({"open":false,"shift":null})));
    };
    let sales: Value = sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(s) ORDER BY s.\"createdAt\" DESC),'[]'::jsonb) FROM sales s WHERE s.cash_shift_id=$1").bind(shift.0).fetch_one(&pool).await?;
    let expenses: Value = sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(e) ORDER BY e.\"createdAt\" DESC),'[]'::jsonb) FROM expenses e WHERE e.cash_shift_id=$1").bind(shift.0).fetch_one(&pool).await?;
    Ok(Json(
        json!({"open":true,"shift":{"id":shift.0,"initial_cash":shift.1,"expected_cash":shift.2,"actual_cash":shift.3,"status":shift.4,"opened_at":shift.5,"notes":shift.6,"sales":sales,"expenses":expenses}}),
    ))
}

pub async fn cash_close(
    State(pool): State<PgPool>,
    AuthUser(claims): AuthUser,
    Json(payload): Json<CloseShiftInput>,
) -> Result<Json<Value>, AppError> {
    if payload.actual_cash.is_sign_negative() {
        return Err(AppError::BadRequest(
            "El efectivo contado no puede ser negativo".into(),
        ));
    }
    let row = sqlx::query_as::<_, (i32, Decimal, Decimal, DateTime<Utc>)>(
        "SELECT id, initial_cash, expected_cash, opened_at FROM cash_shifts WHERE user_id=$1 AND status='abierta' ORDER BY opened_at DESC LIMIT 1",
    )
    .bind(claims.sub)
    .fetch_optional(&pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let difference = payload.actual_cash - row.2;
    let estado_arqueo = if difference == Decimal::ZERO {
        "CUADRADO_EXACTO".to_string()
    } else if difference > Decimal::ZERO {
        format!("SOBRANTE_+${:.2}", difference)
    } else {
        format!("FALTANTE_-${:.2}", difference.abs())
    };

    sqlx::query(
        "UPDATE cash_shifts SET actual_cash=$1, difference=$2, status='cerrada', closed_at=now(), notes=COALESCE($3, notes), \"updatedAt\"=now() WHERE id=$4",
    )
    .bind(payload.actual_cash)
    .bind(difference)
    .bind(payload.notes)
    .bind(row.0)
    .execute(&pool)
    .await?;

    Ok(Json(json!({
        "message": "Arqueo de caja y cierre de turno completado.",
        "resultado": {
            "id": row.0,
            "fondo_inicial": row.1,
            "saldo_teorico_esperado": row.2,
            "efectivo_fisico_contado": payload.actual_cash,
            "diferencia": difference,
            "estado_arqueo": estado_arqueo,
            "opened_at": row.3,
            "closed_at": Utc::now()
        }
    })))
}

pub async fn cash_history(
    State(pool): State<PgPool>,
    AdminUser(_user): AdminUser,
) -> Result<Json<Vec<Value>>, AppError> {
    let rows = sqlx::query_as::<
        _,
        (
            i32,
            i32,
            Decimal,
            Decimal,
            Option<Decimal>,
            Option<Decimal>,
            String,
            DateTime<Utc>,
            Option<DateTime<Utc>>,
            Option<String>,
            Value,
        ),
    >(
        r#"
        SELECT
            cs.id,
            cs.user_id,
            cs.initial_cash,
            cs.expected_cash,
            cs.actual_cash,
            cs.difference,
            cs.status::text,
            cs.opened_at,
            cs.closed_at,
            cs.notes,
            jsonb_build_object('id', u.id, 'username', u.username) AS "user"
        FROM cash_shifts cs
        LEFT JOIN users u ON u.id = cs.user_id
        ORDER BY cs.opened_at DESC
        LIMIT 50
        "#,
    )
    .fetch_all(&pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(
                    id,
                    user_id,
                    initial,
                    expected,
                    actual,
                    difference,
                    status,
                    opened_at,
                    closed_at,
                    notes,
                    user,
                )| {
                    json!({
                        "id": id,
                        "user_id": user_id,
                        "initial_cash": initial,
                        "expected_cash": expected,
                        "actual_cash": actual,
                        "difference": difference,
                        "status": status,
                        "opened_at": opened_at,
                        "closed_at": closed_at,
                        "notes": notes,
                        "user": user
                    })
                },
            )
            .collect(),
    ))
}

pub async fn sale_create(
    State(pool): State<PgPool>,
    AuthUser(claims): AuthUser,
    Json(payload): Json<SaleInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    if payload.products.is_empty()
        || ![
            "Efectivo",
            "Débito",
            "Cuenta corriente",
            "Transferencia",
            "Tarjeta de crédito",
        ]
        .contains(&payload.payment_method.as_str())
    {
        return Err(AppError::BadRequest("Venta inválida".into()));
    }
    let discount = payload.discount_amount.unwrap_or_default();
    if discount.is_sign_negative() {
        return Err(AppError::BadRequest(
            "El descuento no puede ser negativo".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    let mut subtotal = Decimal::ZERO;
    let mut details = Vec::new();
    for item in &payload.products {
        if item.quantity <= 0 {
            return Err(AppError::BadRequest("La cantidad debe ser positiva".into()));
        }
        let product = sqlx::query_as::<_, (String, Decimal, Option<i32>, Decimal)>(
            "SELECT name,price,stock,cost_price FROM products WHERE id=$1 FOR UPDATE",
        )
        .bind(item.product_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;
        if product.2.unwrap_or_default() < item.quantity {
            return Err(AppError::BadRequest(format!(
                "Stock insuficiente para {}",
                product.0
            )));
        }
        subtotal += product.1 * Decimal::from(item.quantity);
        details.push((item.product_id, item.quantity, product.1, product.3));
    }
    if discount > subtotal {
        return Err(AppError::BadRequest(
            "El descuento supera el subtotal".into(),
        ));
    }
    let total = subtotal - discount;
    if payload.payment_method == "Cuenta corriente" {
        let (allow, limit): (bool, Decimal) = sqlx::query_as(
            "SELECT allow_credit,COALESCE(\"creditMax\",0) FROM customers WHERE id=$1",
        )
        .bind(payload.customer_id)
        .fetch_one(&mut *tx)
        .await?;
        let balance: Decimal = sqlx::query_scalar("SELECT COALESCE(SUM(CASE WHEN type='debit' THEN amount ELSE -amount END),0) FROM customer_accounts WHERE customer_id=$1").bind(payload.customer_id).fetch_one(&mut *tx).await?;
        if !allow || balance + total > limit {
            return Err(AppError::BadRequest(
                "El cliente no tiene crédito disponible".into(),
            ));
        }
    }
    let shift: Option<i32> = if payload.payment_method == "Efectivo" {
        sqlx::query_scalar("SELECT id FROM cash_shifts WHERE user_id=$1 AND status='abierta' ORDER BY opened_at DESC LIMIT 1").bind(claims.sub).fetch_optional(&mut *tx).await?
    } else {
        None
    };
    let sale_id: i32 = sqlx::query_scalar("INSERT INTO sales (total,customer_id,user_id,\"paymentMethod\",status,subtotal,discount_amount,cash_shift_id,\"createdAt\",\"updatedAt\") VALUES ($1,$2,$3,$4::\"enum_sales_paymentMethod\",'completada',$5,$6,$7,now(),now()) RETURNING id").bind(total).bind(payload.customer_id).bind(claims.sub).bind(&payload.payment_method).bind(subtotal).bind(discount).bind(shift).fetch_one(&mut *tx).await?;
    for (product_id, quantity, unit_price, unit_cost) in details {
        sqlx::query("INSERT INTO sale_details (sale_id,product_id,quantity,unit_price,unit_cost,\"createdAt\",\"updatedAt\") VALUES ($1,$2,$3,$4,$5,now(),now())").bind(sale_id).bind(product_id).bind(quantity).bind(unit_price).bind(unit_cost).execute(&mut *tx).await?;
        sqlx::query("UPDATE products SET stock=stock-$1,\"updatedAt\"=now() WHERE id=$2")
            .bind(quantity)
            .bind(product_id)
            .execute(&mut *tx)
            .await?;
    }
    if payload.payment_method == "Cuenta corriente" {
        sqlx::query("INSERT INTO customer_accounts (customer_id,sale_id,type,amount,description,\"createdAt\",\"updatedAt\") VALUES ($1,$2,'debit',$3,$4,now(),now())")
            .bind(payload.customer_id)
            .bind(sale_id)
            .bind(total)
            .bind(format!("Compra fiada - Venta #{}", sale_id))
            .execute(&mut *tx)
            .await?;
    }
    if let Some(shift_id) = shift {
        sqlx::query(
            "UPDATE cash_shifts SET expected_cash=expected_cash+$1,\"updatedAt\"=now() WHERE id=$2",
        )
        .bind(total)
        .bind(shift_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "message": "Venta realizada con éxito 🎉",
            "sale_id": sale_id,
            "total": total,
            "sale": {
                "id": sale_id,
                "subtotal": subtotal,
                "discount_amount": discount,
                "total": total
            }
        })),
    ))
}

pub async fn sales_history(
    State(pool): State<PgPool>,
    AuthUser(_user): AuthUser,
    Query(range): Query<DateRange>,
) -> Result<Json<Vec<Value>>, AppError> {
    validate_range(&range)?;
    let rows = sqlx::query_as::<
        _,
        (
            i32,
            DateTime<Utc>,
            Decimal,
            String,
            String,
            String,
            String,
            Value,
        ),
    >(
        r#"
        SELECT
            s.id,
            s."createdAt",
            s.total,
            s."paymentMethod"::text,
            s.status::text,
            COALESCE(u.username, 'Desconocido') AS vendedor,
            COALESCE(c.name, 'Consumidor Final') AS cliente,
            COALESCE(
                (
                    SELECT jsonb_agg(
                        jsonb_build_object(
                            'producto_id', d.product_id,
                            'nombre_producto', COALESCE(p.name, 'Producto Eliminado'),
                            'cantidad', d.quantity,
                            'precio_unitario_historico', d.unit_price,
                            'subtotal', d.quantity * d.unit_price
                        )
                    )
                    FROM sale_details d
                    LEFT JOIN products p ON p.id = d.product_id
                    WHERE d.sale_id = s.id
                ),
                '[]'::jsonb
            ) AS productos_vendidos
        FROM sales s
        LEFT JOIN users u ON u.id = s.user_id
        LEFT JOIN customers c ON c.id = s.customer_id
        WHERE ($1::date IS NULL OR s."createdAt"::date >= $1)
          AND ($2::date IS NULL OR s."createdAt"::date <= $2)
        ORDER BY s."createdAt" DESC
        "#,
    )
    .bind(range.from)
    .bind(range.to)
    .fetch_all(&pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, fecha, total, metodo, status, vendedor, cliente, productos)| {
                    json!({
                        "venta_id": id,
                        "fecha": fecha,
                        "total": total,
                        "metodo_pago": metodo,
                        "status": status,
                        "vendedor": vendedor,
                        "cliente": cliente,
                        "productos_vendidos": productos
                    })
                },
            )
            .collect(),
    ))
}

pub async fn sale_get(
    State(pool): State<PgPool>,
    AuthUser(_user): AuthUser,
    Path(id): Path<i32>,
) -> Result<Json<Value>, AppError> {
    let row = sqlx::query_as::<
        _,
        (
            i32,
            DateTime<Utc>,
            Decimal,
            String,
            String,
            String,
            String,
            Value,
        ),
    >(
        r#"
        SELECT
            s.id,
            s."createdAt",
            s.total,
            s."paymentMethod"::text,
            s.status::text,
            COALESCE(u.username, 'Desconocido') AS vendedor,
            COALESCE(c.name, 'Consumidor Final') AS cliente,
            COALESCE(
                (
                    SELECT jsonb_agg(
                        jsonb_build_object(
                            'producto_id', d.product_id,
                            'nombre_producto', COALESCE(p.name, 'Producto Eliminado'),
                            'cantidad', d.quantity,
                            'precio_unitario_historico', d.unit_price,
                            'subtotal', d.quantity * d.unit_price
                        )
                    )
                    FROM sale_details d
                    LEFT JOIN products p ON p.id = d.product_id
                    WHERE d.sale_id = s.id
                ),
                '[]'::jsonb
            ) AS productos_vendidos
        FROM sales s
        LEFT JOIN users u ON u.id = s.user_id
        LEFT JOIN customers c ON c.id = s.customer_id
        WHERE s.id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(json!({
        "venta_id": row.0,
        "fecha": row.1,
        "total": row.2,
        "metodo_pago": row.3,
        "status": row.4,
        "vendedor": row.5,
        "cliente": row.6,
        "productos_vendidos": row.7
    })))
}

pub async fn sale_cancel(
    State(pool): State<PgPool>,
    AdminUser(_user): AdminUser,
    Path(id): Path<i32>,
) -> Result<Json<Value>, AppError> {
    let mut tx = pool.begin().await?;
    let sale = sqlx::query_as::<_, (Decimal,i32,Option<i32>,String)>("SELECT total,customer_id,cash_shift_id,\"paymentMethod\"::text FROM sales WHERE id=$1 AND status='completada' FOR UPDATE").fetch_optional(&mut *tx).await?.ok_or(AppError::NotFound)?;
    let details = sqlx::query_as::<_, (i32, i32)>(
        "SELECT product_id,quantity FROM sale_details WHERE sale_id=$1",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    for (product_id, quantity) in details {
        sqlx::query("UPDATE products SET stock=stock+$1,\"updatedAt\"=now() WHERE id=$2")
            .bind(quantity)
            .bind(product_id)
            .execute(&mut *tx)
            .await?;
    }
    if sale.3 == "Cuenta corriente" {
        sqlx::query("INSERT INTO customer_accounts (customer_id,sale_id,type,amount,description,\"createdAt\",\"updatedAt\") VALUES ($1,$2,'credit',$3,$4,now(),now())")
            .bind(sale.1)
            .bind(id)
            .bind(sale.0)
            .bind(format!("Anulación de venta #{}", id))
            .execute(&mut *tx)
            .await?;
    }
    if let Some(shift) = sale.2 {
        if sale.3 == "Efectivo" {
            sqlx::query("UPDATE cash_shifts SET expected_cash=GREATEST(0, expected_cash-$1),\"updatedAt\"=now() WHERE id=$2 AND status='abierta'")
                .bind(sale.0)
                .bind(shift)
                .execute(&mut *tx)
                .await?;
        }
    }
    sqlx::query("UPDATE sales SET status='anulada',\"updatedAt\"=now() WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"message": format!("Venta #{} anulada con éxito.", id)}),
    ))
}

#[derive(Debug, Deserialize)]
pub struct PurchaseQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub provider_id: Option<i32>,
}

pub async fn purchases(
    State(pool): State<PgPool>,
    AuthUser(_user): AuthUser,
    Query(query): Query<PurchaseQuery>,
) -> Result<Json<Vec<Value>>, AppError> {
    if query.from.zip(query.to).is_some_and(|(from, to)| from > to) {
        return Err(AppError::BadRequest(
            "El rango de fechas es inválido".into(),
        ));
    }
    let rows = sqlx::query_as::<
        _,
        (
            i32,
            i32,
            i32,
            Option<i32>,
            String,
            String,
            NaiveDate,
            Option<NaiveDate>,
            String,
            String,
            Decimal,
            String,
            Option<String>,
            Value,
            Value,
            Value,
        ),
    >(
        r#"
        SELECT
            p.id,
            p.provider_id,
            p.user_id,
            p.cash_shift_id,
            p.invoice_number,
            p.invoice_type::text,
            p.issue_date,
            p.due_date,
            p.payment_condition::text,
            p.payment_method::text,
            p.total_amount,
            p.status::text,
            p.notes,
            jsonb_build_object('id', pr.id, 'name', pr.name, 'phone', pr.phone) AS provider,
            jsonb_build_object('id', u.id, 'username', u.username) AS "user",
            COALESCE(
                (
                    SELECT jsonb_agg(
                        jsonb_build_object(
                            'id', d.id,
                            'purchase_id', d.purchase_id,
                            'product_id', d.product_id,
                            'quantity', d.quantity,
                            'unit_cost', d.unit_cost,
                            'subtotal', d.subtotal,
                            'product', jsonb_build_object(
                                'id', prod.id,
                                'name', prod.name,
                                'price', prod.price,
                                'cost_price', prod.cost_price
                            )
                        )
                        ORDER BY d.id ASC
                    )
                    FROM purchase_details d
                    LEFT JOIN products prod ON prod.id = d.product_id
                    WHERE d.purchase_id = p.id
                ),
                '[]'::jsonb
            ) AS details
        FROM purchases p
        LEFT JOIN providers pr ON pr.id = p.provider_id
        LEFT JOIN users u ON u.id = p.user_id
        WHERE ($1::date IS NULL OR p.issue_date >= $1)
          AND ($2::date IS NULL OR p.issue_date <= $2)
          AND ($3::int IS NULL OR p.provider_id = $3)
        ORDER BY p.issue_date DESC, p.id DESC
        "#,
    )
    .bind(query.from)
    .bind(query.to)
    .bind(query.provider_id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(
                    id,
                    provider_id,
                    user_id,
                    cash_shift_id,
                    invoice_number,
                    invoice_type,
                    issue_date,
                    due_date,
                    payment_condition,
                    payment_method,
                    total_amount,
                    status,
                    notes,
                    provider,
                    user,
                    details,
                )| {
                    json!({
                        "id": id,
                        "provider_id": provider_id,
                        "user_id": user_id,
                        "cash_shift_id": cash_shift_id,
                        "invoice_number": invoice_number,
                        "invoice_type": invoice_type,
                        "issue_date": issue_date,
                        "due_date": due_date,
                        "payment_condition": payment_condition,
                        "payment_method": payment_method,
                        "total_amount": total_amount,
                        "status": status,
                        "notes": notes,
                        "provider": provider,
                        "user": user,
                        "details": details
                    })
                },
            )
            .collect(),
    ))
}

pub async fn purchase_create(
    State(pool): State<PgPool>,
    AdminUser(claims): AdminUser,
    Json(payload): Json<PurchaseInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    if payload.products.is_empty() || payload.invoice_number.trim().is_empty() {
        return Err(AppError::BadRequest(
            "La compra requiere factura y productos".into(),
        ));
    }
    for item in &payload.products {
        if item.quantity <= 0 || item.unit_cost.is_sign_negative() {
            return Err(AppError::BadRequest("Cantidad y costo inválidos".into()));
        }
    }
    let condition = payload
        .payment_condition
        .unwrap_or_else(|| "Contado".into());
    let method = payload.payment_method.unwrap_or_else(|| "Efectivo".into());
    if !["Contado", "Cuenta corriente"].contains(&condition.as_str())
        || !["Efectivo", "Transferencia", "Tarjeta"].contains(&method.as_str())
    {
        return Err(AppError::BadRequest(
            "Condición o método de pago inválido".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    let total: Decimal = payload
        .products
        .iter()
        .map(|item| item.unit_cost * Decimal::from(item.quantity))
        .sum();

    let shift: Option<i32> = if condition == "Contado" && method == "Efectivo" {
        let active: Option<(i32, Decimal)> = sqlx::query_as(
            "SELECT id, expected_cash FROM cash_shifts WHERE user_id=$1 AND status='abierta' ORDER BY opened_at DESC LIMIT 1 FOR UPDATE",
        )
        .bind(claims.sub)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some((shift_id, expected_cash)) = active {
            if expected_cash < total {
                return Err(AppError::BadRequest(
                    "El efectivo esperado en caja no alcanza para esta compra.".into(),
                ));
            }
            Some(shift_id)
        } else {
            None
        }
    } else {
        None
    };

    let purchase_id: i32 = sqlx::query_scalar("INSERT INTO purchases (provider_id,user_id,cash_shift_id,invoice_number,invoice_type,issue_date,due_date,payment_condition,payment_method,total_amount,status,notes,\"createdAt\",\"updatedAt\") VALUES ($1,$2,$3,$4,COALESCE($5,'Remito'::enum_purchases_invoice_type),COALESCE($6,CURRENT_DATE),$7,$8::enum_purchases_payment_condition,$9::enum_purchases_payment_method,$10,'recibida',$11,now(),now()) RETURNING id").bind(payload.provider_id).bind(claims.sub).bind(shift).bind(payload.invoice_number).bind(payload.invoice_type).bind(payload.issue_date).bind(payload.due_date).bind(&condition).bind(&method).bind(total).bind(payload.notes).fetch_one(&mut *tx).await?;
    for item in payload.products {
        let (old_stock, old_cost): (i32, Decimal) = sqlx::query_as(
            "SELECT COALESCE(stock,0),cost_price FROM products WHERE id=$1 FOR UPDATE",
        )
        .bind(item.product_id)
        .fetch_one(&mut *tx)
        .await?;
        let new_stock = old_stock + item.quantity;
        let average = ((Decimal::from(old_stock) * old_cost)
            + (Decimal::from(item.quantity) * item.unit_cost))
            / Decimal::from(new_stock);
        let subtotal = item.unit_cost * Decimal::from(item.quantity);
        sqlx::query("INSERT INTO purchase_details (purchase_id,product_id,quantity,unit_cost,subtotal,\"createdAt\",\"updatedAt\") VALUES ($1,$2,$3,$4,$5,now(),now())").bind(purchase_id).bind(item.product_id).bind(item.quantity).bind(item.unit_cost).bind(subtotal).execute(&mut *tx).await?;
        sqlx::query("UPDATE products SET stock=$1,cost_price=$2,\"updatedAt\"=now() WHERE id=$3")
            .bind(new_stock)
            .bind(average)
            .bind(item.product_id)
            .execute(&mut *tx)
            .await?;
    }
    if condition == "Cuenta corriente" {
        sqlx::query("INSERT INTO provider_accounts (provider_id,purchase_id,type,amount,description,due_date,\"createdAt\",\"updatedAt\") VALUES ($1,$2,'credit',$3,'Compra a cuenta',$4,now(),now())").bind(payload.provider_id).bind(purchase_id).bind(total).bind(payload.due_date).execute(&mut *tx).await?;
    }
    if let Some(shift_id) = shift {
        sqlx::query("UPDATE cash_shifts SET expected_cash=expected_cash-$1 WHERE id=$2")
            .bind(total)
            .bind(shift_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({"message":"Compra registrada","purchase_id":purchase_id,"total_amount":total})),
    ))
}

pub async fn purchase_get(
    State(pool): State<PgPool>,
    AuthUser(_user): AuthUser,
    Path(id): Path<i32>,
) -> Result<Json<Value>, AppError> {
    let row = sqlx::query_as::<
        _,
        (
            i32,
            i32,
            i32,
            Option<i32>,
            String,
            String,
            NaiveDate,
            Option<NaiveDate>,
            String,
            String,
            Decimal,
            String,
            Option<String>,
            Value,
            Value,
            Value,
        ),
    >(
        r#"
        SELECT
            p.id,
            p.provider_id,
            p.user_id,
            p.cash_shift_id,
            p.invoice_number,
            p.invoice_type::text,
            p.issue_date,
            p.due_date,
            p.payment_condition::text,
            p.payment_method::text,
            p.total_amount,
            p.status::text,
            p.notes,
            jsonb_build_object('id', pr.id, 'name', pr.name, 'phone', pr.phone, 'direction', pr.direction) AS provider,
            jsonb_build_object('id', u.id, 'username', u.username) AS "user",
            COALESCE(
                (
                    SELECT jsonb_agg(
                        jsonb_build_object(
                            'id', d.id,
                            'purchase_id', d.purchase_id,
                            'product_id', d.product_id,
                            'quantity', d.quantity,
                            'unit_cost', d.unit_cost,
                            'subtotal', d.subtotal,
                            'product', jsonb_build_object(
                                'id', prod.id,
                                'name', prod.name,
                                'barcode', prod.barcode,
                                'price', prod.price,
                                'cost_price', prod.cost_price
                            )
                        )
                        ORDER BY d.id ASC
                    )
                    FROM purchase_details d
                    LEFT JOIN products prod ON prod.id = d.product_id
                    WHERE d.purchase_id = p.id
                ),
                '[]'::jsonb
            ) AS details
        FROM purchases p
        LEFT JOIN providers pr ON pr.id = p.provider_id
        LEFT JOIN users u ON u.id = p.user_id
        WHERE p.id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(json!({
        "id": row.0,
        "provider_id": row.1,
        "user_id": row.2,
        "cash_shift_id": row.3,
        "invoice_number": row.4,
        "invoice_type": row.5,
        "issue_date": row.6,
        "due_date": row.7,
        "payment_condition": row.8,
        "payment_method": row.9,
        "total_amount": row.10,
        "status": row.11,
        "notes": row.12,
        "provider": row.13,
        "user": row.14,
        "details": row.15
    })))
}

pub async fn purchase_cancel(
    State(pool): State<PgPool>,
    AdminUser(_user): AdminUser,
    Path(id): Path<i32>,
) -> Result<Json<Value>, AppError> {
    let mut tx = pool.begin().await?;
    let purchase = sqlx::query_as::<_, (i32,Decimal,String,Option<i32>,String)>("SELECT provider_id,total_amount,payment_condition::text,cash_shift_id,payment_method::text FROM purchases WHERE id=$1 AND status='recibida' FOR UPDATE").fetch_optional(&mut *tx).await?.ok_or(AppError::NotFound)?;
    let details = sqlx::query_as::<_, (i32, i32)>(
        "SELECT product_id,quantity FROM purchase_details WHERE purchase_id=$1",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    for (product_id, quantity) in details {
        sqlx::query("UPDATE products SET stock=stock-$1,\"updatedAt\"=now() WHERE id=$2")
            .bind(quantity)
            .bind(product_id)
            .execute(&mut *tx)
            .await?;
    }
    if purchase.2 == "Cuenta corriente" {
        sqlx::query("INSERT INTO provider_accounts (provider_id,purchase_id,type,amount,description,\"createdAt\",\"updatedAt\") VALUES ($1,$2,'debit',$3,'Anulación de compra',now(),now())").bind(purchase.0).bind(id).bind(purchase.1).execute(&mut *tx).await?;
    }
    if purchase.3.is_some() && purchase.4 == "Efectivo" {
        sqlx::query(
            "UPDATE cash_shifts SET expected_cash=expected_cash+$1,\"updatedAt\"=now() WHERE id=$2 AND status='abierta'",
        )
        .bind(purchase.1)
        .bind(purchase.3)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query("UPDATE purchases SET status='anulada',\"updatedAt\"=now() WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"message": format!("Compra #{} anulada con éxito y stock revertido.", id)}),
    ))
}

pub async fn provider_balance(
    State(pool): State<PgPool>,
    AdminUser(_user): AdminUser,
    Path(id): Path<i32>,
) -> Result<Json<Value>, AppError> {
    let provider: String = sqlx::query_scalar("SELECT name FROM providers WHERE id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await?;
    let (bought, paid): (Decimal, Decimal) = sqlx::query_as("SELECT COALESCE(SUM(amount) FILTER (WHERE type='credit'),0),COALESCE(SUM(amount) FILTER (WHERE type='debit'),0) FROM provider_accounts WHERE provider_id=$1").bind(id).fetch_one(&pool).await?;
    let movements: Value = sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(a) ORDER BY a.\"createdAt\" DESC),'[]'::jsonb) FROM provider_accounts a WHERE a.provider_id=$1").bind(id).fetch_one(&pool).await?;
    Ok(Json(
        json!({"provider_id":id,"provider":provider,"total_comprado":bought,"total_pagado":paid,"saldo_actual":bought-paid,"movements":movements}),
    ))
}

pub async fn provider_payment(
    State(pool): State<PgPool>,
    AdminUser(claims): AdminUser,
    Json(payload): Json<ProviderPayment>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    validate_positive(payload.amount)?;
    let method = payload.payment_method.unwrap_or_else(|| "Efectivo".into());
    if !["Efectivo", "Transferencia", "Tarjeta"].contains(&method.as_str()) {
        return Err(AppError::BadRequest("Método de pago inválido".into()));
    }
    let balance: Decimal = sqlx::query_scalar("SELECT COALESCE(SUM(amount) FILTER (WHERE type='credit'),0)-COALESCE(SUM(amount) FILTER (WHERE type='debit'),0) FROM provider_accounts WHERE provider_id=$1")
        .bind(payload.provider_id).fetch_one(&pool).await?;
    if payload.amount > balance {
        return Err(AppError::BadRequest(
            "El pago supera el saldo del proveedor".into(),
        ));
    }
    let shift: Option<i32> = if method == "Efectivo" {
        sqlx::query_scalar("SELECT id FROM cash_shifts WHERE user_id=$1 AND status='abierta' ORDER BY opened_at DESC LIMIT 1")
            .bind(claims.sub).fetch_optional(&pool).await?
    } else {
        None
    };
    if method == "Efectivo" && shift.is_none() {
        return Err(AppError::BadRequest(
            "Se requiere un turno abierto para pagar en efectivo".into(),
        ));
    }
    let movement: i32 = sqlx::query_scalar("INSERT INTO provider_accounts (provider_id,type,amount,description,due_date,\"createdAt\",\"updatedAt\") VALUES ($1,'debit',$2,$3,$4,now(),now()) RETURNING id").bind(payload.provider_id).bind(payload.amount).bind(payload.description).bind(payload.due_date).fetch_one(&pool).await?;
    if let Some(shift_id) = shift {
        sqlx::query(
            "UPDATE cash_shifts SET expected_cash=expected_cash-$1,\"updatedAt\"=now() WHERE id=$2",
        )
        .bind(payload.amount)
        .bind(shift_id)
        .execute(&pool)
        .await?;
    }
    Ok((
        StatusCode::CREATED,
        Json(
            json!({"message":"Pago registrado","movement":{"id":movement,"amount":payload.amount,"payment_method":method}}),
        ),
    ))
}

pub async fn sales_stats(
    State(pool): State<PgPool>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Value>, AppError> {
    // 1. Ventas del día
    let (ventas_del_dia, total_facturado_hoy, cmv_hoy): (i64, Decimal, Decimal) = sqlx::query_as(
        r#"
        SELECT
            COUNT(s.id)::bigint,
            COALESCE(SUM(s.total), 0),
            COALESCE(SUM(d.cmv), 0)
        FROM sales s
        LEFT JOIN (
            SELECT sale_id, SUM(quantity * unit_cost) AS cmv
            FROM sale_details
            GROUP BY sale_id
        ) d ON d.sale_id = s.id
        WHERE s.status = 'completada'
          AND s."createdAt"::date >= CURRENT_DATE
        "#,
    )
    .fetch_one(&pool)
    .await?;

    let ganancia_hoy = if total_facturado_hoy > cmv_hoy {
        total_facturado_hoy - cmv_hoy
    } else {
        Decimal::ZERO
    };
    let ticket_promedio_hoy = if ventas_del_dia > 0 {
        (total_facturado_hoy / Decimal::from(ventas_del_dia)).round_dp(2)
    } else {
        Decimal::ZERO
    };
    let margen_hoy_porcentaje = if total_facturado_hoy > Decimal::ZERO {
        (ganancia_hoy / total_facturado_hoy * Decimal::from(100)).round_dp(1)
    } else {
        Decimal::ZERO
    };

    // 2. Inventario valorizado y stock crítico
    let productos_stock_bajo = sqlx::query_as::<_, (i32, String, i32)>(
        "SELECT id, name, stock FROM products WHERE stock <= 5 ORDER BY stock ASC",
    )
    .fetch_all(&pool)
    .await?;

    let (capital_invertido, total_unidades): (Decimal, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(stock * cost_price), 0), COALESCE(SUM(stock), 0)::bigint FROM products WHERE stock > 0",
    )
    .fetch_one(&pool)
    .await?;

    // 3. Ventas y Gastos del Mes (Punto de Equilibrio)
    let (ventas_mes, cmv_mes): (Decimal, Decimal) = sqlx::query_as(
        r#"
        SELECT
            COALESCE(SUM(s.total), 0),
            COALESCE(SUM(d.cmv), 0)
        FROM sales s
        LEFT JOIN (
            SELECT sale_id, SUM(quantity * unit_cost) AS cmv
            FROM sale_details
            GROUP BY sale_id
        ) d ON d.sale_id = s.id
        WHERE s.status = 'completada'
          AND s."createdAt"::date >= date_trunc('month', CURRENT_DATE)::date
        "#,
    )
    .fetch_one(&pool)
    .await?;

    let gastos_mes: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0) FROM expenses WHERE date >= date_trunc('month', CURRENT_DATE)::date",
    )
    .fetch_one(&pool)
    .await?;

    let ganancia_mes = if ventas_mes > cmv_mes {
        ventas_mes - cmv_mes
    } else {
        Decimal::ZERO
    };
    let margen_mes_porcentaje = if ventas_mes > Decimal::ZERO {
        (ganancia_mes / ventas_mes * Decimal::from(100)).round_dp(1)
    } else {
        Decimal::ZERO
    };
    let punto_equilibrio_ventas = if margen_mes_porcentaje > Decimal::ZERO {
        (gastos_mes / (margen_mes_porcentaje / Decimal::from(100))).round_dp(0)
    } else {
        Decimal::ZERO
    };
    let progreso_porcentaje = if punto_equilibrio_ventas > Decimal::ZERO {
        (ventas_mes / punto_equilibrio_ventas * Decimal::from(100))
            .min(Decimal::from(100))
            .round_dp(0)
    } else if gastos_mes == Decimal::ZERO && ventas_mes > Decimal::ZERO {
        Decimal::from(100)
    } else {
        Decimal::ZERO
    };
    let alcanzado =
        ventas_mes >= punto_equilibrio_ventas && punto_equilibrio_ventas > Decimal::ZERO;

    // 4. Saldo en gaveta (Turno activo del usuario actual)
    let active_shift = sqlx::query_as::<_, (i32, String, Decimal, Decimal)>(
        r#"
        SELECT cs.id, u.username, cs.expected_cash, cs.initial_cash
        FROM cash_shifts cs
        JOIN users u ON u.id = cs.user_id
        WHERE cs.status = 'abierta' AND cs.user_id = $1
        ORDER BY cs."createdAt" DESC
        LIMIT 1
        "#,
    )
    .bind(claims.sub)
    .fetch_optional(&pool)
    .await?;

    let caja = match active_shift {
        Some((shift_id, username, expected, initial)) => json!({
            "abierta": true,
            "turnoId": shift_id,
            "cajero": username,
            "efectivoEsperado": expected,
            "montoInicial": initial
        }),
        None => json!({
            "abierta": false,
            "turnoId": serde_json::Value::Null,
            "cajero": serde_json::Value::Null,
            "efectivoEsperado": 0,
            "montoInicial": 0
        }),
    };

    // 5. Cuentas Corrientes: Balanza Crediticia
    let clientes_con_deuda_rows = sqlx::query_as::<_, (i32, String, Decimal)>(
        r#"
        SELECT c.id, c.name, COALESCE(SUM(CASE WHEN ca.type = 'debit' THEN ca.amount ELSE -ca.amount END), 0) AS saldo
        FROM customers c
        JOIN customer_accounts ca ON ca.customer_id = c.id
        GROUP BY c.id, c.name
        HAVING SUM(CASE WHEN ca.type = 'debit' THEN ca.amount ELSE -ca.amount END) > 0
        ORDER BY saldo DESC
        "#,
    )
    .fetch_all(&pool)
    .await?;

    let mut total_por_cobrar = Decimal::ZERO;
    let clientes_con_deuda: Vec<Value> = clientes_con_deuda_rows
        .into_iter()
        .map(|(id, name, saldo)| {
            total_por_cobrar += saldo;
            json!({
                "id": id,
                "name": name,
                "saldo": saldo
            })
        })
        .collect();

    let total_por_pagar: Decimal = sqlx::query_scalar(
        "SELECT GREATEST(0, COALESCE(SUM(CASE WHEN type = 'credit' THEN amount ELSE -amount END), 0)) FROM provider_accounts",
    )
    .fetch_one(&pool)
    .await?;

    let diferencial_neto = total_por_cobrar - total_por_pagar;

    Ok(Json(json!({
        "ventasDelDia": ventas_del_dia,
        "productosStockBajo": productos_stock_bajo.into_iter().map(|(id, name, stock)| json!({
            "id": id,
            "name": name,
            "stock": stock
        })).collect::<Vec<_>>(),
        "clientesConDeuda": clientes_con_deuda,
        "hoy": {
            "cantidadVentas": ventas_del_dia,
            "totalFacturado": total_facturado_hoy,
            "gananciaNeta": ganancia_hoy,
            "ticketPromedio": ticket_promedio_hoy,
            "margenPorcentaje": margen_hoy_porcentaje
        },
        "inventario": {
            "capitalInvertido": capital_invertido.round_dp(2),
            "totalUnidades": total_unidades
        },
        "puntoEquilibrio": {
            "gastosMes": gastos_mes,
            "ventasMes": ventas_mes,
            "gananciaMes": ganancia_mes,
            "margenMesPorcentaje": margen_mes_porcentaje,
            "puntoEquilibrioVentas": punto_equilibrio_ventas,
            "progresoPorcentaje": progreso_porcentaje,
            "alcanzado": alcanzado
        },
        "caja": caja,
        "balanzaCrediticia": {
            "totalPorCobrar": total_por_cobrar,
            "totalPorPagar": total_por_pagar,
            "diferencialNeto": diferencial_neto
        }
    })))
}

pub async fn sales_summary(
    State(pool): State<PgPool>,
    AdminUser(_user): AdminUser,
    Query(range): Query<DateRange>,
) -> Result<Json<Value>, AppError> {
    validate_range(&range)?;
    let (count, total, discounts): (i64, Decimal, Decimal) = sqlx::query_as("SELECT COUNT(*),COALESCE(SUM(total),0),COALESCE(SUM(discount_amount),0) FROM sales WHERE status='completada' AND ($1::date IS NULL OR \"createdAt\"::date >= $1) AND ($2::date IS NULL OR \"createdAt\"::date <= $2)").bind(range.from).bind(range.to).fetch_one(&pool).await?;
    Ok(Json(
        json!({"cantidad_ventas":count,"total_facturado":total,"ticket_promedio":if count == 0 {Decimal::ZERO} else {(total/Decimal::from(count)).round_dp(2)},"total_descuentos":discounts}),
    ))
}

pub async fn top_products(
    State(pool): State<PgPool>,
    AdminUser(_user): AdminUser,
    Query(range): Query<DateRange>,
) -> Result<Json<Vec<Value>>, AppError> {
    validate_range(&range)?;
    let rows = sqlx::query_as::<_, (i32, String, i64, Decimal, Decimal)>(
        r#"
        SELECT
            d.product_id,
            p.name,
            SUM(d.quantity)::bigint,
            SUM(d.quantity * d.unit_price),
            SUM(d.quantity * d.unit_cost)
        FROM sale_details d
        JOIN sales s ON s.id = d.sale_id
        JOIN products p ON p.id = d.product_id
        WHERE s.status = 'completada'
          AND ($1::date IS NULL OR s."createdAt"::date >= $1)
          AND ($2::date IS NULL OR s."createdAt"::date <= $2)
        GROUP BY d.product_id, p.name
        ORDER BY SUM(d.quantity) DESC
        LIMIT 10
        "#,
    )
    .bind(range.from)
    .bind(range.to)
    .fetch_all(&pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, quantity, billed, cmv)| {
                let margin = billed - cmv;
                let pct = if billed.is_zero() {
                    Decimal::ZERO
                } else {
                    (margin / billed * Decimal::from(100)).round_dp(1)
                };
                json!({
                    "producto_id": id,
                    "nombre": name,
                    "cantidad_vendida": quantity,
                    "total_facturado": billed.round_dp(2),
                    "total_cmv": cmv.round_dp(2),
                    "margen_bruto": margin.round_dp(2),
                    "porcentaje_margen": pct
                })
            })
            .collect(),
    ))
}

pub async fn sales_by_user(
    State(pool): State<PgPool>,
    AdminUser(_user): AdminUser,
    Query(range): Query<DateRange>,
) -> Result<Json<Vec<Value>>, AppError> {
    validate_range(&range)?;
    let rows = sqlx::query_as::<_, (i32, String, String, Decimal, String)>(
        r#"
        SELECT
            s.user_id,
            COALESCE(u.username, 'Usuario #' || s.user_id::text) AS username,
            COALESCE(u.role::text, 'Empleado') AS role,
            s.total,
            s."paymentMethod"::text
        FROM sales s
        LEFT JOIN users u ON u.id = s.user_id
        WHERE s.status = 'completada'
          AND ($1::date IS NULL OR s."createdAt"::date >= $1)
          AND ($2::date IS NULL OR s."createdAt"::date <= $2)
        "#,
    )
    .bind(range.from)
    .bind(range.to)
    .fetch_all(&pool)
    .await?;

    struct UserEntry {
        user_id: i32,
        username: String,
        role: String,
        cantidad_ventas: i64,
        total_facturado: Decimal,
        metodos: std::collections::BTreeMap<String, i64>,
    }

    let mut map: std::collections::BTreeMap<i32, UserEntry> = std::collections::BTreeMap::new();

    for (user_id, username, role, total, payment_method) in rows {
        let entry = map.entry(user_id).or_insert_with(|| UserEntry {
            user_id,
            username,
            role,
            cantidad_ventas: 0,
            total_facturado: Decimal::ZERO,
            metodos: std::collections::BTreeMap::new(),
        });
        entry.cantidad_ventas += 1;
        entry.total_facturado += total;
        *entry.metodos.entry(payment_method).or_insert(0) += 1;
    }

    let mut result: Vec<_> = map.into_values().collect();
    result.sort_by_key(|a| std::cmp::Reverse(a.total_facturado));

    Ok(Json(
        result
            .into_iter()
            .map(|u| {
                let ticket_promedio = if u.cantidad_ventas > 0 {
                    (u.total_facturado / Decimal::from(u.cantidad_ventas)).round_dp(2)
                } else {
                    Decimal::ZERO
                };
                json!({
                    "user_id": u.user_id,
                    "username": u.username,
                    "role": u.role,
                    "cantidad_ventas": u.cantidad_ventas,
                    "total_facturado": u.total_facturado.round_dp(2),
                    "ticket_promedio": ticket_promedio,
                    "metodos": u.metodos
                })
            })
            .collect(),
    ))
}

pub async fn financial_dashboard(
    State(pool): State<PgPool>,
    AdminUser(_user): AdminUser,
    Query(range): Query<DateRange>,
) -> Result<Json<Value>, AppError> {
    validate_range(&range)?;

    // 1. Ventas completadas del período
    let (count_sales, total_ventas_neto, total_subtotal_bruto, total_descuentos, efectivo_ingresado, total_cmv): (
        i64,
        Decimal,
        Decimal,
        Decimal,
        Decimal,
        Decimal,
    ) = sqlx::query_as(
        r#"
        SELECT
            COUNT(s.id)::bigint AS count_sales,
            COALESCE(SUM(s.total), 0) AS total_ventas_neto,
            COALESCE(SUM(COALESCE(s.subtotal, s.total)), 0) AS total_subtotal_bruto,
            COALESCE(SUM(s.discount_amount), 0) AS total_descuentos,
            COALESCE(SUM(CASE WHEN s."paymentMethod" = 'Efectivo' THEN s.total ELSE 0 END), 0) AS efectivo_ingresado,
            COALESCE(SUM(d.cmv), 0) AS total_cmv
        FROM sales s
        LEFT JOIN (
            SELECT sale_id, SUM(quantity * unit_cost) AS cmv
            FROM sale_details
            GROUP BY sale_id
        ) d ON d.sale_id = s.id
        WHERE s.status = 'completada'
          AND ($1::date IS NULL OR s."createdAt"::date >= $1)
          AND ($2::date IS NULL OR s."createdAt"::date <= $2)
        "#,
    )
    .bind(range.from)
    .bind(range.to)
    .fetch_one(&pool)
    .await?;

    let margen_bruto_total = total_ventas_neto - total_cmv;
    let porcentaje_margen_bruto = if total_ventas_neto > Decimal::ZERO {
        (margen_bruto_total / total_ventas_neto * Decimal::from(100)).round_dp(2)
    } else {
        Decimal::ZERO
    };

    // 2. Costos Fijos del período
    let expense_rows = sqlx::query_as::<_, (String, Decimal, Decimal)>(
        r#"
        SELECT
            category,
            COALESCE(SUM(amount), 0) AS cat_total,
            COALESCE(SUM(CASE WHEN "paymentMethod" = 'Efectivo' THEN amount ELSE 0 END), 0) AS cat_cash
        FROM expenses
        WHERE ($1::date IS NULL OR date >= $1)
          AND ($2::date IS NULL OR date <= $2)
        GROUP BY category
        "#,
    )
    .bind(range.from)
    .bind(range.to)
    .fetch_all(&pool)
    .await?;

    let mut desglose: std::collections::BTreeMap<String, Decimal> = [
        ("Estructura".to_string(), Decimal::ZERO),
        ("Herramientas".to_string(), Decimal::ZERO),
        ("Personal".to_string(), Decimal::ZERO),
        ("Marketing".to_string(), Decimal::ZERO),
    ]
    .into_iter()
    .collect();

    let mut costos_fijos_total = Decimal::ZERO;
    let mut efectivo_egresado = Decimal::ZERO;

    for (cat, cat_total, cat_cash) in expense_rows {
        costos_fijos_total += cat_total;
        efectivo_egresado += cat_cash;
        *desglose.entry(cat).or_insert(Decimal::ZERO) += cat_total;
    }

    // 3. Punto de Equilibrio ($) = Total Costos Fijos / % Margen Bruto
    let (punto_equilibrio_pesos, cobertura_porcentaje) = if porcentaje_margen_bruto > Decimal::ZERO
    {
        let pe = (costos_fijos_total / (porcentaje_margen_bruto / Decimal::from(100))).round_dp(2);
        let cob = if pe > Decimal::ZERO {
            (total_ventas_neto / pe * Decimal::from(100)).round_dp(2)
        } else {
            Decimal::ZERO
        };
        (pe, cob)
    } else {
        (Decimal::ZERO, Decimal::ZERO)
    };

    let superado =
        total_ventas_neto >= punto_equilibrio_pesos && punto_equilibrio_pesos > Decimal::ZERO;
    let utilidad_neta = margen_bruto_total - costos_fijos_total;
    let es_rentable = utilidad_neta > Decimal::ZERO;
    let flujo_caja_efectivo_neto = efectivo_ingresado - efectivo_egresado;

    Ok(Json(json!({
        "resumen_ventas": {
            "total_ventas_neto": total_ventas_neto.round_dp(2),
            "total_subtotal_bruto": total_subtotal_bruto.round_dp(2),
            "total_descuentos": total_descuentos.round_dp(2),
            "cantidad_transacciones": count_sales
        },
        "costos_variables_cmv": {
            "total_cmv": total_cmv.round_dp(2),
            "margen_bruto_total": margen_bruto_total.round_dp(2),
            "porcentaje_margen_bruto": porcentaje_margen_bruto.round_dp(2)
        },
        "costos_fijos_operativos": {
            "total_costos_fijos": costos_fijos_total.round_dp(2),
            "desglose": desglose
        },
        "punto_de_equilibrio": {
            "punto_equilibrio_pesos": punto_equilibrio_pesos.round_dp(2),
            "cobertura_porcentaje": cobertura_porcentaje.round_dp(2),
            "superado": superado
        },
        "resultado_final": {
            "utilidad_neta": utilidad_neta.round_dp(2),
            "es_rentable": es_rentable,
            "flujo_caja_efectivo_neto": flujo_caja_efectivo_neto.round_dp(2)
        }
    })))
}

#[derive(Debug, Serialize)]
pub struct ComplianceCheckItem {
    pub id: &'static str,
    pub category: &'static str,
    pub requirement: &'static str,
    pub status: &'static str,
    pub severity: &'static str,
    #[serde(rename = "currentImplementation")]
    pub current_implementation: &'static str,
    #[serde(rename = "technicalGap")]
    pub technical_gap: &'static str,
    pub recommendation: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ComplianceReport {
    pub timestamp: String,
    #[serde(rename = "systemName")]
    pub system_name: &'static str,
    #[serde(rename = "overallScorePercentage")]
    pub overall_score_percentage: u32,
    #[serde(rename = "totalChecks")]
    pub total_checks: usize,
    pub passed: usize,
    pub partial: usize,
    pub failed: usize,
    pub items: Vec<ComplianceCheckItem>,
}

pub fn run_financial_compliance_audit() -> ComplianceReport {
    let items = vec![
        ComplianceCheckItem {
            id: "REG-01",
            category: "REGISTRO_DIARIO",
            requirement: "Ingresos de Ventas: Fecha exacta de la transacción",
            status: "COMPLIANT",
            severity: "LOW",
            current_implementation: "Modelo Sale cuenta con atributo 'createdAt' tipo DataTypes.DATE.",
            technical_gap: "Ninguno.",
            recommendation: "Mantener timezone normalizado en base de datos.",
        },
        ComplianceCheckItem {
            id: "REG-02",
            category: "REGISTRO_DIARIO",
            requirement: "Ingresos de Ventas: Detalle de producto/servicio (modelo o variante)",
            status: "PARTIAL",
            severity: "MEDIUM",
            current_implementation: "Modelo SaleDetail con product_id y relación a Product ('name', 'barcode').",
            technical_gap: "No existe entidad ProductVariant explícita para SKU jerárquicos.",
            recommendation: "Evaluar incorporación de atributos de variante en próximos sprints.",
        },
        ComplianceCheckItem {
            id: "REG-03",
            category: "REGISTRO_DIARIO",
            requirement: "Ingresos de Ventas: Cantidad de unidades entregadas",
            status: "COMPLIANT",
            severity: "LOW",
            current_implementation: "SaleDetail model implementa 'quantity: DataTypes.INTEGER'.",
            technical_gap: "Ninguno para unidades enteras.",
            recommendation: "Evaluar migración a DECIMAL(10,3) si se comercializan productos por kilogramo o fraccionados.",
        },
        ComplianceCheckItem {
            id: "REG-04",
            category: "REGISTRO_DIARIO",
            requirement: "Ingresos de Ventas: Métodos de Pago (Efectivo, Transferencia, Tarjeta de Débito o Crédito)",
            status: "COMPLIANT",
            severity: "LOW",
            current_implementation: "Sale.paymentMethod soporta ENUM('Efectivo', 'Débito', 'Cuenta corriente', 'Transferencia', 'Tarjeta de crédito').",
            technical_gap: "Ninguno para cobros simples.",
            recommendation: "Evaluar tabla intermedia para cobros divididos en un mismo ticket.",
        },
        ComplianceCheckItem {
            id: "REG-05",
            category: "REGISTRO_DIARIO",
            requirement: "Ingresos de Ventas: Monto Neto (libre de descuentos inmediatos)",
            status: "COMPLIANT",
            severity: "LOW",
            current_implementation: "Modelo Sale incluye 'subtotal', 'discount_amount' y 'total' neto calculado.",
            technical_gap: "Ninguno. Trazabilidad completa de subtotal bruto, descuentos aplicados y neto.",
            recommendation: "Mantener validación de descuentos positivos en el controlador.",
        },
        ComplianceCheckItem {
            id: "REG-06",
            category: "REGISTRO_DIARIO",
            requirement: "Costos Variables (CMV): Costo de Adquisición (Precio de compra mayorista)",
            status: "COMPLIANT",
            severity: "LOW",
            current_implementation: "Product posee 'cost_price' y SaleDetail congela 'unit_cost' histórico al momento de la venta.",
            technical_gap: "Ninguno. Permite cálculo exacto del CMV histórico sin alterar transacciones pasadas.",
            recommendation: "Implementar importación masiva de listas de costos de proveedores.",
        },
        ComplianceCheckItem {
            id: "REG-07",
            category: "REGISTRO_DIARIO",
            requirement: "Costos Variables (CMV): Logística (flete, envíos, empaques por lote)",
            status: "PARTIAL",
            severity: "MEDIUM",
            current_implementation: "Los costos de logística pueden imputarse como egreso en Expenses o sumarse al cost_price unitario.",
            technical_gap: "Falta entidad InwardShipment para prorrateo automático de fletes entre productos de un lote.",
            recommendation: "Diseñar módulo de Prorrateo de Flete en Sprint 2.",
        },
        ComplianceCheckItem {
            id: "REG-08",
            category: "REGISTRO_DIARIO",
            requirement: "Costos Variables (CMV): Comisiones retenidas por pasarelas de pago o terminales POS",
            status: "PARTIAL",
            severity: "MEDIUM",
            current_implementation: "Se pueden asentar en Expenses (Herramientas) o deducir en conciliación.",
            technical_gap: "No hay cálculo automático de comisión porcentual por método de cobro en tiempo real.",
            recommendation: "Crear tabla de alícuotas configurables por pasarela (ej. MercadoPago 3.5%).",
        },
        ComplianceCheckItem {
            id: "REG-09",
            category: "REGISTRO_DIARIO",
            requirement: "Costos Fijos: Registro categorizado (Estructura, Herramientas, Personal, Marketing)",
            status: "COMPLIANT",
            severity: "LOW",
            current_implementation: "Modelo Expense implementado con enum de 4 categorías obligatorias, CRUD en Express y vista ExpensesView en frontend.",
            technical_gap: "Ninguno. Totalmente categorizado y filtrable por fecha y tipo.",
            recommendation: "Agregar adjunto de archivo en comprobantes.",
        },
        ComplianceCheckItem {
            id: "MET-01",
            category: "METRICAS_MENSUALES",
            requirement: "Margen Bruto Unitario = Precio de Venta - Costo Variable",
            status: "COMPLIANT",
            severity: "LOW",
            current_implementation: "reportController (/financial-dashboard) calcula CMV total y Margen Bruto ($ y %) por período y por producto.",
            technical_gap: "Ninguno.",
            recommendation: "Visualizar margen por categoría en gráficos de dispersión.",
        },
        ComplianceCheckItem {
            id: "MET-02",
            category: "METRICAS_MENSUALES",
            requirement: "Punto de Equilibrio ($) = Total Costos Fijos Mensuales / % de Margen Bruto",
            status: "COMPLIANT",
            severity: "LOW",
            current_implementation: "Calculado dinámicamente en reportController (/financial-dashboard) y graficado con barra de avance en ReportsView.vue.",
            technical_gap: "Ninguno.",
            recommendation: "Agregar alerta en Dashboard si las ventas están por debajo del umbral a mitad de mes.",
        },
        ComplianceCheckItem {
            id: "MET-03",
            category: "METRICAS_MENSUALES",
            requirement: "Conciliación Flujo de Caja vs Utilidad (Cash Flow real vs Utilidad contable devengada)",
            status: "COMPLIANT",
            severity: "LOW",
            current_implementation: "El dashboard financiero discrimina Utilidad Neta (resultado contable devengado) de Flujo de Caja Neto en Efectivo.",
            technical_gap: "Ninguno a nivel operativo.",
            recommendation: "Integrar cuentas bancarias en Sprint 3.",
        },
        ComplianceCheckItem {
            id: "RUT-01",
            category: "RUTINA_EJECUCION",
            requirement: "Rutina Diaria: Asiento manual 100% de ventas y compras en el día",
            status: "COMPLIANT",
            severity: "LOW",
            current_implementation: "Ventas y compras asentadas con comprobante en tiempo real (módulos Sales y Purchases con cálculo PPP).",
            technical_gap: "Ninguno.",
            recommendation: "Mantener validación estricta de proveedores.",
        },
        ComplianceCheckItem {
            id: "RUT-02",
            category: "RUTINA_EJECUCION",
            requirement: "Rutina Diaria: Digitalización de tickets y facturas en la nube",
            status: "PARTIAL",
            severity: "MEDIUM",
            current_implementation: "Expense y Purchase models cuentan con soporte de comprobante y facturación formal.",
            technical_gap: "Requiere integración de bucket S3/Cloudinary/R2 para carga directa desde el navegador.",
            recommendation: "Integrar servicio de upload presignado S3 en Sprint 3.",
        },
        ComplianceCheckItem {
            id: "RUT-03",
            category: "RUTINA_EJECUCION",
            requirement: "Rutina Diaria: Arqueo de Caja (verificar efectivo físico vs sistema al cierre)",
            status: "COMPLIANT",
            severity: "LOW",
            current_implementation: "CashShift model y cashShiftController implementan apertura con fondo inicial, cálculo teórico de efectivo, conteo físico y cálculo de sobrante/faltante.",
            technical_gap: "Ninguno.",
            recommendation: "Añadir modal de apertura rápida al iniciar sesión si el usuario no tiene turno abierto.",
        },
        ComplianceCheckItem {
            id: "RUT-04",
            category: "RUTINA_EJECUCION",
            requirement: "Rutina Semanal: Reporte de cuentas por cobrar (clientes con saldos pendientes)",
            status: "COMPLIANT",
            severity: "LOW",
            current_implementation: "Implementado en customerAccountController (getCustomerBalance) y getStats (clientesConDeuda).",
            technical_gap: "Ninguno.",
            recommendation: "Agregar exportación a PDF para cobradores.",
        },
        ComplianceCheckItem {
            id: "RUT-05",
            category: "RUTINA_EJECUCION",
            requirement: "Rutina Semanal: Cuentas por Pagar (programar pagos a proveedores según vencimiento)",
            status: "COMPLIANT",
            severity: "LOW",
            current_implementation: "Modelo ProviderAccount implementado con registro de deudas por compra a plazo y seguimiento de fechas de vencimiento (due_date).",
            technical_gap: "Ninguno.",
            recommendation: "Visualizar calendario de vencimientos en la vista de proveedores.",
        },
        ComplianceCheckItem {
            id: "RUT-06",
            category: "RUTINA_EJECUCION",
            requirement: "Rutina Semanal: Conciliación bancaria (cruzar extractos con libro de control)",
            status: "PARTIAL",
            severity: "MEDIUM",
            current_implementation: "Se registran métodos de pago Transferencia y Tarjeta en ventas y gastos.",
            technical_gap: "No hay importación de extracto CSV bancario.",
            recommendation: "Desarrollar importador de extractos en Sprint 3.",
        },
        ComplianceCheckItem {
            id: "RUT-07",
            category: "RUTINA_EJECUCION",
            requirement: "Rutina Mensual: Cierre de Resultados y Utilidad Neta (primeros 3 días hábiles)",
            status: "COMPLIANT",
            severity: "LOW",
            current_implementation: "Endpoint /financial-dashboard y vista ReportsView consolidan ingresos, CMV, costos fijos y utilidad neta en cualquier período mensual.",
            technical_gap: "Ninguno.",
            recommendation: "Agregar botón de exportación mensual de balance.",
        },
        ComplianceCheckItem {
            id: "RUT-08",
            category: "RUTINA_EJECUCION",
            requirement: "Rutina Mensual: Análisis de facturación real vs Punto de Equilibrio",
            status: "COMPLIANT",
            severity: "LOW",
            current_implementation: "ReportsView.vue incluye tarjeta de Punto de Equilibrio con barra de avance porcentual y estado de cobertura superado/pendiente.",
            technical_gap: "Ninguno.",
            recommendation: "Mantener sincronización reactiva con filtros de fecha.",
        },
        ComplianceCheckItem {
            id: "RUT-09",
            category: "RUTINA_EJECUCION",
            requirement: "Rutina Mensual: Provisiones (separar montos para impuestos, sueldos y reposición de stock)",
            status: "PARTIAL",
            severity: "MEDIUM",
            current_implementation: "La Utilidad Neta calculada y el desglose de Costos Fijos por Personal y Estructura permiten proyectar la reserva contable.",
            technical_gap: "Falta widget de cálculo automático de reservas porcentuales sugeridas (ej. 15% impuestos, 20% reinversión).",
            recommendation: "Agregar módulo de Reservas y Provisiones en Sprint 2.",
        },
    ];

    let total_checks = items.len();
    let passed = items.iter().filter(|i| i.status == "COMPLIANT").count();
    let partial = items.iter().filter(|i| i.status == "PARTIAL").count();
    let failed = items.iter().filter(|i| i.status == "NON_COMPLIANT").count();

    let score = (((passed as f64 * 1.0 + partial as f64 * 0.5) / total_checks as f64) * 100.0)
        .round() as u32;

    ComplianceReport {
        timestamp: Utc::now().to_rfc3339(),
        system_name: "KioskoFlow POS & Financial Management System",
        overall_score_percentage: score,
        total_checks,
        passed,
        partial,
        failed,
        items,
    }
}

pub async fn financial_compliance(
    State(_pool): State<PgPool>,
    AdminUser(_user): AdminUser,
) -> Result<Json<ComplianceReport>, AppError> {
    Ok(Json(run_financial_compliance_audit()))
}

fn validate_range(range: &DateRange) -> Result<(), AppError> {
    if range.from.zip(range.to).is_some_and(|(from, to)| from > to) {
        Err(AppError::BadRequest(
            "El rango de fechas es inválido".into(),
        ))
    } else {
        Ok(())
    }
}

#[allow(dead_code)]
async fn _tx<'a>(tx: &mut Transaction<'a, Postgres>) -> Result<(), AppError> {
    let _ = tx;
    Ok(())
}
