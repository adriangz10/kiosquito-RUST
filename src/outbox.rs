use serde_json::Value;
use sqlx::PgPool;
use std::time::Duration;

pub async fn emit_event<'a, E>(
    executor: E,
    aggregate_type: &str,
    aggregate_id: &str,
    event_type: &str,
    payload: &Value,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'a, Database = sqlx::Postgres>,
{
    sqlx::query(
        r#"
        INSERT INTO outbox_events (aggregate_type, aggregate_id, event_type, payload, status, "createdAt")
        VALUES ($1, $2, $3, $4, 'PENDING', now())
        "#,
    )
    .bind(aggregate_type)
    .bind(aggregate_id)
    .bind(event_type)
    .bind(payload)
    .execute(executor)
    .await?;

    Ok(())
}

/// Worker asíncrono en segundo plano para el Outbox Pattern (Local-First Sync)
pub fn spawn_outbox_worker(pool: PgPool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            let result = sqlx::query(
                r#"
                UPDATE outbox_events
                SET status = 'PROCESSED', processed_at = now()
                WHERE id IN (
                    SELECT id FROM outbox_events
                    WHERE status = 'PENDING'
                    ORDER BY id ASC
                    LIMIT 25
                    FOR UPDATE SKIP LOCKED
                )
                "#,
            )
            .execute(&pool)
            .await;

            if let Err(e) = result {
                tracing::warn!("Error al procesar eventos de outbox: {:?}", e);
            }
        }
    });
}
