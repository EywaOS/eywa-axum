# eywa-axum

The unified EYWA framework for Axum services with automatic OpenAPI support and production-ready features.

## Features

### Core Features
- **Automatic OpenAPI**: Routes registered via `routes!()` are automatically documented
- **Controller Pattern**: Optional `#[controller]` macro for grouping routes with automatic path prefixing
- **Scalar UI**: Interactive API documentation at `/scalar`
- **Swagger UI**: Alternative OpenAPI documentation at `/swagger` (with `swagger-ui` feature)
- **EYWA Ecosystem**: Integrated auth, errors, pagination, HATEOAS, and more

### Production-Ready Features

#### 1. Health Checks
Kubernetes-ready liveness and readiness probes.

```rust
EywaApp::new(state)
    .health_checks()  // Adds /health, /health/ready, /health/live
    .serve("0.0.0.0:3000")
    .await
```

**Endpoints:**
- `GET /health` - Basic health check (always returns 200 OK)
- `GET /health/ready` - Readiness probe (checks database connection)
- `GET /health/live` - Liveness probe (always returns 200 OK)

#### 2. Request Context Propagation
Propagate request metadata (correlation ID, user ID, language) through the entire request lifecycle.

```rust
use eywa_axum::middleware::RequestContext;

async fn my_handler(
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<Value>> {
    info!("Handling request {}", ctx.correlation_id);
    info!("User: {:?}", ctx.user_id);
    info!("Language: {}", ctx.language);
    Ok(json!({ "message": "Hello" }))
}

EywaApp::new(state)
    .request_context()  // Enable context propagation
    .mount::<MyController>()
    .serve("0.0.0.0:3000")
    .await
```

**RequestContext Fields:**
- `correlation_id` - From `X-Correlation-ID` header or generated
- `user_id` - From JWT (if authenticated)
- `language` - From `Accept-Language` header (default: "en")
- `request_id` - Always generated, unique per request

#### 3. Request Logging
Structured request logging compatible with Loki/Grafana and other log aggregators.

```rust
EywaApp::new(state)
    .request_context()      // First: extract headers, create context
    .request_logging()      // Second: log with correlation_id
    .serve("0.0.0.0:3000")
    .await
```

**What Gets Logged:**
- HTTP method
- Request path (URI)
- Correlation ID (if request context is enabled)
- Response status code
- Request duration (milliseconds)

**Example Log Output:**
```
http_request{method="GET",uri="/api/projects",correlation_id="a1b2c3d4",status=200,latency_ms=45}: request completed
```

#### 4. Response Compression
Reduce bandwidth usage by compressing HTTP responses.

```rust
EywaApp::new(state)
    .compression()  // Enable gzip, deflate, brotli
    .serve("0.0.0.0:3000")
    .await
```

**Supported Algorithms:**
- Gzip (most compatible)
- Deflate
- Brotli (best compression ratio)

**Impact:** Typically reduces response size by 70-90% for JSON/text content.

#### 5. API Versioning
Automatic version prefixing for API routes without manual repetition.

```rust
#[controller(
    version = "v1",           // NEW: version attribute
    prefix = "/projects",
    state = AppState
)]
impl ProjectsV1 {
    #[route(GET "/")]
    async fn list() -> Result<Json<Vec<ProjectV1>>> {
        // Route: /v1/projects
    }
}

#[controller(
    version = "v2",           // Different version
    prefix = "/projects",
    state = AppState
)]
impl ProjectsV2 {
    #[route(GET "/")]
    async fn list() -> Result<Json<Vec<ProjectV2>>> {
        // Route: /v2/projects
    }
}
```

**Benefits:**
- No need to repeat version in every route path
- Version is automatically included in OpenAPI documentation
- Clean separation between API versions

#### 6. Swagger UI
Alternative OpenAPI documentation UI alongside Scalar.

**Enable Feature:**
```toml
# Cargo.toml
eywa-axum = { version = "0.1", features = ["swagger-ui"] }
```

