//! # eywa-axum
//!
//! The unified EYWA framework for Axum services with automatic OpenAPI support.
//!
//! ## Features
//!
//! - **Automatic OpenAPI**: Routes registered via `routes!()` are automatically documented
//! - **Scalar UI**: Interactive API documentation at `/scalar`
//! - **Swagger UI**: Alternative OpenAPI documentation at `/swagger` (with `swagger-ui` feature)
//! - **Health Checks**: Kubernetes-ready liveness and readiness probes
//! - **Request Context**: Correlation ID, user ID, and language propagation
//! - **Request Logging**: Structured logging compatible with Loki/Grafana
//! - **Response Compression**: Gzip, deflate, and brotli compression
//! - **API Versioning**: Automatic version prefix support (e.g., `/v1/projects`)
//! - **Controller Pattern**: Optional `#[controller]` macro for grouping routes
//! - **EYWA Ecosystem**: Integrated auth, errors, pagination, and more
//!
//! ## Quick Start
//!
//! ```ignore
//! use eywa_axum::prelude::*;
//!
//! #[utoipa::path(get, path = "/health", responses((status = 200, body = String)))]
//! async fn health() -> &'static str {
//!     "OK"
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let state = AppState::new();
//!
//!     EywaApp::new(state)
//!         .info("My API", "1.0.0", "API description")
//!         .routes(routes!(health))
//!         .serve("0.0.0.0:8080")
//!         .await
//! }
//! ```

// Re-export specific modules
mod app;
pub mod config;
mod health;
pub mod middleware;
mod traits;

pub use app::EywaApp;
pub use app::legacy::LegacyEywaApp;
pub use traits::*;

// Re-export health check types
pub use health::{HealthController, HealthStatus};

// Re-export middleware types
pub use middleware::{RequestContext, request_context_middleware_fn};

// Re-export Swagger UI when feature is enabled
#[cfg(feature = "swagger-ui")]
pub use utoipa_swagger_ui::{Config, SwaggerUi};

// Re-export macros from eywa-axum-macros
pub use eywa_axum_macros::{controller, openapi_for, route};

// Re-export utoipa-axum for route registration
pub use utoipa_axum::{router::OpenApiRouter, routes};

// Re-export common dependencies
pub use axum::{
    self, Router,
    extract::{Extension, Json, Path, Query, Request, State},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post, put},
};
pub use serde::{Deserialize, Serialize};
pub use serde_json::{self, json};
pub use tokio;
pub use tracing::{debug, error, info, instrument, warn};

// Re-export database & config
pub use config as config_rs;
pub use eywa_database::{Database, DatabaseConfig, transaction};
pub use sea_orm;

// Re-export EYWA ecosystem
pub use eywa_authentication::{self, JwtService, middleware::auth_middleware};
pub use eywa_errors::{self, AppError, Result};
pub use eywa_hateoas::{self, CollectionResponse, HateoasResponse, Link};
pub use eywa_pagination::{self, PaginationParams};
pub use eywa_types::{self, ApiCollectionResult, ApiResult};
pub use eywa_user_id::{self, UserId};
pub use eywa_utoipa::{self, IntoRouter as IntoRouterUtoipa, OpenApiBuilder, OpenApiRegistrar};

// Re-export OpenAPI (via eywa-utoipa)
pub use eywa_utoipa::{IntoParams, OpenApi, ToSchema, utoipa};
pub use utoipa_scalar::{Scalar, Servable};

/// Prelude for easy importing
pub mod prelude {
    pub use super::{
        ApiCollectionResult,
        ApiResult,
        AppError,
        CollectionResponse,
        Deserialize,
        Extension,
        EywaApp,
        HateoasResponse,
        HealthController,
        HealthStatus,
        IntoParams,
        IntoResponse,
        Json,
        LegacyEywaApp,
        Link,
        // OpenAPI related
        OpenApi,
        OpenApiRouter,
        PaginationParams,
        Path,
        Query,
        Request,
        RequestContext,
        Response,
        Result,
        Router,
        Serialize,
        State,
        ToSchema,
        UserId,
        controller,
        debug,
        delete,
        error,
        get,
        info,
        json,
        patch,
        post,
        put,
        route,
        routes,
        warn,
    };
    pub use crate::config::EywaConfig;
    pub use crate::traits::{IntoRouter, OpenApiPath};
    pub use eywa_database::{Database, DatabaseConfig};
    pub use sea_orm::{self, ActiveModelTrait, ActiveValue, EntityTrait, ModelTrait, QueryFilter};
    pub use uuid::Uuid;
    // Re-export utoipa path macro for route documentation
    pub use eywa_utoipa::utoipa::path;
}
