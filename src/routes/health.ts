import { Hono } from "hono";
import { cache } from "../services/cache.js";
import { scheduler } from "../services/scheduler.js";

const health = new Hono();

health.get("/", (c) => {
  const cacheStats = cache.stats();

  return c.json({
    status: "ok",
    timestamp: new Date().toISOString(),
    uptime: process.uptime(),
    scheduler: {
      running: scheduler.running(),
    },
    cache: {
      size: cacheStats.size,
      keys: cacheStats.keys,
    },
  });
});

export { health };
