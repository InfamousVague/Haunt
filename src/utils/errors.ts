import { HTTPException } from "hono/http-exception";
import type { Context } from "hono";
import { logger } from "./logger.js";

export class APIError extends Error {
  constructor(
    message: string,
    public statusCode: number = 500,
    public code: string = "INTERNAL_ERROR"
  ) {
    super(message);
    this.name = "APIError";
  }
}

export class NotFoundError extends APIError {
  constructor(resource: string) {
    super(`${resource} not found`, 404, "NOT_FOUND");
    this.name = "NotFoundError";
  }
}

export class ValidationError extends APIError {
  constructor(message: string) {
    super(message, 400, "VALIDATION_ERROR");
    this.name = "ValidationError";
  }
}

export class CMCError extends APIError {
  constructor(message: string, statusCode = 502) {
    super(`CoinMarketCap API error: ${message}`, statusCode, "CMC_ERROR");
    this.name = "CMCError";
  }
}

export function errorHandler(err: Error, c: Context) {
  logger.error(`Error: ${err.message}`, {
    name: err.name,
    stack: err.stack,
  });

  if (err instanceof HTTPException) {
    return c.json(
      {
        error: {
          code: "HTTP_ERROR",
          message: err.message,
        },
      },
      err.status
    );
  }

  if (err instanceof APIError) {
    return c.json(
      {
        error: {
          code: err.code,
          message: err.message,
        },
      },
      err.statusCode as 400 | 404 | 500 | 502
    );
  }

  return c.json(
    {
      error: {
        code: "INTERNAL_ERROR",
        message: "An unexpected error occurred",
      },
    },
    500
  );
}
