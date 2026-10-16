/**
 * The browser-side yog-api client, over the public `NEXT_PUBLIC_YOG_API_URL`, for the fetchers
 * of `lib/api/browser/`. Shares `client-core.ts` with `client/server.ts`.
 */

import * as z from "zod";

import { apiGetWithUrl, buildUrl, type QueryValue } from "./client-core";
import { loadClientEnv as getClientEnv } from "../../config/client-env.schema";

/** @throws ApiClientError on a timeout, a network failure, a non-2xx or a validation failure. */
export async function apiGetBrowser<T>(
  path: string,
  query: Record<string, QueryValue | undefined>,
  schema: z.ZodType<T>,
): Promise<T> {
  const { NEXT_PUBLIC_YOG_API_URL, NEXT_PUBLIC_YOG_API_TIMEOUT_MS } = getClientEnv();
  const url = buildUrl(NEXT_PUBLIC_YOG_API_URL, path, query);
  return apiGetWithUrl(url, schema, NEXT_PUBLIC_YOG_API_TIMEOUT_MS);
}