//! EywaApp builder for easy application setup with automatic OpenAPI.
//!
//! This module provides the main application builder that automatically
//! collects OpenAPI paths from controllers.

use axum::{routing::get, Router};
use tokio::net::TcpListener;
use tracing::info;
use utoipa::ToSchema;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::openapi::{Components, Info, OpenApi, Tag};
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};

use crate::traits::IntoRouter;

/// Builder for creating EYWA applications with automatic OpenAPI support.
///
/// Controllers mounted via `mount::<C>()` automatically have their paths
/// registered in the OpenAPI specification.
///
/// # Example
/// ```ignore
/// use eywa_axum::prelude::*;
///
/// EywaApp::new(state)
///     .info("My API", "1.0.0", "API description")
///     .mount::<UserController>()
///     .mount::<TimerController>()
///     .serve("0.0.0.0:8080")
///     .await?;
/// ```
pub struct EywaApp<S>
where
    S: Clone + Send + Sync + 'static,
{
    state: S,
    router: OpenApiRouter<S>,
    info: Option<Info>,
    tags: Vec<Tag>,
    schema_fns: Vec<Box<dyn Fn(&mut utoipa::openapi::Components) + Send + Sync>>,
    path_fns: Vec<Box<dyn Fn(&mut utoipa::openapi::OpenApi) + Send + Sync>>,
    has_health_checks: bool,
}

