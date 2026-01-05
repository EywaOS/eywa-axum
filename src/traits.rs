//! Common traits for the eywa-axum-controller framework.

use axum::Router;

/// OpenAPI path information
#[derive(Clone, Debug)]
pub struct OpenApiPath {
    pub path: String,
    pub method: String,
    pub summary: String,
    pub description: String,
    pub tag: String,
}

/// Trait for controllers that can be converted into an axum Router.
///
/// This trait is automatically implemented by the `#[controller]` macro.
pub trait IntoRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    /// Creates an axum Router from this controller.
    fn into_router(state: S) -> Router<S>;

    /// Returns the URL prefix for this controller.
    fn prefix() -> &'static str {
        ""
    }

    /// Returns the OpenAPI tag for this controller.
    fn tag() -> &'static str {
        "API"
    }

    /// Returns route metadata for OpenAPI generation.
    fn openapi_routes() -> Vec<OpenApiPath> {
        Vec::new()
    }

    /// Register schemas used by this controller.
    /// Called by EywaApp::mount() to collect schemas.
    fn register_schemas(components: &mut utoipa::openapi::Components) {
        // Default: no schemas
        let _ = components;
    }

    /// Register paths in the OpenAPI spec.
    /// Called by EywaApp::mount() to add paths.
    fn register_paths(openapi: &mut utoipa::openapi::OpenApi) {
        // Default: no paths (will be overridden by macro)
        let _ = openapi;
    }
}

/// Marker trait for route handlers.
pub trait RouteHandler<S>: Send + Sync + 'static
where
    S: Clone + Send + Sync + 'static,
{
}
