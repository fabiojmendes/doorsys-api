use poem_openapi::{payload::Json, ApiResponse, Object};
use poem::http::StatusCode;
use poem::error::ResponseError;

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
    pub fn from_err(err: impl std::fmt::Display, status: StatusCode) -> Self {
        let error_response = ErrorResponse {
            success: false,
            message: err.to_string(),
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

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        tracing::error!("Internal Server Error: {:?}", err);
        ApiError::from_err(err, StatusCode::INTERNAL_SERVER_ERROR)
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => ApiError::from_err("Resource not found", StatusCode::NOT_FOUND),
            _ => {
                tracing::error!("Database Error: {:?}", err);
                ApiError::from_err(err, StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}

impl From<postcard::Error> for ApiError {
    fn from(err: postcard::Error) -> Self {
        tracing::error!("Serialization Error: {:?}", err);
        ApiError::from_err(err, StatusCode::INTERNAL_SERVER_ERROR)
    }
}

impl From<rumqttc::ClientError> for ApiError {
    fn from(err: rumqttc::ClientError) -> Self {
        tracing::error!("MQTT Error: {:?}", err);
        ApiError::from_err(err, StatusCode::INTERNAL_SERVER_ERROR)
    }
}
