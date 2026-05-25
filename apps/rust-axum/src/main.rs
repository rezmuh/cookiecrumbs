use axum::{
    body::Body,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use deadpool_postgres::{Config, ManagerConfig, Pool, RecyclingMethod, Runtime};
use redis::{aio::ConnectionManager, AsyncCommands, Client as RedisClient};
use serde::{Deserialize, Serialize};
use std::env;
use std::net::SocketAddr;
use tokio_postgres::NoTls;
use tracing::{error, info, warn};
use tracing_subscriber::prelude::*;

const SERVICE_NAME: &str = "rust-axum";

#[derive(Clone)]
struct AppState {
    db_pool: Pool,
    redis: ConnectionManager,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    service: String,
    version: String,
    dependencies: Dependencies,
}

#[derive(Serialize)]
struct Dependencies {
    postgres: String,
    redis: String,
}

#[derive(Deserialize)]
struct LogRequest {
    level: Option<String>,
    message: String,
    context: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct LogResponse {
    status: String,
}

#[derive(Deserialize)]
struct ErrorRequest {
    message: Option<String>,
}

#[derive(Serialize)]
struct ErrorResponse {
    status: String,
    message: String,
}

#[derive(Deserialize)]
struct TraceFullRequest {
    message: Option<String>,
}

#[derive(Serialize)]
struct TraceFullResponse {
    status: String,
    operations: Vec<String>,
    item_id: i32,
}

#[derive(Deserialize)]
struct MetricRequest {
    name: String,
    value: f64,
    tags: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct MetricResponse {
    status: String,
}

#[derive(Deserialize)]
struct CreateItemRequest {
    message: String,
}

#[derive(Serialize)]
struct ItemResponse {
    id: i32,
    service_name: String,
    message: String,
    created_at: Option<String>,
}

#[derive(Serialize)]
struct ItemsResponse {
    items: Vec<ItemResponse>,
}

#[derive(Serialize)]
struct TraceDbResponse {
    status: String,
    count: i64,
}

#[derive(Serialize)]
struct TraceRedisResponse {
    status: String,
    value: String,
}

fn main() {
    dotenvy::dotenv().ok();

    let _guard = sentry::init((
        env::var("SENTRY_DSN").unwrap_or_default(),
        sentry::ClientOptions {
            release: Some(env::var("SENTRY_RELEASE").unwrap_or_else(|_| "1.0.0".into()).into()),
            environment: Some(env::var("SENTRY_ENVIRONMENT").unwrap_or_else(|_| "development".into()).into()),
            traces_sample_rate: 1.0,
            enable_logs: true,
            ..Default::default()
        },
    ));
    
    // Set up tracing with Sentry integration
    // By default: INFO+ captured as logs, ERROR captured as events
    let sentry_layer = sentry::integrations::tracing::layer()
        .event_filter(|md| match *md.level() {
            tracing::Level::ERROR => sentry::integrations::tracing::EventFilter::Event | sentry::integrations::tracing::EventFilter::Log,
            _ => sentry::integrations::tracing::EventFilter::Log,
        });
    
    // Set up fmt layer to print to stdout
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("info".parse().unwrap()));
    
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(sentry_layer)
        .init();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let db_pool = create_db_pool().await.expect("Failed to create DB pool");
            let redis = create_redis_connection().await.expect("Failed to connect to Redis");

            let state = AppState { db_pool, redis };

            let app = Router::new()
                .route("/health", get(health))
                .route("/demo/log", post(demo_log))
                .route("/demo/error/handled", post(demo_error_handled))
                .route("/demo/error/unhandled", post(demo_error_unhandled))
                .route("/demo/trace/db", get(demo_trace_db))
                .route("/demo/trace/redis", get(demo_trace_redis))
                .route("/demo/trace/full", post(demo_trace_full))
                .route("/demo/metric", post(demo_metric))
                .route("/demo/db/items", get(get_items).post(create_item))
                .layer(sentry_tower::NewSentryLayer::new_from_top())
                .layer(sentry_tower::SentryHttpLayer::with_transaction())
                .with_state(state);

            let addr = SocketAddr::from(([0, 0, 0, 0], 8003));
            info!("Server starting on {}", addr);

            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
            axum::serve(listener, app).await.unwrap();
        });
}

