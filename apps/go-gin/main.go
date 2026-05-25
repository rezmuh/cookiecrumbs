package main

import (
	"database/sql"
	"fmt"
	"net/http"
	"os"
	"time"

	"github.com/getsentry/sentry-go"
	sentrygin "github.com/getsentry/sentry-go/gin"
	"github.com/gin-gonic/gin"
	"github.com/joho/godotenv"
	_ "github.com/lib/pq"
	"github.com/redis/go-redis/v9"
)

const serviceName = "go-gin"

type HealthResponse struct {
	Status       string       `json:"status"`
	Service      string       `json:"service"`
	Version      string       `json:"version"`
	Dependencies Dependencies `json:"dependencies"`
}

type Dependencies struct {
	Postgres string `json:"postgres"`
	Redis    string `json:"redis"`
}

type LogRequest struct {
	Level   string                 `json:"level"`
	Message string                 `json:"message"`
	Context map[string]interface{} `json:"context,omitempty"`
}

type ErrorRequest struct {
	Message string `json:"message"`
}

type ErrorResponse struct {
	Status  string `json:"status"`
	Message string `json:"message"`
}

type TraceFullRequest struct {
	Message string `json:"message"`
}

type TraceFullResponse struct {
	Status     string   `json:"status"`
	Operations []string `json:"operations"`
	ItemID     int      `json:"item_id"`
}

type MetricRequest struct {
	Name  string                 `json:"name"`
	Value float64                `json:"value"`
	Tags  map[string]interface{} `json:"tags,omitempty"`
}

type CreateItemRequest struct {
	Message string `json:"message"`
}

type ItemResponse struct {
	ID          int     `json:"id"`
	ServiceName string  `json:"service_name"`
	Message     string  `json:"message"`
	CreatedAt   *string `json:"created_at"`
}

type ItemsResponse struct {
	Items []ItemResponse `json:"items"`
}

type TraceDbResponse struct {
	Status string `json:"status"`
	Count  int    `json:"count"`
}

type TraceRedisResponse struct {
	Status string `json:"status"`
	Value  string `json:"value"`
}

var (
	db          *sql.DB
	redisClient *redis.Client
)

func main() {
	godotenv.Load()

	err := sentry.Init(sentry.ClientOptions{
		Dsn:              os.Getenv("SENTRY_DSN"),
		Environment:      os.Getenv("SENTRY_ENVIRONMENT"),
		Release:          os.Getenv("SENTRY_RELEASE"),
		TracesSampleRate: 1.0,
		EnableLogs:       true,
		EnableTracing:    true,
	})
	if err != nil {
		fmt.Printf("Sentry initialization failed: %v\n", err)
	}
	defer sentry.Flush(2 * time.Second)

	initDB()
	initRedis()

	r := gin.Default()
	r.Use(sentrygin.New(sentrygin.Options{}))

	r.GET("/health", healthHandler)
	r.POST("/demo/log", demoLogHandler)
	r.POST("/demo/error/handled", demoErrorHandledHandler)
	r.POST("/demo/error/unhandled", demoErrorUnhandledHandler)
	r.GET("/demo/trace/db", demoTraceDbHandler)
	r.GET("/demo/trace/redis", demoTraceRedisHandler)
	r.POST("/demo/trace/full", demoTraceFullHandler)
	r.POST("/demo/metric", demoMetricHandler)
	r.GET("/demo/db/items", getItemsHandler)
	r.POST("/demo/db/items", createItemHandler)

	fmt.Println("Server starting on :8004")
	r.Run("0.0.0.0:8004")
}

func initDB() {
	connStr := fmt.Sprintf("host=%s port=%s dbname=%s user=%s password=%s sslmode=disable",
		os.Getenv("POSTGRES_HOST"),
		os.Getenv("POSTGRES_PORT"),
		os.Getenv("POSTGRES_DB"),
		os.Getenv("POSTGRES_USER"),
		os.Getenv("POSTGRES_PASSWORD"),
	)

	var err error
	db, err = sql.Open("postgres", connStr)
	if err != nil {
		panic(err)
	}
}

func initRedis() {
	redisClient = redis.NewClient(&redis.Options{
		Addr: fmt.Sprintf("%s:%s",
			os.Getenv("REDIS_HOST"),
			os.Getenv("REDIS_PORT"),
		),
	})
}

func healthHandler(c *gin.Context) {
	sentry.CaptureMessage("api.request:health")
	postgresStatus := "connected"
	redisStatus := "connected"

	if err := db.Ping(); err != nil {
		postgresStatus = "disconnected"
	}

	if err := redisClient.Ping(c.Request.Context()).Err(); err != nil {
		redisStatus = "disconnected"
	}

	// Use Sentry Logger for structured logs
	logger := sentry.NewLogger(c.Request.Context())
	logger.Info().
		String("service", serviceName).
		String("postgres", postgresStatus).
		String("redis", redisStatus).
		Emit("[HEALTH] health check performed")

	fmt.Printf("[HEALTH] %s - postgres=%s, redis=%s\n", serviceName, postgresStatus, redisStatus)

	c.JSON(http.StatusOK, HealthResponse{
		Status:  "healthy",
		Service: serviceName,
		Version: "1.0.0",
		Dependencies: Dependencies{
			Postgres: postgresStatus,
			Redis:    redisStatus,
		},
	})
}

