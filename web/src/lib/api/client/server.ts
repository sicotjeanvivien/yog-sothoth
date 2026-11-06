/**
 * The server-side yog-api client: the API's address, as any client calls it, with the dashboard's
 * key. ⚠️ `server-only`: the key must never reach a client bundle.
 */

import "server-only";

import * as z from "zod";

import { apiRequestWithUrl, buildUrl, type QueryValue } from "./client-core";
import { loadClientEnv } from "../../config/client-env.schema";
import { loadServerEnv as getServerEnv } from "../../config/server-env.schema";

/** @throws ApiClientError on a timeout, a network failure, a non-2xx or a validation failure. */
export async function apiGet<T>(
  path: string,
  query: Record<string, QueryValue | undefined>,
  schema: z.ZodType<T>,
): Promise<T> {
  return withKey(path, query, schema);
}

/** `body` POSTed to `path`. @throws ApiClientError, as {@link apiGet}. */
export async function apiPost<T>(path: string, body: unknown, schema: z.ZodType<T>): Promise<T> {
  return withKey(path, {}, schema, body);
}

/** One call with the dashboard's key: a `GET`, or a `POST` of `body`. */
function withKey<T>(
  path: string,
  query: Record<string, QueryValue | undefined>,
  schema: z.ZodType<T>,
  body?: unknown,
): Promise<T> {
  const { YOG_API_KEY, YOG_API_TIMEOUT_MS, YOG_API_URL } = getServerEnv();
  const url = buildUrl(YOG_API_URL ?? loadClientEnv().NEXT_PUBLIC_YOG_API_URL, path, query);
  return apiRequestWithUrl(url, schema, YOG_API_TIMEOUT_MS, {
    authorization: `Bearer ${YOG_API_KEY}`,
    body,
  });
}
