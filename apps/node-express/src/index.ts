import express, { Request, Response } from "express";
import { Pool } from "pg";
import { createClient } from "redis";
import * as Sentry from "@sentry/node";

const SERVICE_NAME = "node-express";
const PORT = 8005;

// Initialize Postgres
const pool = new Pool({
  host: process.env.POSTGRES_HOST || "localhost",
  port: parseInt(process.env.POSTGRES_PORT || "5432"),
  database: process.env.POSTGRES_DB || "cookiecrumbs",
  user: process.env.POSTGRES_USER || "cookiecrumbs",
  password: process.env.POSTGRES_PASSWORD || "cookiecrumbs",
});

// Initialize Redis
const redisClient = createClient({
  socket: {
    host: process.env.REDIS_HOST || "localhost",
    port: parseInt(process.env.REDIS_PORT || "6379"),
  },
});
redisClient.on("error", (err) => console.error("Redis Client Error", err));
redisClient.connect();

const app = express();
app.use(express.json());

// Health check
app.get("/health", async (req: Request, res: Response) => {
  Sentry.metrics.gauge("api.request", 1, {
    attributes: { endpoint: "health", service: SERVICE_NAME, method: "GET" },
  });

  let postgresStatus = "connected";
  let redisStatus = "connected";

  try {
    await pool.query("SELECT 1");
  } catch {
    postgresStatus = "disconnected";
  }

  try {
    await redisClient.ping();
  } catch {
    redisStatus = "disconnected";
  }

  Sentry.metrics.gauge("health.check", 1, {
    attributes: { service: SERVICE_NAME },
  });

  console.log(
    `[HEALTH] ${SERVICE_NAME} - postgres=${postgresStatus}, redis=${redisStatus}`
  );

  res.json({
    status: "healthy",
    service: SERVICE_NAME,
    version: "1.0.0",
    dependencies: {
      postgres: postgresStatus,
      redis: redisStatus,
    },
  });
});

