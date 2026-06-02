/**
 * Server-side fetch wrapper for `yog-api`.
 *
 * Every higher-level call (`fetchPools`, `fetchNetworkStatus`, etc.)
 * imported from a Server Component (or any other server-side code)
 * routes through `apiGet`. Reads `YOG_API_INTERNAL_URL` from the
 * server env to reach yog-api over the internal network — never
 * exposed to the browser.
 *
 * Responsibilities centralised here:
 *
 *   - Resolving the server-side base URL
 *   - Forwarding to `apiGetWithUrl` from `client-core` (which owns
 *     timeout, error classification, zod validation, etc.)
 *
 * The browser-side counterpart lives in `browser/client.ts` — it
 * speaks to yog-api through `NEXT_PUBLIC_YOG_API_URL` (the public
 * gateway), not through this module.
 */

import * as z from "zod";

import { apiGetWithUrl, buildUrl, type QueryValue } from "./client-core";
import { loadServerEnv as getServerEnv } from "../../config/server-env.schema";

/**
 * Perform a GET request against `yog-api` from the server side, and
 * validate the response body against the given schema.
 *
 * @throws ApiClientError on any failure (timeout, network, non-2xx, validation).
 */
export async function apiGet<T>(
  path: string,
  query: Record<string, QueryValue | undefined>,
  schema: z.ZodType<T>,
): Promise<T> {
  const { YOG_API_INTERNAL_URL, YOG_API_TIMEOUT_MS } = getServerEnv();
  const url = buildUrl(YOG_API_INTERNAL_URL, path, query);
  return apiGetWithUrl(url, schema, YOG_API_TIMEOUT_MS);
}