/**
 * Browser-side fetch wrapper for `yog-api`.
 *
 * Mirrors `lib/api/client.ts` but reads the public gateway URL
 * (`NEXT_PUBLIC_YOG_API_URL`, exposed to the browser by Next.js) and
 * is therefore safe to import from Client Components.
 *
 * Architecturally, browser-side fetchers live under `lib/api/browser/*`
 * and use this client; server-side fetchers live at `lib/api/*` and
 * use `client.ts`. Both share the runtime-agnostic core
 * (`client-core.ts`) so timeout, error classification, RFC 9457
 * envelope parsing and zod validation behave identically across
 * runtimes.
 */

import * as z from "zod";

import { apiGetWithUrl, buildUrl, type QueryValue } from "./client-core";
import { loadClientEnv as getClientEnv } from "../../config/client-env.schema";

/**
 * Perform a GET request against `yog-api` from the browser side, and
 * validate the response body against the given schema.
 *
 * @throws ApiClientError on any failure (timeout, network, non-2xx, validation).
 */
export async function apiGetBrowser<T>(
  path: string,
  query: Record<string, QueryValue | undefined>,
  schema: z.ZodType<T>,
): Promise<T> {
  const { NEXT_PUBLIC_YOG_API_URL, NEXT_PUBLIC_YOG_API_TIMEOUT_MS } = getClientEnv();
  const url = buildUrl(NEXT_PUBLIC_YOG_API_URL, path, query);
  return apiGetWithUrl(url, schema, NEXT_PUBLIC_YOG_API_TIMEOUT_MS);
}