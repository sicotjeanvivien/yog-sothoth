/**
 * `GET /api/swaps/by-token` on the server: the first page, by volume.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { cache } from "react";

import { assertInBounds } from "../client/client-core";
import { apiGet } from "../client/server";
import {
  zListSwapsByTokenQuery,
  zListSwapsByTokenResponse,
  type WindowResponse,
} from "../schema/zod.gen";

/** @throws RangeError if `limit` is outside the contract's bounds. */
export const fetchTopTokens = cache((window: WindowResponse, limit: number) => {
  assertInBounds("limit", limit, zListSwapsByTokenQuery.shape.limit.unwrap());
  return apiGet(
    "/api/swaps/by-token",
    { window, sort: "volume", limit },
    zListSwapsByTokenResponse,
  );
});
