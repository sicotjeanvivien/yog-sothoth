/**
 * Browser-side fetcher for `GET /api/network/status`.
 *
 * Mirrors `lib/api/network-status.ts` but reaches yog-api through
 * the public gateway (`NEXT_PUBLIC_YOG_API_URL`). Used by Client
 * Components — most prominently `NetworkStatusPanel`, which polls
 * this endpoint every 10s.
 *
 * The function suffix `Browser` makes the runtime explicit at the
 * point of import: a Server Component importing this would fail
 * at module load (the client env var is not available server-side
 * in the same way the internal one is, and the path is wrong).
 *
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { apiGetBrowser } from "@/lib/api/client/browser";
import {
  NetworkStatusSchema,
  type NetworkStatusResponse,
} from "../schema/network-status";

/** Fetch the current network status snapshot from `yog-api` via the public gateway. */
export async function fetchNetworkStatusBrowser(): Promise<NetworkStatusResponse> {
  return apiGetBrowser("/api/network/status", {}, NetworkStatusSchema);
}