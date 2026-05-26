import * as Sentry from "@sentry/node";
import { nodeProfilingIntegration } from "@sentry/profiling-node";
import dotenv from "dotenv";

// Load env vars first
dotenv.config();

// Initialize Sentry BEFORE importing express
Sentry.init({
  dsn: "https://532d19b53dfc14aa95cbbbcd59fb3d53@o4511451202125824.ingest.us.sentry.io/4511455218696192",
  environment: process.env.SENTRY_ENVIRONMENT || "development",
  release: process.env.SENTRY_RELEASE || "1.0.0",
  tracesSampleRate: 1.0,
  profilesSampleRate: 1.0,
  enableLogs: true,
  integrations: [
    nodeProfilingIntegration(),
    Sentry.httpIntegration(),
    Sentry.expressIntegration(),
    Sentry.postgresIntegration(),
    Sentry.redisIntegration(),
  ],
});
