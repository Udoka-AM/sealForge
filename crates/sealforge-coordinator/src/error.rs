use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use sealforge_api::types::ApiErrorBody;

/// Every handler error maps to a status code and a stable `code` string.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("not allowed")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("{message}")]
    Conflict { code: &'static str, message: String },
    #[error("{0}")]
    Unavailable(String),
    #[error("internal error")]
    Internal(#[from] anyhow::Error),
}

impl ApiError {
    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::Conflict {
            code,
            message: message.into(),
        }
    }

    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "invalid_request"),
            Self::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "validation_failed"),
            Self::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            Self::Conflict { code, .. } => (StatusCode::CONFLICT, code),
            Self::Unavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        Self::Internal(err.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();
        if let Self::Internal(err) = &self {
            // Never include request data here: it may contain addresses or commitments.
            tracing::error!(error = %err, "internal error");
        }
        let body = ApiErrorBody {
            code: code.to_owned(),
            error: self.to_string(),
        };
        (status, Json(body)).into_response()
    }
}
