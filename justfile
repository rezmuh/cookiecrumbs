# Cookiecrumbs task runner
# See AGENTS.md for project context

set dotenv-load

# Default recipe
default:
    @just --list

# Infrastructure
infra-up:
    #!/usr/bin/env bash
    set -euo pipefail
    
    # Load env vars
    export $(grep -v '^#' .env | xargs)
    
    # Start Postgres
    if ! docker ps --format "table {{ '{{' }}.Names{{ '}}' }}" | grep -q "cookiecrumbs-postgres"; then
        echo "Starting Postgres..."
        docker run -d \
            --name cookiecrumbs-postgres \
            -e POSTGRES_DB=$POSTGRES_DB \
            -e POSTGRES_USER=$POSTGRES_USER \
            -e POSTGRES_PASSWORD=$POSTGRES_PASSWORD \
            -p $POSTGRES_PORT:5432 \
            postgres:16-alpine
        echo "Waiting for Postgres to be ready..."
        sleep 3
    else
        echo "Postgres already running"
    fi
    
    # Start Redis
    if ! docker ps --format "table {{ '{{' }}.Names{{ '}}' }}" | grep -q "cookiecrumbs-redis"; then
        echo "Starting Redis..."
        docker run -d \
            --name cookiecrumbs-redis \
            -p $REDIS_PORT:6379 \
            redis:7-alpine
        echo "Waiting for Redis to be ready..."
        sleep 2
    else
        echo "Redis already running"
    fi
    
    echo "Infrastructure is up!"

infra-down:
    #!/usr/bin/env bash
    set -euo pipefail
    
    echo "Stopping containers..."
    docker stop cookiecrumbs-postgres cookiecrumbs-redis 2>/dev/null || true
    docker rm cookiecrumbs-postgres cookiecrumbs-redis 2>/dev/null || true
    echo "Infrastructure is down!"

infra-logs-postgres:
    docker logs -f cookiecrumbs-postgres

infra-logs-redis:
    docker logs -f cookiecrumbs-redis

# Database migrations
migrate:
    #!/usr/bin/env bash
    set -euo pipefail
    
    export $(grep -v '^#' .env | xargs)
    export DATABASE_URL="postgres://$POSTGRES_USER:$POSTGRES_PASSWORD@$POSTGRES_HOST:$POSTGRES_PORT/$POSTGRES_DB?sslmode=disable"
    
    echo "Applying migrations..."
    dbmate --migrations-dir infra/migrations up
    echo "Migrations applied!"

migrate-status:
    #!/usr/bin/env bash
    set -euo pipefail
    
    export $(grep -v '^#' .env | xargs)
    export DATABASE_URL="postgres://$POSTGRES_USER:$POSTGRES_PASSWORD@$POSTGRES_HOST:$POSTGRES_PORT/$POSTGRES_DB?sslmode=disable"
    
    dbmate --migrations-dir infra/migrations status

# Services
run-django:
    #!/usr/bin/env bash
    set -euo pipefail
    cd apps/python-django
    export $(grep -v '^#' ../../.env | xargs)
    export $(grep -v '^#' .env | xargs)
    source venv/bin/activate
    python manage.py runserver 0.0.0.0:8001

run-java:
    #!/usr/bin/env bash
    set -euo pipefail
    cd apps/java-spring
    set -a
    source ../../.env
    source .env
    set +a
    gradle bootRun --no-daemon

run-rust:
    #!/usr/bin/env bash
    set -euo pipefail
    cd apps/rust-axum
    set -a
    source ../../.env
    source .env
    set +a
    cargo run

run-go:
    #!/usr/bin/env bash
    set -euo pipefail
    cd apps/go-gin
    set -a
    source ../../.env
    source .env
    set +a
    go run .

run-node:
    #!/usr/bin/env bash
    set -euo pipefail
    cd apps/node-express
    set -a
    source ../../.env
    source .env
    set +a
    npx tsx --import ./src/instrument.ts src/index.ts
