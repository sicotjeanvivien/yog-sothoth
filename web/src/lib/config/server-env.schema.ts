/**
 * Schema and parser for the server-only environment variables.
 *
 * This module is intentionally free of the `server-only` directive so
 * it can be imported from the vitest test suite (which runs in plain
 * Node without the Next.js bundler boundary). The directive lives in
 * the sibling `server-env.ts` module, which re-exports `getServerEnv`
 * for production code paths.
 */

import * as z from "zod";

// ─────────────────────────────────────────────────────────────────────
// Schema
// ─────────────────────────────────────────────────────────────────────

/**
 * Schema describing every server-only environment variable the Next.js
 * server consumes. Keep this list narrow: only what is actually read
 * by server code lives here, so we can fail fast on misconfiguration.
 */
const ServerEnvSchema = z.object({
  /**
   * Base URL of the yog-api Rust service. Must be a valid URL and
   * must not end with a trailing slash — `url + "/api/pools"` is the
   * concatenation pattern used downstream.
   */
  YOG_API_BASE_URL: z
    .url({ message: "YOG_API_BASE_URL must be a valid URL" })
    .refine((u) => !u.endsWith("/"), {
      message: "YOG_API_BASE_URL must not end with a trailing slash",
    }),

  /**
   * Timeout (in milliseconds) for BFF → yog-api calls. Optional;
   * defaults to 5000ms. Coerced from a string because environment
   * variables are always strings.
   */
  YOG_API_TIMEOUT_MS: z.coerce
    .number()
    .int({ message: "YOG_API_TIMEOUT_MS must be an integer" })
    .positive({ message: "YOG_API_TIMEOUT_MS must be positive" })
    .default(5000),
});

/**
 * Inferred type of the parsed server env. Consumers receive this shape;
 * the raw `process.env` is never exposed.
 */
export type ServerEnv = z.infer<typeof ServerEnvSchema>;

// ─────────────────────────────────────────────────────────────────────
// Parsing
// ─────────────────────────────────────────────────────────────────────

/**
 * Parse the given record against the schema and return the validated
 * env, or throw a formatted error listing every failure at once.
 */
export function parseServerEnv(source: Record<string, string | undefined>): ServerEnv {
  const result = ServerEnvSchema.safeParse(source);

  if (!result.success) {
    const issues = result.error.issues
      .map((issue) => {
        const path = issue.path.join(".") || "(root)";
        return `  - ${path}: ${issue.message}`;
      })
      .join("\n");

    // Throw a single error that lists every failure — easier to fix
    // a misconfigured deployment in one round trip than chasing them
    // one at a time.
    throw new Error(
      `Invalid server-only environment configuration:\n${issues}\n` +
        `See web/.env.example for the expected shape.`,
    );
  }

  return result.data;
}

// ─────────────────────────────────────────────────────────────────────
// Cached singleton accessor
// ─────────────────────────────────────────────────────────────────────

let cached: ServerEnv | null = null;

/**
 * Return the validated server env. The first call performs the parse
 * (and may throw); subsequent calls return the cached value.
 *
 * Caching is intentional: the env is process-wide and immutable for
 * the lifetime of the server, so reparsing on every call would be
 * wasted work.
 */
export function loadServerEnv(): ServerEnv {
  if (cached === null) {
    cached = parseServerEnv(process.env);
  }
  return cached;
}

/**
 * Reset the cached singleton. Used by tests only — production code
 * should never call this.
 *
 * @internal
 */
export function __resetServerEnv(): void {
  cached = null;
}