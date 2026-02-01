type LogLevel = "debug" | "info" | "warn" | "error";

const LOG_COLORS = {
  debug: "\x1b[36m", // cyan
  info: "\x1b[32m", // green
  warn: "\x1b[33m", // yellow
  error: "\x1b[31m", // red
  reset: "\x1b[0m",
};

const isProduction = process.env.NODE_ENV === "production";

function formatTimestamp(): string {
  return new Date().toISOString();
}

function log(level: LogLevel, message: string, data?: unknown): void {
  if (isProduction && level === "debug") return;

  const color = LOG_COLORS[level];
  const reset = LOG_COLORS.reset;
  const timestamp = formatTimestamp();
  const prefix = `${color}[${level.toUpperCase()}]${reset}`;

  if (data !== undefined) {
    console.log(`${prefix} ${timestamp} ${message}`, data);
  } else {
    console.log(`${prefix} ${timestamp} ${message}`);
  }
}

export const logger = {
  debug: (message: string, data?: unknown) => log("debug", message, data),
  info: (message: string, data?: unknown) => log("info", message, data),
  warn: (message: string, data?: unknown) => log("warn", message, data),
  error: (message: string, data?: unknown) => log("error", message, data),

  request: (method: string, path: string, params?: unknown) => {
    log("info", `${method} ${path}`, params);
  },

  response: (path: string, status: number, duration: number) => {
    log("debug", `Response ${path} ${status} (${duration}ms)`);
  },

  ws: (event: string, clientId: string, data?: unknown) => {
    log("debug", `WS [${clientId}] ${event}`, data);
  },

  cache: (action: string, key: string, hit?: boolean) => {
    const hitStr = hit !== undefined ? (hit ? "HIT" : "MISS") : "";
    log("debug", `Cache ${action} ${key} ${hitStr}`);
  },
};
