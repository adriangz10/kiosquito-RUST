use sqlx::{postgres::PgPoolOptions, PgPool};

use crate::auth::hash_password;

pub async fn connect() -> PgPool {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL no seteada");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("No se pudo conectar a la base de datos");

    sqlx::query(
        r#"INSERT INTO customers (id, name, "creditMax", allow_credit, "createdAt", "updatedAt")
           VALUES (1, 'Consumidor Final', 0, false, now(), now())
           ON CONFLICT (id) DO NOTHING"#,
    )
    .execute(&pool)
    .await
    .expect("No se pudo inicializar Consumidor Final");
    sqlx::query("SELECT setval('customers_id_seq', GREATEST((SELECT COALESCE(MAX(id), 1) FROM customers), 1), true)")
        .execute(&pool).await.expect("No se pudo sincronizar customers_id_seq");

    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .expect("No se pudo consultar usuarios");
    if users == 0 {
        let (admin_user, admin_pass) = match (
            std::env::var("INITIAL_ADMIN_USERNAME"),
            std::env::var("INITIAL_ADMIN_PASSWORD"),
        ) {
            (Ok(u), Ok(p)) => (u, p),
            _ => ("admin".to_string(), "admin123".to_string()),
        };
        if admin_pass.chars().count() < 8 {
            panic!("La contraseña del administrador debe tener al menos 8 caracteres");
        }
        let hashed =
            hash_password(&admin_pass).expect("No se pudo cifrar el administrador inicial");
        sqlx::query("INSERT INTO users (username,role,password,\"createdAt\",\"updatedAt\") VALUES ($1,'Administrador',$2,now(),now())")
            .bind(admin_user).bind(hashed).execute(&pool).await.expect("No se pudo crear el administrador inicial");
    }

    pool
}
