/**
 * The server-side yog-api client: the API's address, as any client calls it, with the dashboard's
 * key. ⚠️ `server-only`: the key must never reach a client bundle.
 */

import "server-only";

import * as z from "zod";

import { apiGetWithUrl, buildUrl, type QueryValue } from "./client-core";
import { loadClientEnv } from "../../config/client-env.schema";
import { loadServerEnv as getServerEnv } from "../../config/server-env.schema";

function apiBase(): string {
  return getServerEnv().YOG_API_URL ?? loadClientEnv().NEXT_PUBLIC_YOG_API_URL;
}

/** @throws ApiClientError on a timeout, a network failure, a non-2xx or a validation failure. */
export async function apiGet<T>(
  path: string,
  query: Record<string, QueryValue | undefined>,
  schema: z.ZodType<T>,
): Promise<T> {
  const { YOG_API_KEY, YOG_API_TIMEOUT_MS } = getServerEnv();
  const url = buildUrl(apiBase(), path, query);
  return apiGetWithUrl(url, schema, YOG_API_TIMEOUT_MS, { authorization: `Bearer ${YOG_API_KEY}` });
}

/** `body` POSTed to `path`, with the dashboard's key. */
export async function apiPost<T>(path: string, body: unknown, schema: z.ZodType<T>): Promise<T> {
  const { YOG_API_KEY, YOG_API_TIMEOUT_MS } = getServerEnv();
  const url = buildUrl(apiBase(), path, {});
  return apiGetWithUrl(url, schema, YOG_API_TIMEOUT_MS, {
    authorization: `Bearer ${YOG_API_KEY}`,
    body,
  });
}
