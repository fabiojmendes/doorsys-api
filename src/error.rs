use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("MQTT error: {0}")]
    Mqtt(#[from] rumqttc::ClientError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] postcard::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type DomainResult<T> = Result<T, DomainError>;
