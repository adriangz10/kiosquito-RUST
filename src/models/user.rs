use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, sqlx::Type, Serialize, Deserialize, Clone, PartialEq)]
#[sqlx(type_name = "enum_users_role")]
pub enum Role {
    Administrador,
    Empleado,
    #[sqlx(rename = "Dueño")]
    #[serde(rename = "Dueño", alias = "Dueno")]
    Dueno,
    Encargado,
    Cajero,
    Repositor,
}

#[allow(dead_code)]
impl Role {
    pub fn is_admin_or_owner(&self) -> bool {
        matches!(self, Role::Administrador | Role::Dueno)
    }

    pub fn is_supervisor(&self) -> bool {
        matches!(self, Role::Administrador | Role::Dueno | Role::Encargado)
    }

    pub fn can_operate_pos(&self) -> bool {
        matches!(
            self,
            Role::Administrador
                | Role::Dueno
                | Role::Encargado
                | Role::Cajero
                | Role::Empleado
        )
    }

    pub fn can_manage_inventory(&self) -> bool {
        matches!(
            self,
            Role::Administrador
                | Role::Dueno
                | Role::Encargado
                | Role::Repositor
        )
    }
}

#[derive(Debug, FromRow)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub role: Role,
    pub password: String, // hash, nunca se serializa al cliente
    pub pin_supervisor: Option<String>,
    #[allow(dead_code)]
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[allow(dead_code)]
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserPublic {
    pub id: i32,
    pub username: String,
    pub role: Role,
    pub has_supervisor_pin: bool,
}

#[derive(Debug, Deserialize)]
pub struct RegisterUser {
    pub username: String,
    pub password: String,
    pub role: Role,
    pub pin_supervisor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUser {
    pub username: Option<String>,
    pub password: Option<String>,
    pub role: Option<Role>,
    pub pin_supervisor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub message: String,
    pub token: String,
    pub user: UserPublic,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rbac_roles_hierarchy() {
        assert!(Role::Administrador.is_admin_or_owner());
        assert!(Role::Dueno.is_admin_or_owner());
        assert!(!Role::Encargado.is_admin_or_owner());
        assert!(!Role::Cajero.is_admin_or_owner());
        assert!(!Role::Repositor.is_admin_or_owner());

        assert!(Role::Administrador.is_supervisor());
        assert!(Role::Dueno.is_supervisor());
        assert!(Role::Encargado.is_supervisor());
        assert!(!Role::Cajero.is_supervisor());
        assert!(!Role::Repositor.is_supervisor());

        assert!(Role::Cajero.can_operate_pos());
        assert!(Role::Encargado.can_operate_pos());
        assert!(Role::Dueno.can_operate_pos());
        assert!(!Role::Repositor.can_operate_pos());

        assert!(Role::Repositor.can_manage_inventory());
        assert!(Role::Encargado.can_manage_inventory());
    }
}
