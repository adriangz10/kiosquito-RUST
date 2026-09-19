use crate::models::user::Role;
use argon2::{password_hash::phc::PasswordHash, Argon2, PasswordVerifier};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    #[serde(alias = "id")]
    pub sub: i32,
    pub username: String,
    pub role: Role,
    pub exp: usize,
}

#[derive(Serialize)]
struct TokenClaims<'a> {
    sub: i32,
    username: &'a str,
    role: Role,
    exp: usize,
}

pub fn hash_password(password: &str) -> Result<String, String> {
    bcrypt::hash(password, 10).map_err(|e| e.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    if hash.starts_with("$2a$") || hash.starts_with("$2b$") || hash.starts_with("$2y$") {
        bcrypt::verify(password, hash).unwrap_or(false)
    } else if hash.starts_with("$argon2") {
        match PasswordHash::new(hash) {
            Ok(parsed) => Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok(),
            Err(_) => false,
        }
    } else {
        bcrypt::verify(password, hash).unwrap_or_else(|_| {
            PasswordHash::new(hash)
                .map(|parsed| {
                    Argon2::default()
                        .verify_password(password.as_bytes(), &parsed)
                        .is_ok()
                })
                .unwrap_or(false)
        })
    }
}

pub fn create_jwt(
    user_id: i32,
    username: &str,
    role: Role,
) -> Result<String, jsonwebtoken::errors::Error> {
    let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET no seteada");
    let exp = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(1))
        .unwrap()
        .timestamp() as usize;

    let claims = TokenClaims {
        sub: user_id,
        username,
        role,
        exp,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

pub fn decode_jwt(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET no seteada");
    Ok(decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?
    .claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bcrypt_hashing_and_verification() {
        let password = "super_secure_password_123";
        let hash = hash_password(password).expect("hash failed");
        assert!(hash.starts_with("$2b$") || hash.starts_with("$2a$"));
        assert!(verify_password(password, &hash));
        assert!(!verify_password("wrong_password", &hash));
    }

    #[test]
    fn test_node_bcrypt_hash_compatibility() {
        let password = "admin123";
        let hash = bcrypt::hash(password, 10).expect("bcrypt hash failed");
        assert!(hash.starts_with("$2b$") || hash.starts_with("$2a$"));
        assert!(verify_password(password, &hash));
        assert!(!verify_password("incorrecto", &hash));
    }

    #[test]
    fn test_jwt_id_alias_decoding() {
        std::env::set_var("JWT_SECRET", "una-clave-secreta-de-prueba-12345");
        // Simular token que viene de Node.js con campo `id` en vez de `sub`
        #[derive(Serialize)]
        struct NodeClaims<'a> {
            id: i32,
            username: &'a str,
            role: Role,
            exp: usize,
        }
        let exp = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::hours(1))
            .unwrap()
            .timestamp() as usize;
        let node_token = encode(
            &Header::default(),
            &NodeClaims {
                id: 42,
                username: "juan",
                role: Role::Administrador,
                exp,
            },
            &EncodingKey::from_secret("una-clave-secreta-de-prueba-12345".as_bytes()),
        )
        .expect("encode failed");

        let decoded = decode_jwt(&node_token).expect("jwt decode failed");
        assert_eq!(decoded.sub, 42);
        assert_eq!(decoded.username, "juan");
        assert_eq!(decoded.role, Role::Administrador);
    }
}
