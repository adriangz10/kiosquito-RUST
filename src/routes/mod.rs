use axum::{
    middleware as axum_middleware,
    routing::{get, post},
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
    Router::new().route("/auth/login", post(handlers::auth::login))
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
        // el .layer() acá es el "AuthGuard global" — corre ANTES de llegar al handler
        .layer(axum::middleware::from_fn(middleware::auth::auth_guard))
}

pub fn api_routes() -> Router<PgPool> {
    Router::new().nest(
        "/api",
        Router::new()
            .merge(products_routes())
            .merge(categories_routes())
            .merge(providers_routes())
            .merge(auth_routes())
            .merge(users_routes())
            .merge(financial_routes()),
    )
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
        .route("/sales", post(handlers::financial::sale_create))
        .route("/sales/history", get(handlers::financial::sales_history))
        .route("/sales/{id}", get(handlers::financial::sale_get))
        .route(
            "/sales/{id}/anular",
            axum::routing::patch(handlers::financial::sale_cancel)
                .put(handlers::financial::sale_cancel),
        )
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
