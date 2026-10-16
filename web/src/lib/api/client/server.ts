/**
 * The server-side yog-api client, over `YOG_API_INTERNAL_URL`. The browser's goes through
 * `client/browser.ts` and the public gateway.
 */

import * as z from "zod";

import { apiGetWithUrl, buildUrl, type QueryValue } from "./client-core";
import { loadServerEnv as getServerEnv } from "../../config/server-env.schema";

/** @throws ApiClientError on a timeout, a network failure, a non-2xx or a validation failure. */
export async function apiGet<T>(
  path: string,
  query: Record<string, QueryValue | undefined>,
  schema: z.ZodType<T>,
): Promise<T> {
  const { YOG_API_INTERNAL_URL, YOG_API_TIMEOUT_MS } = getServerEnv();
  const url = buildUrl(YOG_API_INTERNAL_URL, path, query);
  return apiGetWithUrl(url, schema, YOG_API_TIMEOUT_MS);
}