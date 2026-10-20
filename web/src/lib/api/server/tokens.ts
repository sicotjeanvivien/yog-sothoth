/**
 * `GET /api/tokens/activity` on the server: the first page, by volume.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { cache } from "react";

import { assertInBounds } from "../client/client-core";
import { apiGet } from "../client/server";
import {
  zListTokenActivityQuery,
  zListTokenActivityResponse,
  type WindowResponse,
} from "../schema/zod.gen";

/** @throws RangeError if `limit` is outside the contract's bounds. */
export const fetchTopTokens = cache((window: WindowResponse, limit: number) => {
  assertInBounds("limit", limit, zListTokenActivityQuery.shape.limit.unwrap());
  return apiGet(
    "/api/tokens/activity",
    { window, sort: "volume", limit },
    zListTokenActivityResponse,
  );
});