**Access:**
- Swagger UI: `http://localhost:3000/swagger`
- Scalar UI: `http://localhost:3000/scalar` (if scalar feature enabled)

## Complete Setup Example

```rust
use eywa_axum::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let config: MyAppConfig = EywaConfig::load()?;
    let db = Database::connect(&config.database_url).await?;
    let state = AppState { config, db };

    EywaApp::new(state)
        .health_checks()        // Kubernetes probes
        .request_context()      // Correlation ID propagation
        .request_logging()      // Structured logging
        .compression()          // Response compression
        .mount::<ProjectsController>()
        .mount::<TasksController>()
        .layer(auth_middleware())
        .serve("0.0.0.0:3000")
        .await
}
```

## Middleware Ordering

**Recommended order:**

```rust
EywaApp::new(state)
    .request_context()      // 1. First: extract headers, create context
    .request_logging()      // 2. Second: log with correlation_id
    .compression()          // 3. Third: compress before sending
    .layer(custom_middleware())  // 4. Custom middleware
```

**Why this order?**
1. `request_context()` must be first to make context available to logging
2. `request_logging()` needs context for correlation_id
3. `compression()` should be near the end to compress everything

## Feature Flags

```toml
[dependencies]
eywa-axum = { version = "0.1", features = ["scalar", "swagger-ui"] }
```

| Flag | Default | Description |
|------|---------|-------------|
| `scalar` | ✅ | Enable Scalar OpenAPI UI at `/scalar` |
| `swagger-ui` | ❌ | Enable Swagger UI at `/swagger` |

## Controller Macro

The `#[controller]` macro provides a clean way to group routes:

```rust
#[controller(
    version = "v1",           // API version prefix
    prefix = "/projects",     // URL prefix for all routes
    state = AppState,         // Application state type
    tag = "Projects",         // OpenAPI tag name
    middleware = auth_middleware,  // Optional middleware
    security,                 // Require bearer auth for all routes
    schemas(ProjectRequest, ProjectResponse)  // Register schemas
)]
impl ProjectsController {
    #[route(GET "/")]
    async fn list(State(state): State<AppState>) -> Result<Json<Vec<Project>>> {
        // Implementation
    }

    #[route(GET "/:id", summary = "Get project by ID")]
    async fn get(Path(id): Path<Uuid>) -> Result<Json<Project>> {
        // Implementation
    }
}
```

## Request Context in Handlers

```rust
async fn my_handler(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<AppState>,
) -> Result<Json<Value>> {
    info!(
        "Request {} from user {} (lang: {})",
        ctx.correlation_id,
        ctx.user_id.map(|u| u.to_string()).unwrap_or_else(|| "anonymous".to_string()),
        ctx.language
    );
    Ok(json!({ "message": "Hello" }))
}
```

## Testing

### Health Checks
```bash
curl http://localhost:3000/health        # Should return 200
curl http://localhost:3000/health/ready  # Should return 200 if DB is up
curl http://localhost:3000/health/live   # Should return 200
```

### Request Context
```bash
curl -H "X-Correlation-ID: test-123" \
     -H "Accept-Language: it-IT" \
     http://localhost:3000/api/projects
# Check logs for correlation_id and language
```

### Compression
```bash
curl -H "Accept-Encoding: gzip" \
     http://localhost:3000/api/projects \
     --compressed
```

### API Versioning
```bash
curl http://localhost:3000/v1/projects  # Version 1
curl http://localhost:3000/v2/projects  # Version 2
```

### OpenAPI Documentation
- Scalar: http://localhost:3000/scalar
- Swagger UI: http://localhost:3000/swagger

## Dependencies

| Package | Version | Features | Purpose |
|---------|---------|----------|---------|
| `tower-http` | 0.6 | compression-gzip, compression-deflate, compression-br | Response compression |
| `utoipa-swagger-ui` | 8 | (optional) | Swagger UI documentation |

## License

MIT
