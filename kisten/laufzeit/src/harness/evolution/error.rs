use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum EvolutionError {
    #[error("evolution input is invalid")]
    InvalidInput,
    #[error("evolution digest is invalid")]
    InvalidDigest,
    #[error("evolution timestamp is invalid")]
    InvalidTimestamp,
    #[error("evolution expiry must be later than creation")]
    InvalidExpiry,
    #[error("evolution evaluation is missing a reproducibility snapshot")]
    MissingSnapshot,
    #[error("evolution schema is unsupported")]
    UnsupportedSchema,
    #[error("evolution identity is corrupt")]
    Corrupt,
}
