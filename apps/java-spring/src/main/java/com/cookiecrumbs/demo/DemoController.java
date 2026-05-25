package com.cookiecrumbs.demo;

import org.springframework.web.bind.annotation.*;
import org.springframework.http.ResponseEntity;
import org.springframework.http.HttpStatus;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.beans.factory.annotation.Autowired;

import io.sentry.Sentry;
import io.sentry.SentryLevel;
import io.sentry.metrics.SentryMetricsParameters;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.*;

@RestController
@RequestMapping("/")
public class DemoController {
    private static final Logger logger = LoggerFactory.getLogger(DemoController.class);
    private static final String SERVICE_NAME = "java-spring";

    @Autowired
    private JdbcTemplate jdbcTemplate;

    @Autowired
    private StringRedisTemplate redisTemplate;

    @GetMapping("/health")
    public ResponseEntity<Map<String, Object>> health() {
        Sentry.metrics().count("api.request", 1.0);
        String postgresStatus = "connected";
        String redisStatus = "connected";

        try {
            jdbcTemplate.queryForObject("SELECT 1", Integer.class);
        } catch (Exception e) {
            postgresStatus = "disconnected";
        }

        try {
            redisTemplate.opsForValue().get("health-check");
        } catch (Exception e) {
            redisStatus = "disconnected";
        }

        Sentry.metrics().count("health.check", 1.0);
        System.out.println("[HEALTH] " + SERVICE_NAME + " - postgres=" + postgresStatus + ", redis=" + redisStatus);

        Map<String, Object> response = new HashMap<>();
        response.put("status", "healthy");
        response.put("service", SERVICE_NAME);
        response.put("version", "1.0.0");
        
        Map<String, String> deps = new HashMap<>();
        deps.put("postgres", postgresStatus);
        deps.put("redis", redisStatus);
        response.put("dependencies", deps);

        return ResponseEntity.ok(response);
    }

    @PostMapping("/demo/log")
    public ResponseEntity<Map<String, String>> demoLog(@RequestBody Map<String, Object> body) {
        Sentry.metrics().count("api.request", 1.0);
        String level = (String) body.getOrDefault("level", "info");
        String message = (String) body.getOrDefault("message", "");
        Map<String, Object> context = (Map<String, Object>) body.getOrDefault("context", new HashMap<>());

        switch (level.toLowerCase()) {
            case "debug":
                logger.debug(message, context);
                break;
            case "warning":
                logger.warn(message, context);
                break;
            case "error":
                logger.error(message, context);
                break;
            default:
                logger.info(message, context);
        }

        Map<String, String> response = new HashMap<>();
        response.put("status", "logged");
        return ResponseEntity.ok(response);
    }

    @PostMapping("/demo/error/handled")
    public ResponseEntity<Map<String, String>> demoErrorHandled(@RequestBody Map<String, Object> body) {
        Sentry.metrics().count("api.request", 1.0);
        String message = (String) body.getOrDefault("message", "Handled error");

        try {
            throw new RuntimeException(message);
        } catch (Exception e) {
            Sentry.captureException(e);
        }

        Map<String, String> response = new HashMap<>();
        response.put("status", "error_handled");
        response.put("message", message);
        return ResponseEntity.ok(response);
    }

    @PostMapping("/demo/error/unhandled")
    public ResponseEntity<Map<String, String>> demoErrorUnhandled() {
        Sentry.metrics().count("api.request", 1.0);
        throw new RuntimeException("Unhandled error triggered");
    }

    @GetMapping("/demo/trace/db")
    public ResponseEntity<Map<String, Object>> demoTraceDb() {
        Sentry.metrics().count("api.request", 1.0);
        Integer count = jdbcTemplate.queryForObject("SELECT COUNT(*) FROM demo_items", Integer.class);

        Map<String, Object> response = new HashMap<>();
        response.put("status", "db_trace_complete");
        response.put("count", count);
        return ResponseEntity.ok(response);
    }

