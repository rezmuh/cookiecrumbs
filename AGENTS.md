# Cookiecrumbs - Agent Context

## Project Purpose
A polyglot Sentry demo monorepo. Four independent services expose the same REST API on different ports, each connecting to the same Postgres and Redis, and each reporting to its own Sentry project using its native SDK.

## Service Inventory

| Service | Language | Framework | Port | Sentry Config |
|---------|----------|-----------|------|---------------|
| python-django | Python | Django | 8001 | Per-app: DSN, ENVIRONMENT, RELEASE |
| java-spring | Java | Spring Boot | 8002 | Per-app: DSN, ENVIRONMENT, RELEASE |
| rust-axum | Rust | Axum | 8003 | Per-app: DSN, ENVIRONMENT, RELEASE |
| go-gin | Go | Gin | 8004 | Per-app: DSN, ENVIRONMENT, RELEASE |

Shared Infrastructure:
- Postgres: port 5432
- Redis: port 6379

## Project Rules

1. **Shared API Contract**: All 4 apps must match `api-contract/openapi.yaml`. Django is the reference implementation.
2. **Migration Ownership**: Only shared SQL migrations under `infra/migrations/`. No single app owns the schema.
3. **Infra**: Postgres and Redis run as plain Docker containers via `just`. No `docker-compose.yml`.
4. **Telemetry**: Each app uses its own native Sentry SDK and its own DSN. No shared telemetry abstraction layer.
5. **Sentry Config Isolation**: `SENTRY_DSN`, `SENTRY_ENVIRONMENT`, and `SENTRY_RELEASE` are per-app only. No root-level Sentry variables.
6. **Rollout**: Django first, then remaining services after behavior confirmation.
7. **Postman**: Collection uses `{{baseUrl}}` variable only.
8. **Build Tools**: Gradle for Java, dbmate for migrations, just for task runner.
9. **Network Binding**: All services must bind to `0.0.0.0` to allow access from other machines on the local network.

## Environment Layout

- Root `.env` / `.env.example`: Shared infra config only (Postgres, Redis).
- Per-app `.env` / `.env.example`: App port + Sentry config (DSN, ENVIRONMENT, RELEASE).

## Sentry SDK Versions

Minimum versions required for logs and metrics support:

| Service | SDK | Minimum Version | Logs Enabled | Metrics Enabled |
|---------|-----|----------------|--------------|-----------------|
| python-django | sentry-sdk | 2.44.0 | `enable_logs=True` | `sentry_sdk.metrics.gauge()` (use `attributes` not `tags`) |
| java-spring | sentry-java | 8.34.0 | `sentry.logs.enabled=true` | `Sentry.metrics().count()` |
| rust-axum | sentry-rust | 0.42.0 | `enable_logs: true` + tracing layer | Via tracing `info!` macros |
| go-gin | sentry-go | 0.35.0 | `EnableLogs: true` + `sentry.NewLogger(ctx)` | `sentry.CaptureMessage()` (workaround) |

## API Surface

All services expose identical endpoints:

1. `GET /health` - Service metadata and dependency checks
2. `POST /demo/log` - Emit structured logs
3. `POST /demo/error/handled` - Capture handled exception
4. `POST /demo/error/unhandled` - Trigger unhandled exception (500)
5. `GET /demo/trace/db` - Postgres query inside named span
6. `GET /demo/trace/redis` - Redis operation inside named span
7. `POST /demo/trace/full` - Combined trace: log + DB + Redis + nested spans
8. `POST /demo/metric` - Emit custom metric
9. `GET /demo/db/items` - Read shared table rows
10. `POST /demo/db/items` - Insert row into shared table

## Database Schema

Shared table: `demo_items`
- `id` (serial primary key)
- `service_name` (text)
- `message` (text)
- `created_at` (timestamp)

## Redis Key Scheme

- `demo:{service}:last-log`
- `demo:{service}:counter`
- `demo:shared:heartbeat`

## Telemetry Intent

Each runtime should reveal:
- What Sentry captures by default after SDK install
- What appears after framework integration
- What requires extra instrumentation

Preserve language-specific differences as part of the demo. Do not normalize behavior across languages.

## Metrics Implementation

All services emit metrics on every API endpoint:

**`api.request` gauge** - emitted on all 10 endpoints to track API usage:
- **Python**: `sentry_sdk.metrics.gauge("api.request", 1, attributes={"endpoint": "...", "service": SERVICE_NAME, "method": "..."})`
- **Java**: `Sentry.metrics().increment("api.request", 1.0, Map.of("endpoint", "...", "service", SERVICE_NAME, "method", "..."))`
- **Rust**: `info!(metric_name="api.request", endpoint="...", service=SERVICE_NAME, method="...", "api.request")`
- **Go**: `sentry.CaptureMessage("api.request:<endpoint>")` (workaround)

**`health.check` gauge** - emitted on `GET /health` endpoint:
- **Python**: `sentry_sdk.metrics.gauge("health.check", 1, attributes={"service": SERVICE_NAME})`
- **Java**: `Sentry.metrics().increment("health.check", 1.0, Map.of("service", SERVICE_NAME))`
- **Rust**: Via tracing integration
- **Go**: `sentry.CaptureMessage("health.check")`

## Implementation Sequence

1. Root scaffolding (AGENTS.md, shell.nix, justfile, .env.example, README.md)
2. Shared infra (dbmate migrations, openapi.yaml)
3. Django reference app with Sentry integration
4. Validate Django behavior in Sentry
5. Freeze contract
6. Implement Java/Spring Boot
7. Implement Rust/Axum
8. Implement Go/Gin
9. Postman collection
10. Parity verification

## Commands

- `just infra-up` - Start Postgres and Redis containers
- `just infra-down` - Stop and remove containers
- `just migrate` - Apply database migrations
- `just run-django` - Run Django service
- `just run-java` - Run Java service
- `just run-rust` - Run Rust service
- `just run-go` - Run Go service

## Non-Goals

- No Docker Compose
- No Makefile (use justfile)
- No shared telemetry wrapper
- No auth or frontend
- No service-to-service communication
- No production deployment manifests
