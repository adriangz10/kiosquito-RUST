use axum::{
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
            "/products/{id}",
            get(handlers::products::get_one)
                .put(handlers::products::update)
                .delete(handlers::products::delete),
        )
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
}

pub fn auth_routes() -> Router<PgPool> {
    // Públicas, sin guard
    Router::new()
        .route("/auth/register", post(handlers::auth::register))
        .route("/auth/login", post(handlers::auth::login))
}

pub fn users_routes() -> Router<PgPool> {
    Router::new()
        .route("/users", get(handlers::users::list))
        .route(
            "/users/{id}",
            axum::routing::delete(handlers::users::delete),
        )
        // el .layer() acá es el "AuthGuard global" — corre ANTES de llegar al handler
        .layer(axum::middleware::from_fn(middleware::auth::auth_guard))
}
