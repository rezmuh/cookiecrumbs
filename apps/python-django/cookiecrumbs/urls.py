"""cookiecrumbs URL Configuration"""

from django.urls import path
from demo import views

urlpatterns = [
    path("health", views.health, name="health"),
    path("demo/log", views.demo_log, name="demo_log"),
    path("demo/error/handled", views.demo_error_handled, name="demo_error_handled"),
    path(
        "demo/error/unhandled", views.demo_error_unhandled, name="demo_error_unhandled"
    ),
    path("demo/trace/db", views.demo_trace_db, name="demo_trace_db"),
    path("demo/trace/redis", views.demo_trace_redis, name="demo_trace_redis"),
    path("demo/trace/full", views.demo_trace_full, name="demo_trace_full"),
    path("demo/metric", views.demo_metric, name="demo_metric"),
    path("demo/db/items", views.demo_db_items, name="demo_db_items"),
]
