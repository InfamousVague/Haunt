use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Application error types.
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("External API error: {0}")]
    ExternalApi(String),

    #[error("WebSocket error: {0}")]
    #[allow(dead_code)]
    WebSocket(String),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::ExternalApi(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            AppError::WebSocket(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::Reqwest(e) => (StatusCode::BAD_GATEWAY, e.to_string()),
            AppError::SerdeJson(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            AppError::Anyhow(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

        let body = Json(json!({
            "error": message,
            "status": status.as_u16(),
        }));

        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // AppError Display Tests
    // =========================================================================

    #[test]
    fn test_not_found_display() {
        let error = AppError::NotFound("User not found".to_string());
        assert_eq!(error.to_string(), "Not found: User not found");
    }

    #[test]
    fn test_bad_request_display() {
        let error = AppError::BadRequest("Invalid parameter".to_string());
        assert_eq!(error.to_string(), "Bad request: Invalid parameter");
    }

    #[test]
    fn test_internal_display() {
        let error = AppError::Internal("Database error".to_string());
        assert_eq!(error.to_string(), "Internal error: Database error");
    }

    #[test]
    fn test_external_api_display() {
        let error = AppError::ExternalApi("API rate limited".to_string());
        assert_eq!(error.to_string(), "External API error: API rate limited");
    }

    #[test]
    fn test_websocket_display() {
        let error = AppError::WebSocket("Connection closed".to_string());
        assert_eq!(error.to_string(), "WebSocket error: Connection closed");
    }

    // =========================================================================
    // Status Code Tests
    // =========================================================================

    #[test]
    fn test_not_found_status_code() {
        let error = AppError::NotFound("Resource not found".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_bad_request_status_code() {
        let error = AppError::BadRequest("Invalid input".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_internal_status_code() {
        let error = AppError::Internal("Server error".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_external_api_status_code() {
        let error = AppError::ExternalApi("Upstream error".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn test_websocket_status_code() {
        let error = AppError::WebSocket("WS error".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    // =========================================================================
    // From Conversion Tests
    // =========================================================================

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<i32>("invalid").unwrap_err();
        let app_err: AppError = json_err.into();

        match app_err {
            AppError::SerdeJson(_) => {}
            _ => panic!("Expected SerdeJson variant"),
        }
    }

    #[test]
    fn test_serde_json_error_status_code() {
        let json_err = serde_json::from_str::<i32>("invalid").unwrap_err();
        let app_err: AppError = json_err.into();
        let response = app_err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_from_anyhow_error() {
        let anyhow_err = anyhow::anyhow!("Something went wrong");
        let app_err: AppError = anyhow_err.into();

        match app_err {
            AppError::Anyhow(_) => {}
            _ => panic!("Expected Anyhow variant"),
        }
    }

    #[test]
    fn test_anyhow_error_status_code() {
        let anyhow_err = anyhow::anyhow!("Internal issue");
        let app_err: AppError = anyhow_err.into();
        let response = app_err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    // =========================================================================
    // Result Type Tests
    // =========================================================================

    #[test]
    fn test_result_ok() {
        let result: Result<i32> = Ok(42);
        assert!(result.is_ok());
        match result {
            Ok(v) => assert_eq!(v, 42),
            Err(_) => panic!("expected Ok"),
        }
    }

    #[test]
    fn test_result_err() {
        let result: Result<i32> = Err(AppError::NotFound("test".to_string()));
        assert!(result.is_err());
    }

    // =========================================================================
    // Error Debug Tests
    // =========================================================================

    #[test]
    fn test_error_debug_format() {
        let error = AppError::NotFound("test".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("NotFound"));
        assert!(debug_str.contains("test"));
    }
}
