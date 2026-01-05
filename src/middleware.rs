//! Middleware for request context propagation and structured logging.
//!
//! This module provides:
//! - `RequestContext` - Request metadata propagation (correlation ID, user ID, language)
//! - `request_context_middleware_fn` - Axum middleware for context extraction
//! - `request_logging_middleware` - Tower-http TraceLayer for structured logging

use axum::{
    extract::Request,
    http::{HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use eywa_user_id::UserId;

/// Request context propagated through the entire request lifecycle.
///
/// This struct contains metadata that's extracted from incoming request headers
/// and made available to handlers via Axum's Extension extractor.
///
/// # Fields
///
/// - `correlation_id` - Unique identifier for tracking the request across services.
///   Extracted from `X-Correlation-ID` header or generated as a new UUID.
/// - `user_id` - Authenticated user ID, if present (extracted from JWT).
/// - `language` - Content language from `Accept-Language` header (defaults to "en").
/// - `request_id` - Unique identifier for this specific request (always generated).
///
/// # Example
///
/// ```ignore
/// use eywa_axum::prelude::*;
///
/// async fn my_handler(
///     Extension(ctx): Extension<RequestContext>,
/// ) -> Result<Json<Value>> {
///     info!("Handling request {}", ctx.correlation_id);
///     info!("User: {:?}", ctx.user_id);
///     info!("Language: {}", ctx.language);
///     Ok(json!({ "message": "Hello" }))
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RequestContext {
    /// Correlation ID from X-Correlation-ID header or generated
    pub correlation_id: Uuid,

    /// Authenticated user ID (if present)
    pub user_id: Option<UserId>,

    /// Content language from Accept-Language header (default: "en")
    pub language: String,

    /// Unique request ID (always generated)
    pub request_id: Uuid,
}

impl Default for RequestContext {
    fn default() -> Self {
        Self {
            correlation_id: Uuid::new_v4(),
            user_id: None,
            language: "en".to_string(),
            request_id: Uuid::new_v4(),
        }
    }
}

/// Extract correlation ID from headers or generate a new one.
///
/// # Priority
///
/// 1. `X-Correlation-ID` header value (if valid UUID)
/// 2. Generate new UUID
fn extract_correlation_id(headers: &HeaderMap) -> Uuid {
    headers
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or_else(Uuid::new_v4)
}

/// Extract language from Accept-Language header or default to "en".
///
/// # Priority
///
/// 1. `Accept-Language` header value (if present)
/// 2. Default to "en"
fn extract_language(headers: &HeaderMap) -> String {
    headers
        .get("accept-language")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("en")
        .to_string()
}

/// Axum middleware function for request context propagation.
///
/// This middleware:
/// 1. Extracts `X-Correlation-ID` header or generates a new UUID
/// 2. Extracts `Accept-Language` header or defaults to "en"
/// 3. Generates a unique `request_id`
/// 4. Inserts `RequestContext` as an Axum Extension
/// 5. Adds `X-Correlation-ID` to the response headers
///
/// # Example
///
/// ```ignore
/// use eywa_axum::prelude::*;
///
/// EywaApp::new(state)
///     .request_context()
///     .mount::<MyController>()
///     .serve("0.0.0.0:3000")
///     .await
/// ```
pub async fn request_context_middleware_fn(mut req: Request, next: Next) -> Response {
    let headers = req.headers().clone();

    // Extract or generate correlation ID
    let correlation_id = extract_correlation_id(&headers);

    // Extract language
    let language = extract_language(&headers);

    // Generate request ID
    let request_id = Uuid::new_v4();

    // Create request context (user_id will be set by auth middleware if present)
    let ctx = RequestContext {
        correlation_id,
        user_id: None, // Will be set by auth middleware
        language,
        request_id,
    };

    // Insert context into request extensions so logging middleware can access it
    req.extensions_mut().insert(ctx.clone());

    // Continue the request with context
    let mut response = next.run(req).await;

    // Add correlation ID to response headers
    if let Ok(header_value) = HeaderValue::from_str(&correlation_id.to_string()) {
        response
            .headers_mut()
            .insert("x-correlation-id", header_value);
    }

    response
}

/// Request logging middleware using tower-http's TraceLayer.
///
/// This middleware provides structured request logging compatible with
/// log aggregators like Loki, Grafana, Elasticsearch, etc.
///
/// # Logged Fields
///
/// - `method` - HTTP method (GET, POST, etc.)
/// - `uri` - Request path
/// - `correlation_id` - Correlation ID (if request context is enabled)
/// - `status` - HTTP status code
/// - `latency_ms` - Request duration in milliseconds
///
/// # Example
///
/// ```ignore
/// use eywa_axum::prelude::*;
///
/// EywaApp::new(state)
///     .request_logging()
///     .mount::<MyController>()
///     .serve("0.0.0.0:3000")
///     .await
/// ```
///
/// # Example Log Output
///
/// ```text
/// http_request{method="GET",uri="/api/projects",correlation_id="a1b2c3d4",status=200,latency_ms=45}: request completed
/// ```
pub fn request_logging_middleware() -> tower_http::trace::TraceLayer<
    tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>,
    tower_http::trace::DefaultMakeSpan,
> {
    tower_http::trace::TraceLayer::new_for_http().on_response(
        tower_http::trace::DefaultOnResponse::new()
            .level(tracing::Level::INFO)
            .latency_unit(tower_http::LatencyUnit::Millis),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_extract_correlation_id_from_header() {
        let mut headers = HeaderMap::new();
        let uuid = Uuid::new_v4();
        headers.insert(
            "x-correlation-id",
            HeaderValue::from_str(&uuid.to_string()).unwrap(),
        );

        let result = extract_correlation_id(&headers);
        assert_eq!(result, uuid);
    }

    #[test]
    fn test_extract_correlation_id_generate_new() {
        let headers = HeaderMap::new();

        let result = extract_correlation_id(&headers);
        // Should generate a valid UUID
        assert_eq!(result.get_version(), Some(uuid::Version::Random));
    }

    #[test]
    fn test_extract_language_from_header() {
        let mut headers = HeaderMap::new();
        headers.insert("accept-language", HeaderValue::from_static("it-IT"));

        let result = extract_language(&headers);
        assert_eq!(result, "it-IT");
    }

    #[test]
    fn test_extract_language_default() {
        let headers = HeaderMap::new();

        let result = extract_language(&headers);
        assert_eq!(result, "en");
    }

    #[test]
    fn test_request_context_default() {
        let ctx = RequestContext::default();
        assert_eq!(ctx.language, "en");
        assert!(ctx.user_id.is_none());
        assert_eq!(
            ctx.correlation_id.get_version().unwrap(),
            uuid::Version::Random
        );
    }
}
