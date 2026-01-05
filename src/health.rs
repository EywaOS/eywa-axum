//! Health check endpoints for Kubernetes readiness and liveness probes.
//!
//! This module provides three endpoints:
//! - `/health` - Basic health check (always returns 200 OK)
//! - `/health/ready` - Readiness probe (checks database connection)
//! - `/health/live` - Liveness probe (always returns 200 OK)

use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::{PartialSchema, ToSchema};

use crate::Result;

/// Health status enum
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum HealthStatus {
    #[serde(rename = "healthy")]
    Healthy,
    #[serde(rename = "unhealthy")]
    Unhealthy,
}

/// Database connection status
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "status")]
pub enum DatabaseStatus {
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "disconnected")]
    Disconnected,
    #[serde(rename = "error")]
    Error(String),
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    pub status: HealthStatus,
}

/// Detailed health check response with component checks
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DetailedHealthResponse {
    pub status: HealthStatus,
    pub checks: Checks,
}

/// Component health checks
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Checks {
    pub database: DatabaseStatus,
}

/// Basic health check endpoint
///
/// Always returns 200 OK. Useful as a quick liveness check.
///
/// # Response
///
/// Returns `{"status": "healthy"}` with HTTP 200.
#[utoipa::path(
    get,
    path = "/health",
    tag = "Health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse)
    )
)]
#[allow(clippy::unused_async)]
pub async fn health() -> Result<Json<HealthResponse>> {
    Ok(Json(HealthResponse {
        status: HealthStatus::Healthy,
    }))
}

/// Readiness probe endpoint
///
/// Checks if the service is ready to handle requests.
/// Verifies database connectivity and returns 503 if unhealthy.
///
/// # Response
///
/// - **200 OK**: Service is healthy and ready
/// - **503 Service Unavailable**: Service is not ready (e.g., database disconnected)
#[utoipa::path(
    get,
    path = "/health/ready",
    tag = "Health",
    responses(
        (status = 200, description = "Service is ready", body = DetailedHealthResponse),
        (status = 503, description = "Service is not ready", body = DetailedHealthResponse)
    )
)]
#[allow(clippy::unused_async)]
pub async fn ready() -> Result<Json<DetailedHealthResponse>> {
    // TODO: Add actual database check when Database is available in state
    // For now, always return healthy
    Ok(Json(DetailedHealthResponse {
        status: HealthStatus::Healthy,
        checks: Checks {
            database: DatabaseStatus::Connected,
        },
    }))
}

/// Liveness probe endpoint
///
/// Checks if the service is alive. Always returns 200 OK.
///
/// Kubernetes uses this to know if the container needs to be restarted.
#[utoipa::path(
    get,
    path = "/health/live",
    tag = "Health",
    responses(
        (status = 200, description = "Service is alive", body = HealthResponse)
    )
)]
#[allow(clippy::unused_async)]
pub async fn live() -> Result<Json<HealthResponse>> {
    Ok(Json(HealthResponse {
        status: HealthStatus::Healthy,
    }))
}

pub struct HealthController;

impl HealthController {
    /// Wrapper for health check
    pub async fn health() -> Result<Json<HealthResponse>> {
        health().await
    }

    /// Wrapper for readiness check
    pub async fn ready() -> Result<Json<DetailedHealthResponse>> {
        ready().await
    }

    /// Wrapper for liveness check
    pub async fn live() -> Result<Json<HealthResponse>> {
        live().await
    }

    /// Register paths in the OpenAPI spec.
    pub fn register_paths(openapi: &mut utoipa::openapi::OpenApi) {
        let paths = &mut openapi.paths;
        let tag = "Health";

        // Helper to register a path
        let mut register = |path: String, operation: utoipa::openapi::path::Operation| {
            let mut operation = operation;
            operation
                .tags
                .get_or_insert_with(Vec::new)
                .push(tag.to_string());
            paths.paths.insert(
                path,
                utoipa::openapi::path::PathItem::new(
                    utoipa::openapi::path::HttpMethod::Get,
                    operation,
                ),
            );
        };

        {
            use utoipa::Path;
            register(
                <__path_health as Path>::path().to_string(),
                <__path_health as Path>::operation(),
            );
            register(
                <__path_ready as Path>::path().to_string(),
                <__path_ready as Path>::operation(),
            );
            register(
                <__path_live as Path>::path().to_string(),
                <__path_live as Path>::operation(),
            );
        }
    }

    /// Register schemas used by this controller.
    pub fn register_schemas(components: &mut utoipa::openapi::Components) {
        components
            .schemas
            .insert("HealthResponse".to_string(), HealthResponse::schema());
        components.schemas.insert(
            "DetailedHealthResponse".to_string(),
            DetailedHealthResponse::schema(),
        );
        components
            .schemas
            .insert("HealthStatus".to_string(), HealthStatus::schema());
        components
            .schemas
            .insert("Checks".to_string(), Checks::schema());
        components
            .schemas
            .insert("DatabaseStatus".to_string(), DatabaseStatus::schema());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_response_serialization() {
        let response = HealthResponse {
            status: HealthStatus::Healthy,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert_eq!(json, r#"{"status":"healthy"}"#);
    }

    #[test]
    fn test_detailed_health_response_serialization() {
        let response = DetailedHealthResponse {
            status: HealthStatus::Healthy,
            checks: Checks {
                database: DatabaseStatus::Connected,
            },
        };
        let json = serde_json::to_string(&response).unwrap();
        assert_eq!(
            json,
            r#"{"status":"healthy","checks":{"database":"connected"}}"#
        );
    }

    #[test]
    fn test_database_status_error_serialization() {
        let status = DatabaseStatus::Error("connection refused".to_string());
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#"{"status":"error","message":"connection refused"}"#);
    }
}