// Log endpoint
app.post("/demo/log", (req: Request, res: Response) => {
  Sentry.metrics.gauge("api.request", 1, {
    attributes: { endpoint: "demo/log", service: SERVICE_NAME, method: "POST" },
  });

  const { level = "info", message, context = {} } = req.body;

  const logger = Sentry.logger;
  switch (level) {
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

  res.json({ status: "logged" });
});

// Handled error
app.post("/demo/error/handled", (req: Request, res: Response) => {
  Sentry.metrics.gauge("api.request", 1, {
    attributes: {
      endpoint: "demo/error/handled",
      service: SERVICE_NAME,
      method: "POST",
    },
  });

  const message = req.body.message || "Handled error";
  const err = new Error(message);
  Sentry.captureException(err);

  res.json({ status: "error_handled", message });
});

// Unhandled error
app.post("/demo/error/unhandled", () => {
  Sentry.metrics.gauge("api.request", 1, {
    attributes: {
      endpoint: "demo/error/unhandled",
      service: SERVICE_NAME,
      method: "POST",
    },
  });

  throw new Error("Unhandled error triggered");
});

// DB trace
app.get("/demo/trace/db", async (req: Request, res: Response) => {
  Sentry.metrics.gauge("api.request", 1, {
    attributes: {
      endpoint: "demo/trace/db",
      service: SERVICE_NAME,
      method: "GET",
    },
  });

  const result = await Sentry.startSpan(
    {
      op: "db",
      name: "SELECT COUNT(*) FROM demo_items",
      attributes: { "db.statement": "SELECT COUNT(*) FROM demo_items" },
    },
    async () => {
      const { rows } = await pool.query("SELECT COUNT(*) FROM demo_items");
      return parseInt(rows[0].count);
    }
  );

  res.json({ status: "db_trace_complete", count: result });
});

// Redis trace
app.get("/demo/trace/redis", async (req: Request, res: Response) => {
  Sentry.metrics.gauge("api.request", 1, {
    attributes: {
      endpoint: "demo/trace/redis",
      service: SERVICE_NAME,
      method: "GET",
    },
  });

  const key = `demo:${SERVICE_NAME}:counter`;

  const value = await Sentry.startSpan(
    {
      op: "cache",
      name: "redis INCR/GET",
      attributes: { "cache.key": key, "cache.operation": "INCR/GET" },
    },
    async () => {
      await redisClient.incr(key);
      return await redisClient.get(key);
    }
  );

  res.json({ status: "redis_trace_complete", value });
});

// Full trace
app.post("/demo/trace/full", async (req: Request, res: Response) => {
  Sentry.metrics.gauge("api.request", 1, {
    attributes: {
      endpoint: "demo/trace/full",
      service: SERVICE_NAME,
      method: "POST",
    },
  });

  const message = req.body.message || "Full trace test";

  // Log span
  await Sentry.startSpan(
    {
      op: "log",
      name: "emit structured log",
      attributes: { "log.message": message },
    },
    async () => {
      Sentry.logger.info(`Starting full trace: ${message}`);
    }
  );

  // DB span
  const itemId = await Sentry.startSpan(
    {
      op: "db",
      name: "INSERT INTO demo_items",
      attributes: {
        "db.statement":
          "INSERT INTO demo_items (service_name, message) VALUES ($1, $2) RETURNING id",
      },
    },
    async () => {
      const { rows } = await pool.query(
        "INSERT INTO demo_items (service_name, message) VALUES ($1, $2) RETURNING id",
        [SERVICE_NAME, message]
      );
      return rows[0].id;
    }
  );

  // Redis span
  await Sentry.startSpan(
    {
      op: "cache",
      name: "redis SET/INCR",
      attributes: { "cache.operation": "SET/INCR" },
    },
    async () => {
      await redisClient.set(`demo:${SERVICE_NAME}:last-log`, message);
      await redisClient.incr(`demo:${SERVICE_NAME}:counter`);
      await redisClient.set("demo:shared:heartbeat", SERVICE_NAME);
    }
  );

  res.json({
    status: "full_trace_complete",
    operations: ["log", "db_insert", "redis_write", "heartbeat"],
    item_id: itemId,
  });
});

// Metric endpoint
app.post("/demo/metric", (req: Request, res: Response) => {
  Sentry.metrics.gauge("api.request", 1, {
    attributes: { endpoint: "demo/metric", service: SERVICE_NAME, method: "POST" },
  });

  const { name = "demo.counter", value = 1, tags = {} } = req.body;

  Sentry.setTag("metric_name", name);
  Sentry.setTag("metric_value", String(value));
  for (const [key, val] of Object.entries(tags)) {
    Sentry.setTag(`metric_tag_${key}`, String(val));
  }

  Sentry.logger.info(`Metric emitted: ${name}=${value}`, { metric_tags: tags });

  res.json({ status: "metric_emitted" });
});

// Get items
app.get("/demo/db/items", async (req: Request, res: Response) => {
  Sentry.metrics.gauge("api.request", 1, {
    attributes: {
      endpoint: "demo/db/items",
      service: SERVICE_NAME,
      method: "GET",
    },
  });

  const items = await Sentry.startSpan(
    {
      op: "db",
      name: "SELECT items",
      attributes: {
        "db.statement":
          "SELECT id, service_name, message, created_at FROM demo_items ORDER BY created_at DESC LIMIT 100",
      },
    },
    async () => {
      const { rows } = await pool.query(
        "SELECT id, service_name, message, created_at FROM demo_items ORDER BY created_at DESC LIMIT 100"
      );
      return rows.map((row) => ({
        id: row.id,
        service_name: row.service_name,
        message: row.message,
        created_at: row.created_at ? row.created_at.toISOString() : null,
      }));
    }
  );

  res.json({ items });
});

// Create item
app.post("/demo/db/items", async (req: Request, res: Response) => {
  Sentry.metrics.gauge("api.request", 1, {
    attributes: {
      endpoint: "demo/db/items",
      service: SERVICE_NAME,
      method: "POST",
    },
  });

  const { message } = req.body;

  const item = await Sentry.startSpan(
    {
      op: "db",
      name: "INSERT item",
      attributes: {
        "db.statement":
          "INSERT INTO demo_items (service_name, message) VALUES ($1, $2) RETURNING id, service_name, message, created_at",
      },
    },
    async () => {
      const { rows } = await pool.query(
        "INSERT INTO demo_items (service_name, message) VALUES ($1, $2) RETURNING id, service_name, message, created_at",
        [SERVICE_NAME, message]
      );
      return {
        id: rows[0].id,
        service_name: rows[0].service_name,
        message: rows[0].message,
        created_at: rows[0].created_at
          ? rows[0].created_at.toISOString()
          : null,
      };
    }
  );

  res.status(201).json(item);
});

// Sentry error handler
Sentry.setupExpressErrorHandler(app);

app.listen(PORT, "0.0.0.0", () => {
  console.log(`Server starting on :${PORT}`);
});
