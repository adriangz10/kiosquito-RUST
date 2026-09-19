use crate::auth::decode_jwt;
use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

pub async fn auth_guard(mut req: Request, next: Next) -> Result<Response, Response> {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"message":"Token no proporcionado."})),
            )
                .into_response()
        })?;

    let claims = decode_jwt(token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"message":"Token inválido o expirado."})),
        )
            .into_response()
    })?;

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}