func demoLogHandler(c *gin.Context) {
	sentry.CaptureMessage("api.request:demo/log")
	var req LogRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	level := req.Level
	if level == "" {
		level = "info"
	}

	logger := sentry.NewLogger(c.Request.Context())
	switch level {
	case "debug":
		logger.Debug().Emit(req.Message)
	case "warning":
		logger.Warn().Emit(req.Message)
	case "error":
		logger.Error().Emit(req.Message)
	default:
		logger.Info().Emit(req.Message)
	}

	c.JSON(http.StatusOK, gin.H{"status": "logged"})
}

func demoErrorHandledHandler(c *gin.Context) {
	sentry.CaptureMessage("api.request:demo/error/handled")
	var req ErrorRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	message := req.Message
	if message == "" {
		message = "Handled error"
	}

	err := fmt.Errorf("%s", message)
	sentry.CaptureException(err)

	c.JSON(http.StatusOK, ErrorResponse{
		Status:  "error_handled",
		Message: message,
	})
}

func demoErrorUnhandledHandler(c *gin.Context) {
	sentry.CaptureMessage("api.request:demo/error/unhandled")
	panic("Unhandled error triggered")
}

func demoTraceDbHandler(c *gin.Context) {
	logger := sentry.NewLogger(c.Request.Context())
	logger.Info().Emit("Starting DB trace")

	var count int
	err := db.QueryRow("SELECT COUNT(*) FROM demo_items").Scan(&count)
	if err != nil {
		logger.Error().Emitf("DB query failed: %v", err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}

	logger.Info().Int("count", count).Emit("DB trace complete")
	c.JSON(http.StatusOK, TraceDbResponse{
		Status: "db_trace_complete",
		Count:  count,
	})
}

func demoTraceRedisHandler(c *gin.Context) {
	logger := sentry.NewLogger(c.Request.Context())
	logger.Info().Emit("Starting Redis trace")

	key := fmt.Sprintf("demo:%s:counter", serviceName)
	ctx := c.Request.Context()

	err := redisClient.Incr(ctx, key).Err()
	if err != nil {
		logger.Error().Emitf("Redis incr failed: %v", err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}

	value, err := redisClient.Get(ctx, key).Result()
	if err != nil {
		logger.Error().Emitf("Redis get failed: %v", err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}

	logger.Info().String("value", value).Emit("Redis trace complete")
	c.JSON(http.StatusOK, TraceRedisResponse{
		Status: "redis_trace_complete",
		Value:  value,
	})
}

func demoTraceFullHandler(c *gin.Context) {
	logger := sentry.NewLogger(c.Request.Context())
	logger.Info().Emit("Starting full trace")

	var req TraceFullRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	message := req.Message
	if message == "" {
		message = "Full trace test"
	}

	logger.Info().String("message", message).Emit("Processing full trace")

	// DB operation
	var itemID int
	err := db.QueryRow(
		"INSERT INTO demo_items (service_name, message) VALUES ($1, $2) RETURNING id",
		serviceName, message,
	).Scan(&itemID)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}

	// Redis operations
	ctx := c.Request.Context()
	redisClient.Set(ctx, fmt.Sprintf("demo:%s:last-log", serviceName), message, 0)
	redisClient.Incr(ctx, fmt.Sprintf("demo:%s:counter", serviceName))
	redisClient.Set(ctx, "demo:shared:heartbeat", serviceName, 0)

	logger.Info().Int("item_id", itemID).Emit("Full trace complete")
	c.JSON(http.StatusOK, TraceFullResponse{
		Status:     "full_trace_complete",
		Operations: []string{"log", "db_insert", "redis_write", "heartbeat"},
		ItemID:     itemID,
	})
}

func demoMetricHandler(c *gin.Context) {
	sentry.CaptureMessage("api.request:demo/metric")
	var req MetricRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	sentry.ConfigureScope(func(scope *sentry.Scope) {
		scope.SetTag("metric_name", req.Name)
		scope.SetTag("metric_value", fmt.Sprintf("%f", req.Value))
		for key, value := range req.Tags {
			scope.SetTag(fmt.Sprintf("metric_tag_%s", key), fmt.Sprintf("%v", value))
		}
	})

	fmt.Printf("Metric emitted: %s=%f\n", req.Name, req.Value)

	c.JSON(http.StatusOK, gin.H{"status": "metric_emitted"})
}

func getItemsHandler(c *gin.Context) {
	sentry.CaptureMessage("api.request:demo/db/items")
	rows, err := db.Query("SELECT id, service_name, message, created_at FROM demo_items ORDER BY created_at DESC LIMIT 100")
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}
	defer rows.Close()

	var items []ItemResponse
	for rows.Next() {
		var item ItemResponse
		var createdAt sql.NullTime
		err := rows.Scan(&item.ID, &item.ServiceName, &item.Message, &createdAt)
		if err != nil {
			continue
		}
		if createdAt.Valid {
			timeStr := createdAt.Time.Format(time.RFC3339)
			item.CreatedAt = &timeStr
		}
		items = append(items, item)
	}

	c.JSON(http.StatusOK, ItemsResponse{Items: items})
}

func createItemHandler(c *gin.Context) {
	sentry.CaptureMessage("api.request:demo/db/items")
	var req CreateItemRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	var item ItemResponse
	var createdAt sql.NullTime
	err := db.QueryRow(
		"INSERT INTO demo_items (service_name, message) VALUES ($1, $2) RETURNING id, service_name, message, created_at",
		serviceName, req.Message,
	).Scan(&item.ID, &item.ServiceName, &item.Message, &createdAt)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}

	if createdAt.Valid {
		timeStr := createdAt.Time.Format(time.RFC3339)
		item.CreatedAt = &timeStr
	}

	c.JSON(http.StatusCreated, item)
}