impl<S> EywaApp<S>
where
    S: Clone + Send + Sync + 'static,
{
    /// Create a new EywaApp with the given state.
    pub fn new(state: S) -> Self {
        Self {
            state,
            router: OpenApiRouter::new(),
            info: None,
            tags: Vec::new(),
            schema_fns: Vec::new(),
            path_fns: Vec::new(),
            has_health_checks: false,
        }
    }

    /// Set API info (title, version, description).
    ///
    /// # Example
    /// ```ignore
    /// app.info("My API", "1.0.0", "API description")
    /// ```
    pub fn info(
        mut self,
        title: impl Into<String>,
        version: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        self.info = Some(
            utoipa::openapi::InfoBuilder::new()
                .title(title.into())
                .version(version.into())
                .description(Some(description.into()))
                .build(),
        );
        self
    }

    /// Add a tag with description.
    ///
    /// # Example
    /// ```ignore
    /// app.tag("Timer", "Timer management endpoints")
    /// ```
    pub fn tag(mut self, name: impl Into<String>, description: impl Into<String>) -> Self {
        self.tags.push(
            utoipa::openapi::tag::TagBuilder::new()
                .name(name.into())
                .description(Some(description.into()))
                .build(),
        );
        self
    }

    /// Register schemas for OpenAPI components.
    ///
    /// # Example
    /// ```ignore
    /// app.schema::<MyRequest>()
    ///    .schema::<MyResponse>()
    /// ```
    pub fn schema<T: ToSchema + 'static>(mut self) -> Self {
        self.schema_fns
            .push(Box::new(|components: &mut utoipa::openapi::Components| {
                let name = T::name().to_string();
                let schema = T::schema();
                components.schemas.insert(name, schema);
            }));
        self
    }

    /// Mount a controller to the application.
    ///
    /// This automatically:
    /// 1. Registers all routes from the controller
    /// 2. Collects OpenAPI paths from `__UTOIPA_PATHS__`
    /// 3. Adds the controller's tag
    ///
    /// # Example
    /// ```ignore
    /// app.mount::<TimerController>()
    ///    .mount::<UserController>()
    /// ```
    pub fn mount<C>(mut self) -> Self
    where
        C: IntoRouter<S>,
    {
        let prefix = C::prefix();
        let controller_tag = C::tag();

        // Get the controller's router
        let controller_router = C::into_router(self.state.clone());

        // Convert to OpenApiRouter
        let controller_openapi_router: OpenApiRouter<S> = OpenApiRouter::from(controller_router);

        // Get OpenAPI route metadata
        let openapi_routes = C::openapi_routes();

        // Log routes
        for route in &openapi_routes {
            info!("📍 {} {} [{}]", route.method, route.path, route.tag);
        }

        // Nest or merge the controller router
        if prefix.is_empty() {
            self.router = self.router.merge(controller_openapi_router);
        } else {
            self.router = self.router.nest(prefix, controller_openapi_router);
        }

        // Add controller tag if not already present
        if !self.tags.iter().any(|t| t.name == controller_tag) {
            self.tags.push(
                utoipa::openapi::tag::TagBuilder::new()
                    .name(controller_tag)
                    .build(),
            );
        }

        // Collect controller's schemas
        self.schema_fns.push(Box::new(|components| {
            C::register_schemas(components);
        }));

        // Collect controller's paths
        self.path_fns.push(Box::new(|openapi| {
            C::register_paths(openapi);
        }));

        self
    }

    /// Add routes using utoipa-axum's `routes!` macro.
    ///
    /// For when you want to add routes outside of controllers.
    pub fn routes(mut self, routes: utoipa_axum::router::UtoipaMethodRouter<S>) -> Self {
        self.router = self.router.routes(routes);
        self
    }

    /// Merge another OpenApiRouter into this one.
    pub fn merge(mut self, other: OpenApiRouter<S>) -> Self {
        self.router = self.router.merge(other);
        self
    }

    /// Apply a middleware layer.
    pub fn layer<L>(mut self, layer: L) -> Self
    where
        L: tower::Layer<axum::routing::Route> + Clone + Send + Sync + 'static,
        L::Service: tower::Service<axum::extract::Request> + Clone + Send + Sync + 'static,
        <L::Service as tower::Service<axum::extract::Request>>::Future: Send + 'static,
        <L::Service as tower::Service<axum::extract::Request>>::Response:
            axum::response::IntoResponse + 'static,
        <L::Service as tower::Service<axum::extract::Request>>::Error:
            Into<std::convert::Infallible> + 'static,
    {
        self.router = self.router.layer(layer);
        self
    }

    /// Add health check endpoints for Kubernetes probes.
    ///
    /// Adds three endpoints:
    /// - `/health` - Basic health check (always returns 200 OK)
    /// - `/health/ready` - Readiness probe (checks database connection)
    /// - `/health/live` - Liveness probe (always returns 200 OK)
    ///
    /// # Example
    /// ```ignore
    /// EywaApp::new(state)
    ///     .health_checks()
    ///     .serve("0.0.0.0:3000")
    ///     .await
    /// ```
    pub fn health_checks(mut self) -> Self {
        use crate::health::HealthController;

        self.router = self.router
            .route("/health", get(HealthController::health))
            .route("/health/ready", get(HealthController::ready))
            .route("/health/live", get(HealthController::live));

        self.path_fns.push(Box::new(|openapi| {
            HealthController::register_paths(openapi);
        }));

        self.schema_fns.push(Box::new(|components| {
            HealthController::register_schemas(components);
        }));

        self.has_health_checks = true;
        self
    }

    /// Enable response compression using gzip, deflate, and brotli.
    ///
    /// Automatically compresses responses based on Accept-Encoding header.
    /// Typically reduces response size by 70-90% for JSON/text content.
    ///
    /// # Example
    /// ```ignore
    /// EywaApp::new(state)
    ///     .compression()
    ///     .serve("0.0.0.0:3000")
    ///     .await
    /// ```
    pub fn compression(mut self) -> Self {
        use tower_http::compression::CompressionLayer;

        self.router = self.router.layer(CompressionLayer::new());
        self
    }

    /// Enable structured request logging compatible with Loki/Grafana.
    ///
    /// Logs HTTP method, path, correlation ID, status code, and latency.
    /// Should be called after `.request_context()` to include correlation IDs.
    ///
    /// # Example
    /// ```ignore
    /// EywaApp::new(state)
    ///     .request_context()
    ///     .request_logging()
    ///     .serve("0.0.0.0:3000")
    ///     .await
    /// ```
    pub fn request_logging(mut self) -> Self {
        use crate::middleware::request_logging_middleware;

        self.router = self.router.layer(request_logging_middleware());
        self
    }

    /// Enable request context propagation (correlation ID, user ID, language).
    ///
    /// Extracts request metadata from headers and makes it available to handlers
    /// via `Extension<RequestContext>`. Should be called before `.request_logging()`.
    ///
    /// # Example
    /// ```ignore
    /// use eywa_axum::middleware::RequestContext;
    ///
    /// async fn handler(
    ///     Extension(ctx): Extension<RequestContext>,
    /// ) -> Result<Json<Value>> {
    ///     info!("Handling request {}", ctx.correlation_id);
    ///     // ...
    /// }
    ///
    /// EywaApp::new(state)
    ///     .request_context()
    ///     .mount::<MyController>()
    ///     .serve("0.0.0.0:3000")
    ///     .await
    /// ```
    pub fn request_context(mut self) -> Self {
        use crate::middleware::request_context_middleware_fn;

        use tower_http::normalize_path::NormalizePathLayer;
        use tower::ServiceBuilder;

        self.router = self.router.layer(
            ServiceBuilder::new()
                .layer(NormalizePathLayer::trim_trailing_slash())
                .layer(axum::middleware::from_fn(request_context_middleware_fn))
        );
        self
    }

    /// Serve the application with automatic Scalar UI.
    ///
    /// This method:
    /// 1. Builds the final OpenAPI spec
    /// 2. Adds a `/scalar` endpoint for interactive API documentation
    /// 3. Adds a `/swagger` endpoint if swagger-ui feature is enabled
    /// 4. Starts the HTTP server
    pub async fn serve(self, addr: &str) -> crate::Result<()> {
        // Split router to get OpenAPI
        let (router, mut openapi) = self.router.split_for_parts();

        // Apply custom info if provided
        if let Some(info) = self.info {
            openapi.info = info;
        }

        // Add tags
        if !self.tags.is_empty() {
            openapi.tags = Some(self.tags);
        }

        // Add schemas and security scheme to components
        let mut components = openapi.components.unwrap_or_else(Components::new);

        // Add bearer security scheme
        components.add_security_scheme(
            "bearer",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .description(Some("JWT Bearer token"))
                    .build(),
            ),
        );

        // Add custom schemas
        for schema_fn in self.schema_fns {
            schema_fn(&mut components);
        }

        openapi.components = Some(components);

        // Add collected paths
        for path_fn in self.path_fns {
            path_fn(&mut openapi);
        }

        // Log API info
        info!("📚 API: {} v{}", openapi.info.title, openapi.info.version);
        if let Some(ref desc) = openapi.info.description {
            info!("   {}", desc);
        }

        // Log discovered paths
        for (path, item) in &openapi.paths.paths {
            let methods: Vec<_> = [
                item.get.as_ref().map(|_| "GET"),
                item.post.as_ref().map(|_| "POST"),
                item.put.as_ref().map(|_| "PUT"),
                item.delete.as_ref().map(|_| "DELETE"),
                item.patch.as_ref().map(|_| "PATCH"),
            ]
            .into_iter()
            .flatten()
            .collect();
            info!("   {} [{}]", path, methods.join(", "));
        }

        // Create final router with Scalar UI
        let router = router
            .merge(Scalar::with_url("/scalar", openapi.clone()));

        // Add Swagger UI if feature is enabled
        #[cfg(feature = "swagger-ui")]
        let router = {
            use utoipa_swagger_ui::SwaggerUi;
            router.merge(SwaggerUi::new("/swagger")
                .url("/api-docs/openapi.json", openapi.clone()))
        };

        let router = router.with_state(self.state);

        // Bind and serve
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| eywa_errors::AppError::InternalServerError(e.to_string()))?;

        info!("🚀 Server listening on http://{}", addr);

        // Display available endpoints
        info!("📚 Available endpoints:");
        info!("   - Scalar: http://{}/scalar", addr);
        #[cfg(feature = "swagger-ui")]
        info!("   - Swagger UI: http://{}/swagger", addr);
        if self.has_health_checks {
            info!("   - Health Checks: http://{}/health", addr);
        }

        axum::serve(listener, router.into_make_service())
            .await
            .map_err(|e: std::io::Error| eywa_errors::AppError::InternalServerError(e.to_string()))
    }
}