async fn create_db_pool() -> Result<Pool, Box<dyn std::error::Error>> {
    let mut cfg = Config::new();
    cfg.host = Some(env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".into()));
    cfg.port = Some(env::var("POSTGRES_PORT")?.parse()?);
    cfg.dbname = Some(env::var("POSTGRES_DB").unwrap_or_else(|_| "cookiecrumbs".into()));
    cfg.user = Some(env::var("POSTGRES_USER").unwrap_or_else(|_| "cookiecrumbs".into()));
    cfg.password = Some(env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "cookiecrumbs".into()));
    cfg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });

    Ok(cfg.create_pool(Some(Runtime::Tokio1), NoTls)?)
}

async fn create_redis_connection() -> Result<ConnectionManager, Box<dyn std::error::Error>> {
    let redis_url = format!(
        "redis://{}:{}",
        env::var("REDIS_HOST").unwrap_or_else(|_| "localhost".into()),
        env::var("REDIS_PORT").unwrap_or_else(|_| "6379".into())
    );
    let client = RedisClient::open(redis_url)?;
    Ok(ConnectionManager::new(client).await?)
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    info!(metric_name="api.request", endpoint="health", service=SERVICE_NAME, method="GET", "api.request");
    let mut postgres_status = "connected";
    let mut redis_status = "connected";

    if let Err(_) = state.db_pool.get().await {
        postgres_status = "disconnected";
    }

    if let Err(_) = redis::cmd("PING").query_async::<_, ()>(&mut state.redis.clone()).await {
        redis_status = "disconnected";
    }

    info!("[HEALTH] {} - postgres={}, redis={}", SERVICE_NAME, postgres_status, redis_status);
    info!(metric_name="health.check", service=SERVICE_NAME, "health.check");

    Json(HealthResponse {
        status: "healthy".into(),
        service: SERVICE_NAME.into(),
        version: "1.0.0".into(),
        dependencies: Dependencies {
            postgres: postgres_status.into(),
            redis: redis_status.into(),
        },
    })
}

async fn demo_log(Json(body): Json<LogRequest>) -> impl IntoResponse {
    info!(metric_name="api.request", endpoint="demo/log", service=SERVICE_NAME, method="POST", "api.request");
    let level = body.level.unwrap_or_else(|| "info".into());
    let message = body.message;

    match level.as_str() {
        "debug" => info!("[DEBUG] {}", message),
        "warning" => warn!("{}", message),
        "error" => error!("{}", message),
        _ => info!("{}", message),
    }

    Json(LogResponse {
        status: "logged".into(),
    })
}

async fn demo_error_handled(Json(body): Json<ErrorRequest>) -> impl IntoResponse {
    info!(metric_name="api.request", endpoint="demo/error/handled", service=SERVICE_NAME, method="POST", "api.request");
    let message = body.message.unwrap_or_else(|| "Handled error".into());

    let err = std::io::Error::new(std::io::ErrorKind::Other, message.clone());
    sentry::capture_error(&err);

    Json(ErrorResponse {
        status: "error_handled".into(),
        message,
    })
}

async fn demo_error_unhandled() -> Result<Json<serde_json::Value>, AppError> {
    info!(metric_name="api.request", endpoint="demo/error/unhandled", service=SERVICE_NAME, method="POST", "api.request");
    error!("Unhandled error triggered");
    sentry::capture_message("Unhandled error triggered", sentry::Level::Error);
    panic!("Unhandled error triggered");
}

async fn demo_trace_db(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    info!(metric_name="api.request", endpoint="demo/trace/db", service=SERVICE_NAME, method="GET", "api.request");
    let client = state.db_pool.get().await?;
    let row = client.query_one("SELECT COUNT(*) FROM demo_items", &[]).await?;
    let count: i64 = row.get(0);

    Ok(Json(TraceDbResponse {
        status: "db_trace_complete".into(),
        count,
    }))
}

async fn demo_trace_redis(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    info!(metric_name="api.request", endpoint="demo/trace/redis", service=SERVICE_NAME, method="GET", "api.request");
    let key = format!("demo:{}:counter", SERVICE_NAME);
    let mut redis = state.redis.clone();
    
    redis.incr::<_, _, i64>(&key, 1).await?;
    let value: String = redis.get(&key).await?;

    Ok(Json(TraceRedisResponse {
        status: "redis_trace_complete".into(),
        value,
    }))
}

