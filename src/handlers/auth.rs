use crate::auth::{create_jwt, verify_password};
use crate::errors::AppError;
use crate::models::user::{LoginRequest, LoginResponse, User, UserPublic};
use axum::{extract::State, Json};
use sqlx::PgPool;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

struct LoginFailures {
    attempts: u8,
    first_failure: Instant,
}

static LOGIN_FAILURES: OnceLock<Mutex<HashMap<String, LoginFailures>>> = OnceLock::new();

fn login_is_blocked(username: &str) -> Option<u64> {
    let failures = LOGIN_FAILURES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut failures = failures
        .lock()
        .expect("lock de intentos de login envenenado");
    if let Some(entry) = failures.get(username) {
        if entry.first_failure.elapsed() < Duration::from_secs(300) && entry.attempts >= 5 {
            let remaining_secs = 300 - entry.first_failure.elapsed().as_secs();
            let minutes = remaining_secs.div_ceil(60);
            return Some(minutes);
        }
        if entry.first_failure.elapsed() >= Duration::from_secs(300) {
            failures.remove(username);
        }
    }
    None
}

fn record_login_failure(username: &str) {
    let failures = LOGIN_FAILURES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut failures = failures
        .lock()
        .expect("lock de intentos de login envenenado");
    let entry = failures
        .entry(username.to_string())
        .or_insert(LoginFailures {
            attempts: 0,
            first_failure: Instant::now(),
        });
    if entry.first_failure.elapsed() >= Duration::from_secs(300) {
        entry.attempts = 0;
        entry.first_failure = Instant::now();
    }
    entry.attempts = entry.attempts.saturating_add(1);
}

fn clear_login_failures(username: &str) {
    if let Some(failures) = LOGIN_FAILURES.get() {
        failures
            .lock()
            .expect("lock de intentos de login envenenado")
            .remove(username);
    }
}

pub async fn login(
    State(pool): State<PgPool>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    if let Some(minutes) = login_is_blocked(&payload.username) {
        return Err(AppError::TooManyRequests(format!(
            "Demasiados intentos fallidos. Esperá {} minuto(s).",
            minutes
        )));
    }

    let user = sqlx::query_as::<_, User>(
        r#"SELECT id, username, role, password, "createdAt", "updatedAt"
           FROM users WHERE username = $1"#,
    )
    .bind(&payload.username)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| {
        record_login_failure(&payload.username);
        AppError::BadRequest("Nombre de usuario o contraseña incorrectos.".into())
    })?;

    if !verify_password(&payload.password, &user.password) {
        record_login_failure(&payload.username);
        return Err(AppError::BadRequest(
            "Nombre de usuario o contraseña incorrectos.".into(),
        ));
    }
    clear_login_failures(&payload.username);

    let role = user.role.clone();
    let token =
        create_jwt(user.id, &user.username, role.clone()).map_err(|_| AppError::Internal)?;

    Ok(Json(LoginResponse {
        message: "Inicio de sesión exitoso.".to_string(),
        token,
        user: UserPublic {
            id: user.id,
            username: user.username,
            role,
            has_supervisor_pin: user.pin_supervisor.is_some(),
        },
    }))
}
