use sqlx::{postgres::PgPoolOptions, PgPool};

use crate::auth::hash_password;

pub async fn connect() -> PgPool {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL no seteada");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("No se pudo conectar a la base de datos");

    run_migrations(&pool).await;

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

async fn run_migrations(pool: &PgPool) {
    // 1. Ampliación de roles
    let _ = sqlx::query("ALTER TYPE enum_users_role ADD VALUE IF NOT EXISTS 'Dueño'").execute(pool).await;
    let _ = sqlx::query("ALTER TYPE enum_users_role ADD VALUE IF NOT EXISTS 'Dueno'").execute(pool).await;
    let _ = sqlx::query("ALTER TYPE enum_users_role ADD VALUE IF NOT EXISTS 'Encargado'").execute(pool).await;
    let _ = sqlx::query("ALTER TYPE enum_users_role ADD VALUE IF NOT EXISTS 'Cajero'").execute(pool).await;
    let _ = sqlx::query("ALTER TYPE enum_users_role ADD VALUE IF NOT EXISTS 'Repositor'").execute(pool).await;

    // 2. Columnas adicionales en tablas existentes
    let _ = sqlx::query("ALTER TABLE users ADD COLUMN IF NOT EXISTS pin_supervisor VARCHAR(255) NULL").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE products ADD COLUMN IF NOT EXISTS min_margin NUMERIC(5,2) NOT NULL DEFAULT 0.00").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE products ADD COLUMN IF NOT EXISTS brand VARCHAR(100) NULL").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE products ALTER COLUMN stock TYPE NUMERIC(12, 3)").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE sales ADD COLUMN IF NOT EXISTS tipo_comprobante VARCHAR(50) NOT NULL DEFAULT 'TICKET_NO_FISCAL'").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE sale_details ALTER COLUMN quantity TYPE NUMERIC(12, 3)").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE cash_shifts ADD COLUMN IF NOT EXISTS tarjetas_declarado NUMERIC(12, 2) NULL").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE cash_shifts ADD COLUMN IF NOT EXISTS transferencias_declarado NUMERIC(12, 2) NULL").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE cash_shifts ADD COLUMN IF NOT EXISTS expected_cards NUMERIC(12, 2) NOT NULL DEFAULT 0.00").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE cash_shifts ADD COLUMN IF NOT EXISTS expected_transfers NUMERIC(12, 2) NOT NULL DEFAULT 0.00").execute(pool).await;

    // 3. Tabla venta_pagos (Cobros multimedio y pagos combinados)
    let _ = sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS venta_pagos (
            id SERIAL PRIMARY KEY,
            sale_id INTEGER NOT NULL REFERENCES sales(id) ON DELETE CASCADE,
            medio_pago VARCHAR(50) NOT NULL,
            monto_imputado NUMERIC(12, 2) NOT NULL,
            monto_recibido NUMERIC(12, 2) NOT NULL,
            vuelto_entregado NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
            referencia VARCHAR(100) NULL,
            "createdAt" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
    "#).execute(pool).await;

    // 4. Tabla caja_movimientos (Sangrías e ingresos directos de caja)
    let _ = sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS caja_movimientos (
            id SERIAL PRIMARY KEY,
            caja_sesion_id INTEGER NOT NULL REFERENCES cash_shifts(id) ON DELETE CASCADE,
            user_id INTEGER NOT NULL REFERENCES users(id),
            tipo VARCHAR(20) NOT NULL,
            monto NUMERIC(12, 2) NOT NULL,
            motivo VARCHAR(255) NOT NULL,
            "createdAt" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
    "#).execute(pool).await;

    // 5. Tabla movimientos_stock_kardex (Trazabilidad inmutable de inventario)
    let _ = sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS movimientos_stock_kardex (
            id SERIAL PRIMARY KEY,
            producto_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
            user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
            tipo_movimiento VARCHAR(50) NOT NULL,
            cantidad NUMERIC(12, 3) NOT NULL,
            stock_anterior NUMERIC(12, 3) NOT NULL,
            stock_resultante NUMERIC(12, 3) NOT NULL,
            costo_unitario NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
            referencia_id INTEGER NULL,
            motivo TEXT NULL,
            "createdAt" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
    "#).execute(pool).await;

    // 6. Tabla lotes_vencimientos (Vencimientos y mermas)
    let _ = sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS lotes_vencimientos (
            id SERIAL PRIMARY KEY,
            producto_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
            numero_lote VARCHAR(100) NULL,
            fecha_vencimiento DATE NOT NULL,
            cantidad NUMERIC(12, 3) NOT NULL DEFAULT 0,
            estado VARCHAR(50) NOT NULL DEFAULT 'ACTIVO',
            "createdAt" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
            "updatedAt" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
    "#).execute(pool).await;

    // 7. Tabla historico_precios_costo (Auditoría de precios y márgenes)
    let _ = sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS historico_precios_costo (
            id SERIAL PRIMARY KEY,
            producto_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
            precio_costo_anterior NUMERIC(12, 2) NOT NULL,
            precio_costo_nuevo NUMERIC(12, 2) NOT NULL,
            precio_venta_anterior NUMERIC(12, 2) NOT NULL,
            precio_venta_nuevo NUMERIC(12, 2) NOT NULL,
            motivo VARCHAR(255) NULL,
            user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
            "createdAt" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
    "#).execute(pool).await;

    // 8. Tabla outbox_events (Local-First y sincronización híbrida)
    let _ = sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS outbox_events (
            id SERIAL PRIMARY KEY,
            aggregate_type VARCHAR(100) NOT NULL,
            aggregate_id VARCHAR(100) NOT NULL,
            event_type VARCHAR(100) NOT NULL,
            payload JSONB NOT NULL,
            status VARCHAR(20) NOT NULL DEFAULT 'PENDING',
            "createdAt" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
            processed_at TIMESTAMP WITH TIME ZONE NULL
        )
    "#).execute(pool).await;
}
