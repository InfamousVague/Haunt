import { Hono } from "hono";
import { health } from "./health.js";
import { crypto } from "./crypto.js";
import { market } from "./market.js";

const api = new Hono();

// Mount routes
api.route("/health", health);
api.route("/crypto", crypto);
api.route("/market", market);

export { api };
