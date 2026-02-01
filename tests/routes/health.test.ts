import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { app } from "../../src/app.js";

describe("Health Route", () => {
  it("GET /api/health should return health status", async () => {
    const res = await app.request("/api/health");
    expect(res.status).toBe(200);

    const body = await res.json();
    expect(body.status).toBe("ok");
    expect(body.timestamp).toBeDefined();
    expect(typeof body.uptime).toBe("number");
    expect(body.scheduler).toBeDefined();
    expect(body.cache).toBeDefined();
  });

  it("GET /api/health should include cache stats", async () => {
    const res = await app.request("/api/health");
    const body = await res.json();

    expect(typeof body.cache.size).toBe("number");
    expect(Array.isArray(body.cache.keys)).toBe(true);
  });

  it("GET /api/health should include scheduler status", async () => {
    const res = await app.request("/api/health");
    const body = await res.json();

    expect(typeof body.scheduler.running).toBe("boolean");
  });
});

describe("Root Route", () => {
  it("GET / should return API info", async () => {
    const res = await app.request("/");
    expect(res.status).toBe(200);

    const body = await res.json();
    expect(body.name).toBe("Haunt API");
    expect(body.version).toBe("0.1.0");
    expect(body.docs).toBe("/docs");
    expect(body.health).toBe("/api/health");
  });
});

describe("404 Handler", () => {
  it("should return 404 for unknown routes", async () => {
    const res = await app.request("/unknown/route");
    expect(res.status).toBe(404);

    const body = await res.json();
    expect(body.error.code).toBe("NOT_FOUND");
    expect(body.error.message).toBe("Route not found");
  });
});

describe("OpenAPI", () => {
  it("GET /openapi.json should return OpenAPI spec", async () => {
    const res = await app.request("/openapi.json");
    expect(res.status).toBe(200);

    const body = await res.json();
    expect(body.openapi).toBe("3.0.0");
    expect(body.info.title).toBe("Haunt API");
    expect(body.paths).toBeDefined();
  });
});
