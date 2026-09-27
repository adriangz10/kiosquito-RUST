use axum::{
    middleware as axum_middleware,
    routing::{get, post, put},
    Router,
};
use sqlx::PgPool;

use crate::{handlers, middleware};

pub fn products_routes() -> Router<PgPool> {
    Router::new()
        .route(
            "/products",
            get(handlers::products::list).post(handlers::products::create),
        )
        .route(
            "/products/barcode/{barcode}",
            get(handlers::products::get_by_barcode),
        )
        // Contrato 3 (Fase 7.1) y BR-PRE-001: Aumento Masivo de Precios
        .route(
            "/products/aumento-masivo",
            put(handlers::products::aumento_masivo),
        )
        .route(
            "/productos/aumento-masivo",
            put(handlers::products::aumento_masivo),
        )
        .route(
            "/products/{id}",
            get(handlers::products::get_one)
                .put(handlers::products::update)
                .delete(handlers::products::delete),
        )
        .layer(axum_middleware::from_fn(
            crate::middleware::auth::auth_guard,
        ))
}

pub fn categories_routes() -> Router<PgPool> {
    Router::new()
        .route(
            "/categories",
            get(handlers::categories::list).post(handlers::categories::create),
        )
        .route(
            "/categories/{id}",
            get(handlers::categories::get_one)
                .put(handlers::categories::update)
                .delete(handlers::categories::delete),
        )
        .layer(axum_middleware::from_fn(
            crate::middleware::auth::auth_guard,
        ))
}

pub fn providers_routes() -> Router<PgPool> {
    Router::new()
        .route(
            "/providers",
            get(handlers::providers::list).post(handlers::providers::create),
        )
        .route(
            "/providers/{id}",
            get(handlers::providers::get_one)
                .put(handlers::providers::update)
                .delete(handlers::providers::delete),
        )
        .layer(axum_middleware::from_fn(
            crate::middleware::auth::auth_guard,
        ))
}

pub fn auth_routes() -> Router<PgPool> {
    // Públicas, sin guard
    Router::new()
        .route("/auth/login", post(handlers::auth::login))
        .route(
            "/auth/verify-supervisor-pin",
            post(handlers::users::verify_supervisor_pin),
        )
}

pub fn users_routes() -> Router<PgPool> {
    Router::new()
        .route(
            "/users",
            get(handlers::users::list).post(handlers::users::create),
        )
        .route(
            "/auth/users",
            get(handlers::users::list).post(handlers::users::create),
        )
        .route(
            "/users/{id}",
            axum::routing::delete(handlers::users::delete).put(handlers::users::update),
        )
        .route(
            "/auth/users/{id}",
            axum::routing::delete(handlers::users::delete).put(handlers::users::update),
        )
        .layer(axum::middleware::from_fn(middleware::auth::auth_guard))
}

pub fn api_routes() -> Router<PgPool> {
    let subroutes = Router::new()
        .merge(products_routes())
        .merge(categories_routes())
        .merge(providers_routes())
        .merge(auth_routes())
        .merge(users_routes())
        .merge(financial_routes());

    // Soporte simultáneo para /api y /api/v1 especificado en el PDF
    Router::new()
        .nest("/api", subroutes.clone())
        .nest("/api/v1", subroutes)
}

fn financial_routes() -> Router<PgPool> {
    Router::new()
        .route(
            "/customers",
            get(handlers::financial::customers).post(handlers::financial::customer_create),
        )
        .route(
            "/customers/search",
            get(handlers::financial::customer_search),
        )
        .route(
            "/customers/{id}",
            get(handlers::financial::customer_get).put(handlers::financial::customer_update),
        )
        .route(
            "/customer-accounts/payment",
            post(handlers::financial::customer_payment),
        )
        .route(
            "/customer-accounts/balance/{id}",
            get(handlers::financial::customer_balance_endpoint),
        )
        .route(
            "/customer-accounts/history/{id}",
            get(handlers::financial::customer_history),
        )
        .route(
            "/expenses",
            get(handlers::financial::expenses).post(handlers::financial::expense_create),
        )
        .route(
            "/expenses/{id}",
            axum::routing::delete(handlers::financial::expense_delete),
        )
        .route(
            "/expenses/summary",
            get(handlers::financial::expense_summary),
        )
        // Cajas
        .route("/cash-shifts/open", post(handlers::financial::cash_open))
        .route(
            "/cash-shifts/current",
            get(handlers::financial::cash_current),
        )
        .route("/cash-shifts/close", post(handlers::financial::cash_close))
        .route(
            "/cash-shifts/history",
            get(handlers::financial::cash_history),
        )
        // Contrato 2 (Fase 7.1): Cierre Ciego de Caja
        .route("/cajas/cierres", post(handlers::financial::pos_cierre_ciego))
        // RF-CAJ-002: Sangrías y movimientos de caja
        .route(
            "/cajas/movimientos",
            post(handlers::financial::caja_movimiento_create),
        )
        .route(
            "/cajas/movimientos/{caja_sesion_id}",
            get(handlers::financial::caja_movimientos_list),
        )
        // Ventas y POS
        .route("/sales", post(handlers::financial::sale_create))
        .route("/sales/history", get(handlers::financial::sales_history))
        .route("/sales/{id}", get(handlers::financial::sale_get))
        .route(
            "/sales/{id}/anular",
            axum::routing::patch(handlers::financial::sale_cancel)
                .put(handlers::financial::sale_cancel),
        )
        // Contrato 1 (Fase 7.1) y BR-POS-004: Venta Multimedio
        .route("/pos/ventas", post(handlers::financial::pos_venta_create))
        // Inventario, Kardex (BR-INV-002) y Vencimientos (RF-INV-003)
        .route(
            "/inventario/kardex",
            get(handlers::financial::kardex_list),
        )
        .route(
            "/inventario/ajuste",
            post(handlers::financial::inventario_ajuste),
        )
        .route(
            "/inventario/vencimientos",
            get(handlers::financial::vencimientos_list),
        )
        .route(
            "/inventario/lotes",
            post(handlers::financial::lote_create),
        )
        // Compras y Proveedores
        .route(
            "/purchases",
            get(handlers::financial::purchases).post(handlers::financial::purchase_create),
        )
        .route("/purchases/{id}", get(handlers::financial::purchase_get))
        .route(
            "/purchases/{id}/anular",
            post(handlers::financial::purchase_cancel),
        )
        .route(
            "/provider-accounts/balance/{id}",
            get(handlers::financial::provider_balance),
        )
        .route(
            "/provider-accounts/payment",
            post(handlers::financial::provider_payment),
        )
        .route("/sales/stats", get(handlers::financial::sales_stats))
        .route(
            "/reports/sales-summary",
            get(handlers::financial::sales_summary),
        )
        .route(
            "/reports/top-products",
            get(handlers::financial::top_products),
        )
        .route(
            "/reports/sales-by-user",
            get(handlers::financial::sales_by_user),
        )
        .route(
            "/reports/financial-compliance",
            get(handlers::financial::financial_compliance),
        )
        .route(
            "/reports/financial-dashboard",
            get(handlers::financial::financial_dashboard),
        )
        .layer(axum_middleware::from_fn(
            crate::middleware::auth::auth_guard,
        ))
}
