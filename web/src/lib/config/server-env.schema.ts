/**
 * The server-only environment. ⚠️ No `server-only` directive here, so vitest can import it:
 * the directive lives in `server-env.ts`, which re-exports for production code.
 */

import * as z from "zod";

/** Only what server code reads, so a misconfiguration fails fast. */
const ServerEnvSchema = z.object({
  /**
   * yog-api's internal URL, for server-side code; never sent to the browser, which uses the
   * public gateway. `http://yog-api:5000` in Docker, `http://127.0.0.1:5000` outside.
   */
  YOG_API_INTERNAL_URL: z
    .url({ message: "YOG_API_INTERNAL_URL must be a valid URL" })
    .refine((u) => !u.endsWith("/"), {
      message: "YOG_API_INTERNAL_URL must not end with a trailing slash",
    }),

  /** Timeout of server-side yog-api calls, in ms; 5000 by default. */
  YOG_API_TIMEOUT_MS: z.coerce
    .number()
    .int({ message: "YOG_API_TIMEOUT_MS must be an integer" })
    .positive({ message: "YOG_API_TIMEOUT_MS must be positive" })
    .default(5000),
});

export type ServerEnv = z.infer<typeof ServerEnvSchema>;

/** The validated env, or one error listing every failure at once. */
export function parseServerEnv(source: Record<string, string | undefined>): ServerEnv {
  const result = ServerEnvSchema.safeParse(source);

  if (!result.success) {
    const issues = result.error.issues
      .map((issue) => {
        const path = issue.path.join(".") || "(root)";
        return `  - ${path}: ${issue.message}`;
      })
      .join("\n");

    throw new Error(
      `Invalid server-only environment configuration:\n${issues}\n` +
      `See web/.env.example for the expected shape.`,
    );
  }

  return result.data;
}

let cached: ServerEnv | null = null;

/** The validated server env, parsed on the first call (which may throw) and then cached. */
export function loadServerEnv(): ServerEnv {
  if (cached === null) {
    cached = parseServerEnv(process.env);
  }
  return cached;
}

/**
 * Resets the cached env, for tests only.
 * @internal
 */
export function __resetServerEnv(): void {
  cached = null;
}