import "dotenv/config";
import { serve } from "@hono/node-server";
import type { Server } from "http";
import { app } from "./app.js";
import { initWebSocket, closeWebSocket } from "./websocket/index.js";
import { scheduler } from "./services/scheduler.js";
import { cache } from "./services/cache.js";
import { logger } from "./utils/logger.js";

const PORT = parseInt(process.env.PORT || "3001", 10);
const HOST = process.env.HOST || "0.0.0.0";
let isShuttingDown = false;

// Create HTTP server
const server = serve({
  fetch: app.fetch,
  port: PORT,
  hostname: HOST,
});

// Initialize WebSocket - cast to http.Server for ws compatibility
initWebSocket(server as unknown as Server);

// Start the scheduler
scheduler.start();

logger.info(`Haunt API server running at http://${HOST}:${PORT}`);
logger.info(`WebSocket server running at ws://${HOST}:${PORT}/ws`);
logger.info(`API docs available at http://${HOST}:${PORT}/docs`);

// Graceful shutdown
async function shutdown(signal: string) {
  if (isShuttingDown) {
    logger.info("Shutdown already in progress...");
    return;
  }
  isShuttingDown = true;

  logger.info(`\nReceived ${signal}, shutting down gracefully...`);

  // Stop accepting new connections
  scheduler.stop();
  logger.info("Scheduler stopped");

  // Close WebSocket connections
  await closeWebSocket();
  logger.info("WebSocket connections closed");

  // Clear cache
  cache.destroy();
  logger.info("Cache cleared");

  // Close HTTP server
  server.close(() => {
    logger.info("HTTP server closed");
    logger.info("Goodbye!");
    process.exit(0);
  });

  // Force exit after 5 seconds
  setTimeout(() => {
    logger.error("Forced shutdown after timeout");
    process.exit(1);
  }, 5_000);
}

// Handle termination signals
process.on("SIGTERM", () => shutdown("SIGTERM"));
process.on("SIGINT", () => shutdown("SIGINT"));

// Handle stdin for interactive quit (press 'q' or 'Q')
if (process.stdin.isTTY) {
  process.stdin.setRawMode(true);
  process.stdin.resume();
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (key: string) => {
    if (key === "q" || key === "Q" || key === "\u0003") {
      // q, Q, or Ctrl+C
      shutdown("user request");
    }
  });
  logger.info("Press 'q' to quit");
}
