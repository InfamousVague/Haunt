use crate::AppState;
use axum::{routing::get, Json, Router};
use serde::Serialize;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/health", get(health))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_response_serialization() {
        let response = HealthResponse {
            status: "ok",
            version: "1.0.0",
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"status\":\"ok\""));
        assert!(json.contains("\"version\":\"1.0.0\""));
    }

    #[test]
    fn test_health_response_fields() {
        let response = HealthResponse {
            status: "ok",
            version: env!("CARGO_PKG_VERSION"),
        };

        assert_eq!(response.status, "ok");
        assert!(!response.version.is_empty());
    }

    #[tokio::test]
    async fn test_health_handler() {
        let Json(response) = health().await;
        assert_eq!(response.status, "ok");
        assert_eq!(response.version, env!("CARGO_PKG_VERSION"));
    }
}
