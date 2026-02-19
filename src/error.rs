use std::fmt;

#[derive(Debug)]
pub enum DbError {
    InvalidIdentifier(String),
    InvalidInput(String),
    ConnectionError(sqlx::Error),
    SqlError(sqlx::Error),
    NotFound(String),
    NoDatabaseSelected,
    ReadOnlyViolation(String),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::InvalidIdentifier(name) => write!(f, "Invalid identifier: {name}"),
            DbError::InvalidInput(msg) => write!(f, "Invalid input: {msg}"),
            DbError::ConnectionError(e) => write!(f, "Connection error: {e}"),
            DbError::SqlError(e) => write!(f, "SQL error: {e}"),
            DbError::NotFound(msg) => write!(f, "Not found: {msg}"),
            DbError::NoDatabaseSelected => write!(f, "No database selected"),
            DbError::ReadOnlyViolation(msg) => write!(f, "Read-only violation: {msg}"),
        }
    }
}

impl std::error::Error for DbError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DbError::ConnectionError(e) | DbError::SqlError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for DbError {
    fn from(e: sqlx::Error) -> Self {
        DbError::SqlError(e)
    }
}