/// Legacy EywaApp for backward compatibility (uses manual OpenAPI).
pub mod legacy {
    use super::*;

    /// Legacy builder that supports manual OpenAPI configuration.
    pub struct LegacyEywaApp<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        state: S,
        router: Router<S>,
        openapi: OpenApi,
    }

    impl<S> LegacyEywaApp<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        /// Create a new LegacyEywaApp with the given state.
        pub fn new(state: S) -> Self {
            Self {
                state,
                router: Router::new(),
                openapi: OpenApi::default(),
            }
        }

        /// Set a custom OpenAPI specification.
        pub fn with_openapi(mut self, openapi: OpenApi) -> Self {
            self.openapi = openapi;
            self
        }

        /// Mount a controller (legacy pattern).
        pub fn mount<C>(mut self) -> Self
        where
            C: IntoRouter<S>,
        {
            let prefix = C::prefix();
            let controller_router = C::into_router(self.state.clone());

            if prefix.is_empty() {
                self.router = self.router.merge(controller_router);
            } else {
                self.router = self.router.nest(prefix, controller_router);
            }

            let routes = C::openapi_routes();
            for route in routes {
                info!("📍 {} {} [{}]", route.method, route.path, route.tag);
            }

            self
        }

        /// Apply a middleware layer.
        pub fn layer<L>(mut self, layer: L) -> Self
        where
            L: tower::Layer<axum::routing::Route> + Clone + Send + Sync + 'static,
            L::Service: tower::Service<axum::extract::Request> + Clone + Send + Sync + 'static,
            <L::Service as tower::Service<axum::extract::Request>>::Future: Send + 'static,
            <L::Service as tower::Service<axum::extract::Request>>::Response:
                axum::response::IntoResponse + 'static,
            <L::Service as tower::Service<axum::extract::Request>>::Error:
                Into<std::convert::Infallible> + 'static,
        {
            self.router = self.router.layer(layer);
            self
        }

        /// Serve the application.
        pub async fn serve(self, addr: &str) -> crate::Result<()> {
            let listener = TcpListener::bind(addr)
                .await
                .map_err(|e| eywa_errors::AppError::InternalServerError(e.to_string()))?;

            info!("🚀 Server listening on http://{}", addr);
            info!("📖 OpenAPI docs available at http://{}/scalar", addr);

            let router = self
                .router
                .merge(Scalar::with_url("/scalar", self.openapi))
                .with_state(self.state);

            axum::serve(listener, router.into_make_service())
                .await
                .map_err(|e: std::io::Error| {
                    eywa_errors::AppError::InternalServerError(e.to_string())
                })
        }
    }
}
