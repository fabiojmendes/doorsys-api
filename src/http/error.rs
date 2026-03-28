use crate::error::DomainError;
use poem::error::ResponseError;
use poem::http::StatusCode;
use poem_openapi::{payload::Json, ApiResponse, Object};

#[derive(Debug, Object)]
pub struct ErrorResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, ApiResponse)]
pub enum ApiError {
    #[oai(status = 400)]
    BadRequest(Json<ErrorResponse>),
    #[oai(status = 404)]
    NotFound(Json<ErrorResponse>),
    #[oai(status = 500)]
    InternalServerError(Json<ErrorResponse>),
}

impl ApiError {
    pub fn from_err<E: std::fmt::Display + std::fmt::Debug>(err: E, status: StatusCode) -> Self {
        let msg = err.to_string();

        if status.is_server_error() {
            tracing::error!("Internal Server Error: {:?}", err);
        } else if status.is_client_error() {
            tracing::warn!("Client Error ({}): {:?}", status, err);
        }

        let error_response = ErrorResponse {
            success: false,
            message: msg,
        };
        match status {
            StatusCode::NOT_FOUND => ApiError::NotFound(Json(error_response)),
            StatusCode::BAD_REQUEST => ApiError::BadRequest(Json(error_response)),
            _ => ApiError::InternalServerError(Json(error_response)),
        }
    }
}

impl ResponseError for ApiError {
    fn status(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::InternalServerError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<DomainError> for ApiError {
    fn from(err: DomainError) -> Self {
        match err {
            DomainError::NotFound(msg) => ApiError::from_err(msg, StatusCode::NOT_FOUND),
            DomainError::Validation(msg) => ApiError::from_err(msg, StatusCode::BAD_REQUEST),
            _ => ApiError::from_err(err, StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::from_err(err, StatusCode::INTERNAL_SERVER_ERROR)
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => {
                ApiError::from_err("Resource not found", StatusCode::NOT_FOUND)
            }
            _ => ApiError::from_err(err, StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}

impl From<postcard::Error> for ApiError {
    fn from(err: postcard::Error) -> Self {
        ApiError::from_err(err, StatusCode::INTERNAL_SERVER_ERROR)
    }
}

impl From<rumqttc::ClientError> for ApiError {
    fn from(err: rumqttc::ClientError) -> Self {
        ApiError::from_err(err, StatusCode::INTERNAL_SERVER_ERROR)
    }
}
