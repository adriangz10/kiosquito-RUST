use sqlx::{postgres::PgPoolOptions, PgPool};

pub async fn connect() -> PgPool {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL no seteada");

    PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("No se pudo conectar a la base de datos")
}
