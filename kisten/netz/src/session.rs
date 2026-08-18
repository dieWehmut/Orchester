use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use getrandom::fill as fill_random;
use sha2::{Digest, Sha256};
use thiserror::Error;

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
