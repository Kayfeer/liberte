use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Blob not found: {0}")]
    BlobNotFound(Uuid),

    #[error("Blob too large: {size} bytes (max {max})")]
    BlobTooLarge { size: usize, max: usize },

    #[error("Blob storage error: {0}")]
    BlobStorage(String),

    #[error("Premium verification failed")]
    PremiumVerificationFailed,

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ServerError::BlobNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            ServerError::BlobTooLarge { .. } => {
                (StatusCode::PAYLOAD_TOO_LARGE, self.to_string())
            }
            ServerError::BlobStorage(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Blob storage error".to_string())
            }
            ServerError::PremiumVerificationFailed => {
                (StatusCode::UNAUTHORIZED, self.to_string())
            }
            ServerError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ServerError::Forbidden(_) => (StatusCode::FORBIDDEN, self.to_string()),
            ServerError::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
        };

        let body = serde_json::json!({
            "error": message,
        });

        (status, axum::Json(body)).into_response()
    }
}
