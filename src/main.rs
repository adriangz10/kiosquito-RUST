mod auth;
mod db;
mod errors;
mod extractors;
mod handlers;
mod middleware;
mod models;
mod routes;

use axum::{
    http::{header, HeaderValue, Method},
    routing::get,
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let pool = db::connect().await;

    let app = Router::new()
        .route("/health", get(handlers::health::check))
        .merge(routes::api_routes())
        .layer(axum::middleware::from_fn_with_state(
            pool.clone(),
            middleware::audit::audit,
        ))
        .layer(
            CorsLayer::new()
                .allow_origin(
                    std::env::var("FRONTEND_URL")
                        .unwrap_or_else(|_| "http://localhost:5173".to_string())
                        .parse::<HeaderValue>()
                        .expect("FRONTEND_URL inválida"),
                )
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::PATCH,
                    Method::DELETE,
                ])
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
                .allow_credentials(true),
        )
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
