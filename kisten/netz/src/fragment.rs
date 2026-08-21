use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FragmentTokenStoreError {
    #[error("fragment token cannot be empty")]
    EmptyToken,
}

#[derive(Debug)]
pub struct FragmentTokenStore {
    ttl: Duration,
    tokens: Mutex<HashMap<[u8; 32], SystemTime>>,
}

impl FragmentTokenStore {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            tokens: Mutex::new(HashMap::new()),
        }
    }

    pub fn register(&self, token: &str) -> Result<(), FragmentTokenStoreError> {
        if token.is_empty() {
            return Err(FragmentTokenStoreError::EmptyToken);
        }
        self.tokens
            .lock()
            .expect("fragment token store lock")
            .insert(digest(token), SystemTime::now() + self.ttl);
        Ok(())
    }

    pub fn consume(&self, token: &str) -> bool {
        let hash = digest(token);
        let mut tokens = self.tokens.lock().expect("fragment token store lock");
        let Some(expires_at) = tokens.remove(&hash) else {
            return false;
        };
        expires_at > SystemTime::now()
    }
}

fn digest(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}
