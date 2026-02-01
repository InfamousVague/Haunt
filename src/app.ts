import { Hono } from "hono";
import { cors } from "hono/cors";
import { logger as honoLogger } from "hono/logger";
import { swaggerUI } from "@hono/swagger-ui";
import { api } from "./routes/index.js";
import { errorHandler } from "./utils/errors.js";
import { logger } from "./utils/logger.js";

// Create the Hono app
const app = new Hono();

// Middleware
app.use("*", cors());
app.use("*", honoLogger());

// Error handling
app.onError(errorHandler);

// Swagger UI
app.get("/docs", swaggerUI({ url: "/openapi.json" }));

// OpenAPI spec
app.get("/openapi.json", (c) => {
  return c.json({
    openapi: "3.0.0",
    info: {
      title: "Haunt API",
      version: "0.1.0",
      description: "Cryptocurrency data API with real-time WebSocket updates",
    },
    servers: [
      {
        url: "http://localhost:3001",
        description: "Development server",
      },
    ],
    paths: {
      "/api/health": {
        get: {
          summary: "Health check",
          tags: ["Health"],
          responses: {
            "200": {
              description: "Server is healthy",
              content: {
                "application/json": {
                  schema: {
                    type: "object",
                    properties: {
                      status: { type: "string" },
                      timestamp: { type: "string" },
                      uptime: { type: "number" },
                    },
                  },
                },
              },
            },
          },
        },
      },
      "/api/crypto/listings": {
        get: {
          summary: "Get cryptocurrency listings",
          tags: ["Crypto"],
          parameters: [
            {
              name: "start",
              in: "query",
              schema: { type: "integer", default: 1 },
            },
            {
              name: "limit",
              in: "query",
              schema: { type: "integer", default: 100, maximum: 500 },
            },
            {
              name: "sort",
              in: "query",
              schema: { type: "string", default: "market_cap" },
            },
            {
              name: "sort_dir",
              in: "query",
              schema: { type: "string", enum: ["asc", "desc"], default: "desc" },
            },
          ],
          responses: {
            "200": {
              description: "List of cryptocurrencies",
            },
          },
        },
      },
      "/api/crypto/search": {
        get: {
          summary: "Search cryptocurrencies",
          tags: ["Crypto"],
          parameters: [
            {
              name: "q",
              in: "query",
              required: true,
              schema: { type: "string" },
            },
            {
              name: "limit",
              in: "query",
              schema: { type: "integer", default: 20 },
            },
          ],
          responses: {
            "200": {
              description: "Search results",
            },
          },
        },
      },
      "/api/crypto/{id}": {
        get: {
          summary: "Get cryptocurrency by ID",
          tags: ["Crypto"],
          parameters: [
            {
              name: "id",
              in: "path",
              required: true,
              schema: { type: "integer" },
            },
          ],
          responses: {
            "200": {
              description: "Cryptocurrency details",
            },
            "404": {
              description: "Cryptocurrency not found",
            },
          },
        },
      },
      "/api/crypto/{id}/quotes": {
        get: {
          summary: "Get latest quotes for cryptocurrency",
          tags: ["Crypto"],
          parameters: [
            {
              name: "id",
              in: "path",
              required: true,
              schema: { type: "integer" },
            },
          ],
          responses: {
            "200": {
              description: "Latest quotes",
            },
            "404": {
              description: "Cryptocurrency not found",
            },
          },
        },
      },
      "/api/market/global": {
        get: {
          summary: "Get global market metrics",
          tags: ["Market"],
          responses: {
            "200": {
              description: "Global market metrics",
            },
          },
        },
      },
      "/api/market/fear-greed": {
        get: {
          summary: "Get Fear & Greed Index",
          tags: ["Market"],
          responses: {
            "200": {
              description: "Fear & Greed Index",
            },
          },
        },
      },
    },
  });
});

// Mount API routes
app.route("/api", api);

// Root route
app.get("/", (c) => {
  return c.json({
    name: "Haunt API",
    version: "0.1.0",
    docs: "/docs",
    health: "/api/health",
  });
});

// 404 handler
app.notFound((c) => {
  return c.json({ error: { code: "NOT_FOUND", message: "Route not found" } }, 404);
});

export { app };
