use crate::auth::Claims;
use crate::models::user::Role;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};

// Cualquier usuario logueado
pub struct AuthUser(pub Claims);

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Claims>()
            .cloned()
            .map(AuthUser)
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

// Solo usuarios con rol Administrador
pub struct AdminUser(pub Claims);

impl<S: Send + Sync> FromRequestParts<S> for AdminUser {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let AuthUser(claims) = AuthUser::from_request_parts(parts, state).await?;
        if claims.role != Role::Administrador {
            return Err(StatusCode::FORBIDDEN);
        }
        Ok(AdminUser(claims))
    }
}
