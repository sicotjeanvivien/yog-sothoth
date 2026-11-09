/**
 * The server-only environment, checked at start-up by `instrumentation.ts`. ⚠️ No `server-only`
 * directive here, so vitest can import it: the API client that reads it carries it.
 */

import * as z from "zod";

/** Only what server code reads, so a misconfiguration fails fast. */
const ServerEnvSchema = z.object({
  /**
   * yog-api's URL for server-side code. It differs from the browser's only in the dev compose,
   * where `localhost` is the web container: `http://yog-api:5000` there.
   */
  YOG_API_URL: z
    .url({ message: "YOG_API_URL must be a valid URL" })
    .refine((u) => !u.endsWith("/"), {
      message: "YOG_API_URL must not end with a trailing slash",
    }),

  /** The dashboard's API key, sent on every server-side call; never prefixed `NEXT_PUBLIC_`. */
  YOG_API_KEY: z
    .string({ message: "YOG_API_KEY is required: create it with `yog-migrate create-api-key`" })
    .regex(/^yog_[0-9a-f]{8}_[0-9a-f]{64}$/, {
      message: "YOG_API_KEY must read yog_<8 hex>_<64 hex>",
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