/**
 * The browser-visible `NEXT_PUBLIC_*` environment: never a secret. Validated on the first
 * `loadClientEnv`, not at import, which Next.js's build-time analysis would trip on.
 */

import * as z from "zod";

const ClientEnvSchema = z.object({
  /** yog-api's public URL, without a trailing slash, so `${url}${path}` stays well-formed. */
  NEXT_PUBLIC_YOG_API_URL: z
    .string()
    .url()
    .refine((url) => !url.endsWith("/"), {
      message: "must not end with a trailing slash",
    }),

  /** Timeout of browser-side yog-api calls, in ms. */
  NEXT_PUBLIC_YOG_API_TIMEOUT_MS: z.coerce.number().int().positive().default(10_000),
});

export type ClientEnv = z.infer<typeof ClientEnvSchema>;

let cached: ClientEnv | null = null;

/** ⚠️ Literal `process.env.NEXT_PUBLIC_*` reads: Next.js replaces each with its value at build. */
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