    @GetMapping("/demo/trace/redis")
    public ResponseEntity<Map<String, String>> demoTraceRedis() {
        Sentry.metrics().count("api.request", 1.0);
        String key = String.format("demo:%s:counter", SERVICE_NAME);
        redisTemplate.opsForValue().increment(key);
        String value = redisTemplate.opsForValue().get(key);

        Map<String, String> response = new HashMap<>();
        response.put("status", "redis_trace_complete");
        response.put("value", value);
        return ResponseEntity.ok(response);
    }

    @PostMapping("/demo/trace/full")
    public ResponseEntity<Map<String, Object>> demoTraceFull(@RequestBody Map<String, Object> body) {
        Sentry.metrics().count("api.request", 1.0);
        String message = (String) body.getOrDefault("message", "Full trace test");

        logger.info("Starting full trace: {}", message);

        // DB operation
        Integer itemId = jdbcTemplate.queryForObject(
            "INSERT INTO demo_items (service_name, message) VALUES (?, ?) RETURNING id",
            Integer.class,
            SERVICE_NAME, message
        );

        // Redis operations
        redisTemplate.opsForValue().set(String.format("demo:%s:last-log", SERVICE_NAME), message);
        redisTemplate.opsForValue().increment(String.format("demo:%s:counter", SERVICE_NAME));
        redisTemplate.opsForValue().set("demo:shared:heartbeat", SERVICE_NAME);

        Map<String, Object> response = new HashMap<>();
        response.put("status", "full_trace_complete");
        response.put("operations", Arrays.asList("log", "db_insert", "redis_write", "heartbeat"));
        response.put("item_id", itemId);
        return ResponseEntity.ok(response);
    }

    @PostMapping("/demo/metric")
    public ResponseEntity<Map<String, String>> demoMetric(@RequestBody Map<String, Object> body) {
        Sentry.metrics().count("api.request", 1.0);
        String name = (String) body.getOrDefault("name", "demo.counter");
        Number value = (Number) body.getOrDefault("value", 1);
        Map<String, Object> tags = (Map<String, Object>) body.getOrDefault("tags", new HashMap<>());

        Sentry.setTag("metric_name", name);
        Sentry.setTag("metric_value", value.toString());
        for (Map.Entry<String, Object> entry : tags.entrySet()) {
            Sentry.setTag("metric_tag_" + entry.getKey(), entry.getValue().toString());
        }

        logger.info("Metric emitted: {}={}", name, value);

        Map<String, String> response = new HashMap<>();
        response.put("status", "metric_emitted");
        return ResponseEntity.ok(response);
    }

    @GetMapping("/demo/db/items")
    public ResponseEntity<Map<String, Object>> getItems() {
        Sentry.metrics().count("api.request", 1.0);
        List<Map<String, Object>> items = jdbcTemplate.query(
            "SELECT id, service_name, message, created_at FROM demo_items ORDER BY created_at DESC LIMIT 100",
            (rs, rowNum) -> {
                Map<String, Object> item = new HashMap<>();
                item.put("id", rs.getInt("id"));
                item.put("service_name", rs.getString("service_name"));
                item.put("message", rs.getString("message"));
                item.put("created_at", rs.getTimestamp("created_at") != null ? rs.getTimestamp("created_at").toInstant().toString() : null);
                return item;
            }
        );

        Map<String, Object> response = new HashMap<>();
        response.put("items", items);
        return ResponseEntity.ok(response);
    }

    @PostMapping("/demo/db/items")
    public ResponseEntity<Map<String, Object>> createItem(@RequestBody Map<String, Object> body) {
        Sentry.metrics().count("api.request", 1.0);
        String message = (String) body.getOrDefault("message", "");

        Map<String, Object> result = jdbcTemplate.queryForMap(
            "INSERT INTO demo_items (service_name, message) VALUES (?, ?) RETURNING id, service_name, message, created_at",
            SERVICE_NAME, message
        );

        Map<String, Object> response = new HashMap<>();
        response.put("id", result.get("id"));
        response.put("service_name", result.get("service_name"));
        response.put("message", result.get("message"));
        response.put("created_at", result.get("created_at") != null ? result.get("created_at").toString() : null);
        return ResponseEntity.status(HttpStatus.CREATED).body(response);
    }
}
