/**
 * The server-side yog-api client, over `YOG_API_URL` with the dashboard's key. The browser's goes
 * through `client/browser.ts`, without a key.
 */

import "server-only";

import * as z from "zod";

import { apiGetWithUrl, apiPostWithUrl, buildUrl, type QueryValue } from "./client-core";
import { loadServerEnv as getServerEnv } from "../../config/server-env.schema";

/** @throws ApiClientError on a timeout, a network failure, a non-2xx or a validation failure. */
export async function apiGet<T>(
  path: string,
  query: Record<string, QueryValue | undefined>,
  schema: z.ZodType<T>,
): Promise<T> {
  const { YOG_API_URL, YOG_API_KEY, YOG_API_TIMEOUT_MS } = getServerEnv();
  const url = buildUrl(YOG_API_URL, path, query);
  return apiGetWithUrl(url, schema, YOG_API_TIMEOUT_MS, bearer(YOG_API_KEY));
}

/** @throws ApiClientError on a timeout, a network failure, a non-2xx or a validation failure. */
export async function apiPost<T>(path: string, schema: z.ZodType<T>): Promise<T> {
  const { YOG_API_URL, YOG_API_KEY, YOG_API_TIMEOUT_MS } = getServerEnv();
  const url = buildUrl(YOG_API_URL, path, {});
  return apiPostWithUrl(url, schema, YOG_API_TIMEOUT_MS, bearer(YOG_API_KEY));
}

function bearer(key: string): Record<string, string> {
  return { Authorization: `Bearer ${key}` };
}
