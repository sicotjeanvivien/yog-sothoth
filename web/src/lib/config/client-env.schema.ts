/**
 * Browser-visible environment variables.
 *
 * Symmetric to `server-env.schema.ts` but for the `NEXT_PUBLIC_*`
 * vars that Next.js inlines into the client bundle. These are the
 * only values the browser is allowed to see — anything secret must
 * stay in the server env schema.
 *
 * Validation runs at load time (first call to `loadClientEnv`)
 * rather than at module import: this avoids module-graph issues
 * when Next.js does its build-time analysis.
 */

import * as z from "zod";

const ClientEnvSchema = z.object({
  /**
   * Public URL of yog-api, used by Client Components to reach the
   * Rust API directly through the gateway. Must not end with a
   * trailing slash so `${url}${path}` stays well-formed.
   *
   * In production: `https://api.yog-scope.xyz`.
   * In local dev: `http://localhost:5000` (or `http://127.0.0.1:5000`).
   */
  NEXT_PUBLIC_YOG_API_URL: z
    .string()
    .url()
    .refine((url) => !url.endsWith("/"), {
      message: "must not end with a trailing slash",
    }),

  /**
   * Timeout in milliseconds for browser-side calls to yog-api. The
   * client aborts and surfaces an `ApiClientError.timeout` after
   * this. Set higher than the server-side default if the public
   * gateway adds noticeable latency.
   */
  NEXT_PUBLIC_YOG_API_TIMEOUT_MS: z.coerce.number().int().positive().default(10_000),
});

export type ClientEnv = z.infer<typeof ClientEnvSchema>;

let cached: ClientEnv | null = null;

/**
 * Validate and return the browser-visible environment.
 *
 * IMPORTANT: only `NEXT_PUBLIC_*` keys are inlined by Next.js into
 * the client bundle. Reading from `process.env` here works at build
 * time on the server and at runtime on the client (Next.js replaces
 * each access with its literal value during the build).
 */
export function loadClientEnv(): ClientEnv {
  if (cached !== null) return cached;

  const parsed = ClientEnvSchema.safeParse({
    NEXT_PUBLIC_YOG_API_URL: process.env['NEXT_PUBLIC_YOG_API_URL'],
    NEXT_PUBLIC_YOG_API_TIMEOUT_MS: process.env['NEXT_PUBLIC_YOG_API_TIMEOUT_MS'],
  });

  if (!parsed.success) {
    const issues = parsed.error.issues
      .map((i) => `  - ${i.path.join(".")}: ${i.message}`)
      .join("\n");
    throw new Error(`Invalid client environment:\n${issues}`);
  }

  cached = parsed.data;
  return cached;
}