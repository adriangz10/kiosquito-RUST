mod auth;
mod db;
mod errors;
mod extractors;
mod handlers;
mod middleware;
mod models;
mod routes;

use axum::{routing::get, Router};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let pool = db::connect().await;

    let app = Router::new()
        .route("/health", get(handlers::health::check))
        .merge(routes::products_routes())
        .merge(routes::categories_routes())
        .merge(routes::providers_routes())
        .merge(routes::auth_routes())
        .merge(routes::users_routes())
        .with_state(pool);

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .unwrap();
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("Servidor corriendo en http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
