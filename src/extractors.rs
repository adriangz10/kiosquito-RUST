use crate::auth::Claims;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::Json,
};
use serde_json::json;

// Cualquier usuario logueado
pub struct AuthUser(pub Claims);

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Claims>()
            .cloned()
            .map(AuthUser)
            .ok_or((
                StatusCode::UNAUTHORIZED,
                Json(json!({"message":"Token no proporcionado."})),
            ))
    }
}

// Usuarios con rol Administrador o Dueño
pub struct AdminUser(pub Claims);

impl<S: Send + Sync> FromRequestParts<S> for AdminUser {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let AuthUser(claims) = AuthUser::from_request_parts(parts, state).await?;
        if !claims.role.is_admin_or_owner() {
            return Err((
                StatusCode::FORBIDDEN,
                Json(json!({"message":"Acceso denegado. Se requiere rol de administrador o dueño."})),
            ));
        }
        Ok(AdminUser(claims))
    }
}

// Supervisores: Administrador, Dueño o Encargado
pub struct SupervisorUser(pub Claims);

impl<S: Send + Sync> FromRequestParts<S> for SupervisorUser {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let AuthUser(claims) = AuthUser::from_request_parts(parts, state).await?;
        if !claims.role.is_supervisor() {
            return Err((
                StatusCode::FORBIDDEN,
                Json(json!({"message":"Acceso denegado. Se requiere rol de supervisor (Administrador, Dueño o Encargado)."})),
            ));
        }
        Ok(SupervisorUser(claims))
    }
}
