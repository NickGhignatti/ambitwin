use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::domain::DomainError;

const INTERNAL_MESSAGE: &str = "An unexpected error occurred. Please try again later.";

impl DomainError {
    /// One place decides how a failure is named, so an error that reaches the client
    /// mid-stream reads exactly like the same error delivered as a status code.
    pub fn describe(&self) -> (StatusCode, &'static str, String) {
        match self {
            DomainError::Validation(m) => (StatusCode::BAD_REQUEST, "Validation Error", m.clone()),
            DomainError::Unauthorized(m) => {
                (StatusCode::UNAUTHORIZED, "Unauthorized Error", m.clone())
            }
            DomainError::NotFound(m) => (StatusCode::NOT_FOUND, "Not Found Error", m.clone()),
            DomainError::Conflict(m) => (StatusCode::CONFLICT, "Conflict Error", m.clone()),
            DomainError::BadGateway(m) => (StatusCode::BAD_GATEWAY, "Bad Gateway Error", m.clone()),
            DomainError::Internal(e) => {
                log::error!("{e:?}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Server Error",
                    INTERNAL_MESSAGE.to_string(),
                )
            }
        }
    }
}

impl IntoResponse for DomainError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = self.describe();
        (
            status,
            Json(serde_json::json!({ "type": error_type, "message": message })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn rendered(error: DomainError) -> (StatusCode, serde_json::Value) {
        let response = error.into_response();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn each_error_renders_its_status_type_and_message() {
        let cases = [
            (
                DomainError::Validation("content must be a non-empty string".into()),
                StatusCode::BAD_REQUEST,
                "Validation Error",
                "content must be a non-empty string",
            ),
            (
                DomainError::Unauthorized("Missing authentication token".into()),
                StatusCode::UNAUTHORIZED,
                "Unauthorized Error",
                "Missing authentication token",
            ),
            (
                DomainError::not_found(),
                StatusCode::NOT_FOUND,
                "Not Found Error",
                "Conversation not found",
            ),
            (
                DomainError::Conflict("Conversation message limit reached".into()),
                StatusCode::CONFLICT,
                "Conflict Error",
                "Conversation message limit reached",
            ),
            (
                DomainError::BadGateway("Could not reach agent".into()),
                StatusCode::BAD_GATEWAY,
                "Bad Gateway Error",
                "Could not reach agent",
            ),
            // the cause stays in the log: the client sees only the generic message
            (
                DomainError::Internal(anyhow::anyhow!("mongo credentials rejected")),
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                INTERNAL_MESSAGE,
            ),
        ];

        for (error, status, error_type, message) in cases {
            let (rendered_status, body) = rendered(error).await;
            assert_eq!(rendered_status, status, "{error_type}");
            assert_eq!(body["type"], error_type);
            assert_eq!(body["message"], message, "{error_type}");
        }
    }
}
