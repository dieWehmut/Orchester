use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    Json,
};
use cookie::{Cookie, SameSite};
use getrandom::fill as fill_random;
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{health::no_store_headers, ServerContext};

pub const SESSION_COOKIE_NAME: &str = "orchester_session";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionBootstrapDto {
    pub schema_version: u8,
    pub csrf_token: String,
    pub expires_at: u64,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SessionBootstrap {
    pub session_cookie: String,
    pub csrf_token: String,
    pub expires_at: u64,
}

impl std::fmt::Debug for SessionBootstrap {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SessionBootstrap")
            .field("session_cookie", &"[REDACTED]")
            .field("csrf_token", &"[REDACTED]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SessionStoreError {
    #[error("session is unknown or expired")]
    UnknownSession,
    #[error("session token generation failed")]
    Entropy,
}

#[derive(Debug)]
pub struct SessionStore {
    ttl: Duration,
    sessions: Mutex<HashMap<[u8; 32], StoredSession>>,
}

#[derive(Debug, Clone, Copy)]
struct StoredSession {
    csrf_hash: [u8; 32],
    expires_at: SystemTime,
}

impl SessionStore {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn issue(&self) -> Result<SessionBootstrap, SessionStoreError> {
        let session_cookie = random_token()?;
        let csrf_token = random_token()?;
        let expires_at = SystemTime::now() + self.ttl;
        let session_hash = digest(&session_cookie);
        let csrf_hash = digest(&csrf_token);
        self.sessions.lock().expect("session store lock").insert(
            session_hash,
            StoredSession {
                csrf_hash,
                expires_at,
            },
        );

        Ok(SessionBootstrap {
            session_cookie,
            csrf_token,
            expires_at: unix_seconds(expires_at),
        })
    }

    pub fn validate(&self, session_cookie: &str, csrf_token: &str) -> bool {
        self.validate_result(session_cookie, csrf_token).is_ok()
    }

    pub fn validate_result(
        &self,
        session_cookie: &str,
        csrf_token: &str,
    ) -> Result<(), SessionStoreError> {
        let session_hash = digest(session_cookie);
        let csrf_hash = digest(csrf_token);
        let mut sessions = self.sessions.lock().expect("session store lock");
        let Some(session) = sessions.get(&session_hash).copied() else {
            return Err(SessionStoreError::UnknownSession);
        };
        if SystemTime::now() >= session.expires_at || session.csrf_hash != csrf_hash {
            if SystemTime::now() >= session.expires_at {
                sessions.remove(&session_hash);
            }
            return Err(SessionStoreError::UnknownSession);
        }
        Ok(())
    }

    pub fn revoke(&self, session_cookie: &str) -> bool {
        self.sessions
            .lock()
            .expect("session store lock")
            .remove(&digest(session_cookie))
            .is_some()
    }
}

pub async fn session_bootstrap_handler(
    State(context): State<ServerContext>,
) -> Result<(HeaderMap, Json<SessionBootstrapDto>), StatusCode> {
    let issued = context
        .sessions()
        .issue()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let cookie = Cookie::build((SESSION_COOKIE_NAME, issued.session_cookie))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .build();
    let mut headers = no_store_headers();
    headers.insert(
        header::SET_COOKIE,
        cookie
            .to_string()
            .parse()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );
    Ok((
        headers,
        Json(SessionBootstrapDto {
            schema_version: 1,
            csrf_token: issued.csrf_token,
            expires_at: issued.expires_at,
        }),
    ))
}

pub async fn session_revoke_handler(
    State(context): State<ServerContext>,
    headers: HeaderMap,
) -> StatusCode {
    let Some(cookie_header) = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
    else {
        return StatusCode::FORBIDDEN;
    };
    let Some(session_cookie) = Cookie::split_parse(cookie_header)
        .filter_map(Result::ok)
        .find(|cookie| cookie.name() == SESSION_COOKIE_NAME)
    else {
        return StatusCode::FORBIDDEN;
    };
    let Some(csrf_token) = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
    else {
        return StatusCode::FORBIDDEN;
    };
    if !context
        .sessions()
        .validate(session_cookie.value(), csrf_token)
    {
        return StatusCode::FORBIDDEN;
    }
    if context.sessions().revoke(session_cookie.value()) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::FORBIDDEN
    }
}

fn random_token() -> Result<String, SessionStoreError> {
    let mut bytes = [0_u8; 32];
    fill_random(&mut bytes).map_err(|_| SessionStoreError::Entropy)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn digest(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

fn unix_seconds(value: SystemTime) -> u64 {
    value
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
