use thiserror::Error;
use serde::{Serialize, Deserialize};

#[derive(Error, Debug, Serialize, Deserialize)]
pub enum AppError {
    #[error("Ping error: {0}")]
    #[serde(rename = "ping_error")]
    PingError(String),

    #[error("DNS resolution error: {0}")]
    #[serde(rename = "dns_error")]
    DnsError(String),

    #[error("Traceroute error: {0}")]
    #[serde(rename = "traceroute_error")]
    TracerouteError(String),

    #[error("Bandwidth test error: {0}")]
    #[serde(rename = "bandwidth_error")]
    BandwidthError(String),

    #[error("Database error: {0}")]
    #[serde(rename = "database_error")]
    DatabaseError(String),

    #[error("IO error: {0}")]
    #[serde(rename = "io_error")]
    IoError(String),

    #[error("Invalid target: {0}")]
    #[serde(rename = "invalid_target")]
    InvalidTarget(String),

    #[error("Operation cancelled")]
    #[serde(rename = "cancelled")]
    Cancelled,

    #[error("Internal error: {0}")]
    #[serde(rename = "internal_error")]
    Internal(String),
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::DatabaseError(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::IoError(err.to_string())
    }
}

impl From<AppError> for String {
    fn from(err: AppError) -> Self {
        err.to_string()
    }
}

pub type AppResult<T> = Result<T, AppError>;