use crate::error::AppError;
use aide::{
    OperationOutput,
    generate::GenContext,
    openapi::{Operation, Response},
};
use axum::{Json, http::StatusCode, response::IntoResponse};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug)]
pub struct HttpError(AppError);
pub type HttpResult<T> = Result<T, HttpError>;

#[derive(Serialize, JsonSchema)]
pub struct ErrorBody {
    code: u16,
    success: bool,
    msg: String,
}
impl From<AppError> for HttpError {
    fn from(error: AppError) -> Self {
        Self(error)
    }
}
impl IntoResponse for HttpError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self.0 {
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Conflict(_) | AppError::DuplicateEntryLog => StatusCode::CONFLICT,
            AppError::InvalidInput(_) => StatusCode::BAD_REQUEST,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::SyncFailed(_) | AppError::PublishFailed(_) => StatusCode::SERVICE_UNAVAILABLE,
        };
        if status.is_server_error() {
            tracing::error!(error = ?self.0, "Request failed");
        }
        let body = ErrorBody {
            code: status.as_u16(),
            success: false,
            msg: self.0.to_string(),
        };
        (status, Json(body)).into_response()
    }
}
impl OperationOutput for HttpError {
    type Inner = ErrorBody;
    fn inferred_responses(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Vec<(Option<u16>, Response)> {
        let Some(response) = Json::<ErrorBody>::operation_response(ctx, operation) else {
            return Vec::new();
        };
        [400, 404, 409, 500, 503]
            .into_iter()
            .map(|code| {
                let mut response = response.clone();
                response.description = StatusCode::from_u16(code)
                    .unwrap()
                    .canonical_reason()
                    .unwrap()
                    .into();
                (Some(code), response)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn maps_errors_without_exposing_internal_details() {
        for (error, status) in [
            (AppError::NotFound, 404),
            (AppError::Conflict("conflict"), 409),
            (AppError::DuplicateEntryLog, 409),
            (AppError::InvalidInput("invalid"), 400),
            (
                AppError::Internal(anyhow::anyhow!("secret database details")),
                500,
            ),
            (
                AppError::SyncFailed(anyhow::anyhow!("secret broker details")),
                503,
            ),
        ] {
            let response = HttpError::from(error).into_response();
            assert_eq!(response.status().as_u16(), status);
            assert_eq!(response.headers()["content-type"], "application/json");
            let bytes = axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap();
            let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(body["code"], status);
            assert_eq!(body["success"], false);
            assert!(!body["msg"].as_str().unwrap().contains("secret"));
        }
    }
}
