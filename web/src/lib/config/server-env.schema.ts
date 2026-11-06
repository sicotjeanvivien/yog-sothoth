/**
 * The server-only environment. ⚠️ No `server-only` directive here, so vitest can import it:
 * the directive lives in `server-env.ts`.
 */

import * as z from "zod";

/** Only what server code reads, so a misconfiguration fails fast. */
const ServerEnvSchema = z.object({
  /**
   * The dashboard's API key, sent by server code as `Authorization: Bearer`; never in the
   * browser. Issued by `yog-migrate create-api-key dashboard`.
   */
  YOG_API_KEY: z.string().regex(/^yog_[0-9a-f]{8}_[0-9a-f]{64}$/, {
    message: "YOG_API_KEY must be an API key (yog-migrate create-api-key)",
  }),

  /**
   * Where server code calls the API, with the key; unset, the browser's `NEXT_PUBLIC_YOG_API_URL`.
   * ⚠️ Set in the dev compose only, where `localhost` is the web container itself.
   */
  YOG_API_URL: z
    .url({ message: "YOG_API_URL must be a valid URL" })
    .refine((u) => !u.endsWith("/"), { message: "YOG_API_URL must not end with a trailing slash" })
    .optional(),

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