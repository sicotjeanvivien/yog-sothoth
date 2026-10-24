/**
 * `/api/protocols` and its measures, on the server. ⚠️ Each is wrapped in React's `cache`:
 * several blocks of one page read the same response, fetched once per render.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { cache } from "react";

import { apiGet } from "../client/server";
import {
  zGetProtocolActivityDailyResponse,
  zGetProtocolActivityHourlyResponse,
  zGetProtocolActivityResponse,
  zGetProtocolTvlHourlyResponse,
  zGetProtocolTvlResponse,
  zListLiquidityMovesResponse,
  zListProtocolsResponse,
  type ProtocolResponse,
  type WindowResponse,
} from "../schema/zod.gen";

export const fetchProtocols = cache(() =>
  apiGet("/api/protocols", {}, zListProtocolsResponse),
);

export const fetchProtocolTvl = cache(() =>
  apiGet("/api/protocols/tvl", {}, zGetProtocolTvlResponse),
);

export const fetchProtocolTvlHourly = cache((window: WindowResponse) =>
  apiGet("/api/protocols/tvl/hourly", { window }, zGetProtocolTvlHourlyResponse),
);

export const fetchProtocolActivity = cache((window: WindowResponse) =>
  apiGet("/api/protocols/activity", { window }, zGetProtocolActivityResponse),
);

/** The window that ends where `window` starts. */
export const fetchPreviousProtocolActivity = cache((window: WindowResponse) =>
  apiGet(
    "/api/protocols/activity",
    { window, offset: window },
    zGetProtocolActivityResponse,
  ),
);

/** Hour by hour over 24 h, by 24-hour slices ending at `asOf` beyond. */
export const fetchProtocolActivitySeries = cache((window: WindowResponse) =>
  window === "24h"
    ? apiGet("/api/protocols/activity/hourly", { window }, zGetProtocolActivityHourlyResponse)
    : apiGet("/api/protocols/activity/daily", { window }, zGetProtocolActivityDailyResponse),
);

/** The protocol's largest valued liquidity moves over the window. */
export const fetchLiquidityMoves = cache(
  (protocol: ProtocolResponse, window: WindowResponse, limit: number) =>
    apiGet(
      `/api/protocols/${protocol}/liquidity-moves`,
      { window, limit: String(limit) },
      zListLiquidityMovesResponse,
    ),
);
