use crate::auth::{decode_jwt, Claims};
use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::{header, Method},
    middleware::Next,
    response::Response,
};
use serde_json::{json, Value};
use sqlx::PgPool;

pub async fn audit(State(pool): State<PgPool>, req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let user_id = req
        .extensions()
        .get::<Claims>()
        .map(|claims| claims.sub)
        .or_else(|| {
            req.headers()
                .get(header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("Bearer "))
                .and_then(|token| decode_jwt(token).ok())
                .map(|claims| claims.sub)
        });
    let (parts, body) = req.into_parts();
    let body_bytes = to_bytes(body, 1024 * 1024).await.unwrap_or_default();
    let payload = serde_json::from_slice::<Value>(&body_bytes).unwrap_or_else(|_| json!({}));
    let request = Request::from_parts(parts, Body::from(body_bytes));
    let response = next.run(request).await;

    if response.status().is_success()
        && matches!(
            method,
            Method::POST | Method::PUT | Method::PATCH | Method::DELETE
        )
    {
        let entity_id = path
            .rsplit('/')
            .next()
            .filter(|value| value.parse::<i64>().is_ok());
        let mut redacted = payload;
        if let Some(object) = redacted.as_object_mut() {
            object.remove("password");
            object.remove("token");
        }
        if let Err(error) = sqlx::query(
            "INSERT INTO audit_logs (user_id,method,path,entity_id,payload,\"createdAt\") VALUES ($1,$2,$3,$4,$5,now())",
        )
        .bind(user_id)
        .bind(method.as_str())
        .bind(&path)
        .bind(entity_id)
        .bind(redacted)
        .execute(&pool)
        .await
        {
            tracing::warn!(?error, %path, "No se pudo registrar auditoría");
        }
    }

    response
}
