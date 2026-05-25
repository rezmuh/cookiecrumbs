import json
import logging
import os

import redis
import sentry_sdk
from django.conf import settings
from django.db import connection
from django.http import JsonResponse
from django.views.decorators.csrf import csrf_exempt
from django.views.decorators.http import require_http_methods

logger = logging.getLogger(__name__)

SERVICE_NAME = "python-django"


def get_redis_client():
    return redis.Redis(
        host=settings.REDIS_HOST, port=settings.REDIS_PORT, decode_responses=True
    )


@require_http_methods(["GET"])
def health(request):
    sentry_sdk.metrics.gauge(
        "api.request",
        1,
        attributes={"endpoint": "health", "service": SERVICE_NAME, "method": "GET"},
    )
    postgres_status = "connected"
    redis_status = "connected"

    try:
        with connection.cursor() as cursor:
            cursor.execute("SELECT 1")
    except Exception:
        postgres_status = "disconnected"

    try:
        r = get_redis_client()
        r.ping()
    except Exception:
        redis_status = "disconnected"

    sentry_sdk.metrics.gauge("health.check", 1, attributes={"service": SERVICE_NAME})
    logger.info(
        f"[HEALTH] {SERVICE_NAME} - postgres={postgres_status}, redis={redis_status}"
    )

    return JsonResponse(
        {
            "status": "healthy",
            "service": SERVICE_NAME,
            "version": "1.0.0",
            "dependencies": {
                "postgres": postgres_status,
                "redis": redis_status,
            },
        }
    )


@csrf_exempt
@require_http_methods(["POST"])
def demo_log(request):
    sentry_sdk.metrics.gauge(
        "api.request",
        1,
        attributes={"endpoint": "demo/log", "service": SERVICE_NAME, "method": "POST"},
    )
    data = json.loads(request.body)
    level = data.get("level", "info")
    message = data.get("message", "")
    context = data.get("context", {})

    log_func = getattr(logger, level, logger.info)
    log_func(message, extra={"context": context})

    return JsonResponse({"status": "logged"})


@csrf_exempt
@require_http_methods(["POST"])
def demo_error_handled(request):
    sentry_sdk.metrics.gauge(
        "api.request",
        1,
        attributes={
            "endpoint": "demo/error/handled",
            "service": SERVICE_NAME,
            "method": "POST",
        },
    )
    data = json.loads(request.body)
    message = data.get("message", "Handled error")

    try:
        raise ValueError(message)
    except Exception as e:
        sentry_sdk.capture_exception(e)

    return JsonResponse(
        {
            "status": "error_handled",
            "message": message,
        }
    )


@csrf_exempt
@require_http_methods(["POST"])
def demo_error_unhandled(request):
    sentry_sdk.metrics.gauge(
        "api.request",
        1,
        attributes={
            "endpoint": "demo/error/unhandled",
            "service": SERVICE_NAME,
            "method": "POST",
        },
    )
    raise RuntimeError("Unhandled error triggered")


@require_http_methods(["GET"])
def demo_trace_db(request):
    sentry_sdk.metrics.gauge(
        "api.request",
        1,
        attributes={
            "endpoint": "demo/trace/db",
            "service": SERVICE_NAME,
            "method": "GET",
        },
    )
    with connection.cursor() as cursor:
        cursor.execute("SELECT COUNT(*) FROM demo_items")
        count = cursor.fetchone()[0]

    return JsonResponse(
        {
            "status": "db_trace_complete",
            "count": count,
        }
    )


@require_http_methods(["GET"])
def demo_trace_redis(request):
    sentry_sdk.metrics.gauge(
        "api.request",
        1,
        attributes={
            "endpoint": "demo/trace/redis",
            "service": SERVICE_NAME,
            "method": "GET",
        },
    )
    r = get_redis_client()
    key = f"demo:{SERVICE_NAME}:counter"

    r.incr(key)
    value = r.get(key)

    return JsonResponse(
        {
            "status": "redis_trace_complete",
            "value": value,
        }
    )


@csrf_exempt
@require_http_methods(["POST"])
def demo_trace_full(request):
    sentry_sdk.metrics.gauge(
        "api.request",
        1,
        attributes={
            "endpoint": "demo/trace/full",
            "service": SERVICE_NAME,
            "method": "POST",
        },
    )
    data = json.loads(request.body)
    message = data.get("message", "Full trace test")

    logger.info(f"Starting full trace: {message}")

    # DB operation
    with connection.cursor() as cursor:
        cursor.execute(
            "INSERT INTO demo_items (service_name, message) VALUES (%s, %s) RETURNING id",
            [SERVICE_NAME, message],
        )
        item_id = cursor.fetchone()[0]

    # Redis operation
    r = get_redis_client()
    r.set(f"demo:{SERVICE_NAME}:last-log", message)
    r.incr(f"demo:{SERVICE_NAME}:counter")

    # Update shared heartbeat
    r.set("demo:shared:heartbeat", SERVICE_NAME)

    return JsonResponse(
        {
            "status": "full_trace_complete",
            "operations": ["log", "db_insert", "redis_write", "heartbeat"],
            "item_id": item_id,
        }
    )


@csrf_exempt
@require_http_methods(["POST"])
def demo_metric(request):
    sentry_sdk.metrics.gauge(
        "api.request",
        1,
        attributes={
            "endpoint": "demo/metric",
            "service": SERVICE_NAME,
            "method": "POST",
        },
    )
    data = json.loads(request.body)
    name = data.get("name", "demo.counter")
    value = data.get("value", 1)
    tags = data.get("tags", {})

    # Emit metric via Sentry
    sentry_sdk.set_tag("metric_name", name)
    sentry_sdk.set_tag("metric_value", str(value))
    for tag_key, tag_value in tags.items():
        sentry_sdk.set_tag(f"metric_tag_{tag_key}", str(tag_value))

    logger.info(f"Metric emitted: {name}={value}", extra={"metric_tags": tags})

    return JsonResponse({"status": "metric_emitted"})


@require_http_methods(["GET", "POST"])
def demo_db_items(request):
    if request.method == "GET":
        sentry_sdk.metrics.gauge(
            "api.request",
            1,
            attributes={
                "endpoint": "demo/db/items",
                "service": SERVICE_NAME,
                "method": "GET",
            },
        )
        with connection.cursor() as cursor:
            cursor.execute(
                "SELECT id, service_name, message, created_at FROM demo_items ORDER BY created_at DESC LIMIT 100"
            )
            rows = cursor.fetchall()

        items = [
            {
                "id": row[0],
                "service_name": row[1],
                "message": row[2],
                "created_at": row[3].isoformat() if row[3] else None,
            }
            for row in rows
        ]

        return JsonResponse({"items": items})

    else:  # POST
        sentry_sdk.metrics.gauge(
            "api.request",
            1,
            attributes={
                "endpoint": "demo/db/items",
                "service": SERVICE_NAME,
                "method": "POST",
            },
        )
        data = json.loads(request.body)
        message = data.get("message", "")

        with connection.cursor() as cursor:
            cursor.execute(
                "INSERT INTO demo_items (service_name, message) VALUES (%s, %s) RETURNING id, service_name, message, created_at",
                [SERVICE_NAME, message],
            )
            row = cursor.fetchone()

        return JsonResponse(
            {
                "id": row[0],
                "service_name": row[1],
                "message": row[2],
                "created_at": row[3].isoformat() if row[3] else None,
            },
            status=201,
        )