async fn demo_trace_full(
    State(state): State<AppState>,
    Json(body): Json<TraceFullRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!(metric_name="api.request", endpoint="demo/trace/full", service=SERVICE_NAME, method="POST", "api.request");
    let message = body.message.unwrap_or_else(|| "Full trace test".into());

    info!("Starting full trace: {}", message);

    // DB operation
    let client = state.db_pool.get().await?;
    let row = client
        .query_one(
            "INSERT INTO demo_items (service_name, message) VALUES ($1, $2) RETURNING id",
            &[&SERVICE_NAME, &message],
        )
        .await?;
    let item_id: i32 = row.get(0);

    // Redis operations
    let mut redis = state.redis.clone();
    redis.set::<_, _, ()>(format!("demo:{}:last-log", SERVICE_NAME), &message).await?;
    redis.incr::<_, _, ()>(format!("demo:{}:counter", SERVICE_NAME), 1).await?;
    redis.set::<_, _, ()>("demo:shared:heartbeat", SERVICE_NAME).await?;

    Ok(Json(TraceFullResponse {
        status: "full_trace_complete".into(),
        operations: vec![
            "log".into(),
            "db_insert".into(),
            "redis_write".into(),
            "heartbeat".into(),
        ],
        item_id,
    }))
}

async fn demo_metric(Json(body): Json<MetricRequest>) -> impl IntoResponse {
    info!(metric_name="api.request", endpoint="demo/metric", service=SERVICE_NAME, method="POST", "api.request");
    sentry::configure_scope(|scope| {
        scope.set_tag("metric_name", &body.name);
        scope.set_tag("metric_value", &body.value.to_string());
        if let Some(tags) = body.tags {
            if let Some(obj) = tags.as_object() {
                for (key, value) in obj {
                    scope.set_tag(&format!("metric_tag_{}", key), &value.to_string());
                }
            }
        }
    });

    info!("Metric emitted: {}={}", body.name, body.value);

    Json(MetricResponse {
        status: "metric_emitted".into(),
    })
}

async fn get_items(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    info!(metric_name="api.request", endpoint="demo/db/items", service=SERVICE_NAME, method="GET", "api.request");
    let client = state.db_pool.get().await?;
    let rows = client
        .query(
            "SELECT id, service_name, message, created_at FROM demo_items ORDER BY created_at DESC LIMIT 100",
            &[],
        )
        .await?;

    let items: Vec<ItemResponse> = rows
        .iter()
        .map(|row| ItemResponse {
            id: row.get(0),
            service_name: row.get(1),
            message: row.get(2),
            created_at: row.get::<_, Option<chrono::DateTime<chrono::Utc>>>(3).map(|dt| dt.to_rfc3339()),
        })
        .collect();

    Ok(Json(ItemsResponse { items }))
}

async fn create_item(
    State(state): State<AppState>,
    Json(body): Json<CreateItemRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!(metric_name="api.request", endpoint="demo/db/items", service=SERVICE_NAME, method="POST", "api.request");
    let client = state.db_pool.get().await?;
    let row = client
        .query_one(
            "INSERT INTO demo_items (service_name, message) VALUES ($1, $2) RETURNING id, service_name, message, created_at",
            &[&SERVICE_NAME, &body.message],
        )
        .await?;

    let item = ItemResponse {
        id: row.get(0),
        service_name: row.get(1),
        message: row.get(2),
        created_at: row.get::<_, Option<chrono::DateTime<chrono::Utc>>>(3).map(|dt| dt.to_rfc3339()),
    };

    Ok((StatusCode::CREATED, Json(item)))
}

#[derive(Debug)]
enum AppError {
    Db(deadpool_postgres::PoolError),
    Postgres(tokio_postgres::Error),
    Redis(redis::RedisError),
}

impl From<deadpool_postgres::PoolError> for AppError {
    fn from(err: deadpool_postgres::PoolError) -> Self {
        AppError::Db(err)
    }
}

impl From<tokio_postgres::Error> for AppError {
    fn from(err: tokio_postgres::Error) -> Self {
        AppError::Postgres(err)
    }
}

impl From<redis::RedisError> for AppError {
    fn from(err: redis::RedisError) -> Self {
        AppError::Redis(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database pool error"),
            AppError::Postgres(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error"),
            AppError::Redis(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Redis error"),
        };

        let body = Json(serde_json::json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
