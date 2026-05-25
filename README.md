# Cookiecrumbs

A polyglot Sentry demo monorepo. Four independent services (Python/Django, Java/Spring Boot, Rust/Axum, Go/Gin) expose the same REST API on different ports, each connecting to the same Postgres and Redis, and each reporting to its own Sentry project using its native SDK.

## Services

| Service | Language | Port | Sentry Project |
|---------|----------|------|----------------|
| python-django | Python / Django | 8001 | Separate DSN |
| java-spring | Java / Spring Boot | 8002 | Separate DSN |
| rust-axum | Rust / Axum | 8003 | Separate DSN |
| go-gin | Go / Gin | 8004 | Separate DSN |

Shared infrastructure:
- Postgres: port 5432
- Redis: port 6379

## Quick Start

1. Enter the Nix shell:
   ```bash
   nix-shell
   ```

2. Start Postgres and Redis:
   ```bash
   just infra-up
   ```

3. Apply migrations:
   ```bash
   just migrate
   ```

4. Run a service:
   ```bash
   just run-django
   ```

5. Test with Postman:
   - Import `api-contract/postman/sentry-demo.postman_collection.json`
   - Set `baseUrl` to `http://localhost:8001` (or the port of the service you want to test)

## Project Rules

- **Shared API contract**: All 4 apps must match `api-contract/openapi.yaml`. Django is the reference implementation.
- **Migration ownership**: Only shared SQL migrations under `infra/migrations/`. No app owns the schema.
- **Infra**: Postgres and Redis run as plain Docker containers via `just`. No `docker-compose.yml`.
- **Telemetry**: Each app uses its own native Sentry SDK and its own DSN. No shared telemetry wrapper.
- **Sentry config isolation**: `SENTRY_DSN`, `SENTRY_ENVIRONMENT`, and `SENTRY_RELEASE` are per-app only. No root-level Sentry variables.
- **Rollout**: Django first, then remaining services after behavior confirmation.
- **Postman**: Collection uses `{{baseUrl}}` variable only.

## API Endpoints

All services expose:

1. `GET /health`
2. `POST /demo/log`
3. `POST /demo/error/handled`
4. `POST /demo/error/unhandled`
5. `GET /demo/trace/db`
6. `GET /demo/trace/redis`
7. `POST /demo/trace/full`
8. `POST /demo/metric`
9. `GET /demo/db/items`
10. `POST /demo/db/items`

## Environment

- Shared infra config lives at the repo root in `.env`.
- Sentry config is isolated per service in each app's `.env`.

## License

MIT
