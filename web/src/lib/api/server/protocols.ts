/**
 * The catalogue and the measures of the protocols, on the server, at the root or under one
 * protocol by `Scope`. ⚠️ Each is wrapped in React's `cache`: several blocks of one page read
 * the same response, fetched once per render.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { cache } from "react";

import { apiGet } from "../client/server";
import {
  zGetPoolCreationTotalsResponse,
  zGetPoolDiscoveryTotalsResponse,
  zGetProtocolLiquidityMovesDailyResponse,
  zGetProtocolLiquidityMovesHourlyResponse,
  zGetProtocolLiquidityMoveTotalsResponse,
  zGetProtocolResponse,
  zGetProtocolRotationRunsDailyResponse,
  zGetProtocolRotationRunsHourlyResponse,
  zGetProtocolRotationRunTotalsResponse,
  zGetSwapsDailyResponse,
  zGetSwapsHourlyResponse,
  zGetSwapTotalsResponse,
  zGetTvlHourlyResponse,
  zGetTvlResponse,
  zListProtocolLiquidityMovesResponse,
  zListProtocolRotationRunsResponse,
  zListProtocolsResponse,
  type ProtocolResponse,
  type WindowResponse,
} from "../schema/zod.gen";
import { scopedPath, type Scope } from "./scope";

/** The catalogue's first page: every protocol of the domain. */
export const fetchProtocols = cache(() =>
  apiGet("/api/protocols", {}, zListProtocolsResponse),
);

export const fetchProtocol = cache((protocol: ProtocolResponse) =>
  apiGet(`/api/protocols/${protocol}`, {}, zGetProtocolResponse),
);

export const fetchSwapTotals = cache((scope: Scope, window: WindowResponse) =>
  apiGet(scopedPath(scope, "swaps/totals"), { window }, zGetSwapTotalsResponse),
);

/** The window that ends where `window` starts. */
export const fetchPreviousSwapTotals = cache((scope: Scope, window: WindowResponse) =>
  apiGet(
    scopedPath(scope, "swaps/totals"),
    { window, offset: window },
    zGetSwapTotalsResponse,
  ),
);

/** Hour by hour over 24 h, by 24-hour slices ending at `asOf` beyond. */
export const fetchSwapsSeries = cache((scope: Scope, window: WindowResponse) =>
  window === "24h"
    ? apiGet(scopedPath(scope, "swaps/hourly"), { window }, zGetSwapsHourlyResponse)
    : apiGet(scopedPath(scope, "swaps/daily"), { window }, zGetSwapsDailyResponse),
);

export const fetchPoolCreationTotals = cache((scope: Scope, window: WindowResponse) =>
  apiGet(scopedPath(scope, "pool-creations/totals"), { window }, zGetPoolCreationTotalsResponse),
);

/** The last TVL reading of each protocol. */
export const fetchTvl = cache((scope: Scope) =>
  apiGet(scopedPath(scope, "tvl"), {}, zGetTvlResponse),
);

export const fetchTvlHourly = cache((scope: Scope, window: WindowResponse) =>
  apiGet(scopedPath(scope, "tvl/hourly"), { window }, zGetTvlHourlyResponse),
);

/** What the pools first seen in the window hold, at the last reading of their part. */
export const fetchPoolDiscoveryTotals = cache((scope: Scope, window: WindowResponse) =>
  apiGet(
    scopedPath(scope, "pool-discoveries/totals"),
    { window },
    zGetPoolDiscoveryTotalsResponse,
  ),
);

/** No row while the protocol's liquidity does not cover the window whole. */
export const fetchLiquidityMoveTotals = cache(
  (protocol: ProtocolResponse, window: WindowResponse) =>
    apiGet(
      `/api/protocols/${protocol}/liquidity-moves/totals`,
      { window },
      zGetProtocolLiquidityMoveTotalsResponse,
    ),
);

/** The protocol's liquidity flows, on the bars of `fetchSwapsSeries`. */
export const fetchLiquiditySeries = cache((protocol: ProtocolResponse, window: WindowResponse) =>
  window === "24h"
    ? apiGet(
        `/api/protocols/${protocol}/liquidity-moves/hourly`,
        { window },
        zGetProtocolLiquidityMovesHourlyResponse,
      )
    : apiGet(
        `/api/protocols/${protocol}/liquidity-moves/daily`,
        { window },
        zGetProtocolLiquidityMovesDailyResponse,
      ),
);

/** The protocol's largest valued liquidity moves over the window. */
export const fetchLargestLiquidityMoves = cache(
  (protocol: ProtocolResponse, window: WindowResponse, limit: number) =>
    apiGet(
      `/api/protocols/${protocol}/liquidity-moves`,
      { window, sort: "amount", limit: String(limit) },
      zListProtocolLiquidityMovesResponse,
    ),
);

/** The protocol's volume in rotation runs, on the bars of `fetchSwapsSeries`. */
export const fetchRotationRunsSeries = cache((protocol: ProtocolResponse, window: WindowResponse) =>
  window === "24h"
    ? apiGet(
        `/api/protocols/${protocol}/rotation-runs/hourly`,
        { window },
        zGetProtocolRotationRunsHourlyResponse,
      )
    : apiGet(
        `/api/protocols/${protocol}/rotation-runs/daily`,
        { window },
        zGetProtocolRotationRunsDailyResponse,
      ),
);

export const fetchRotationRunTotals = cache((protocol: ProtocolResponse, window: WindowResponse) =>
  apiGet(
    `/api/protocols/${protocol}/rotation-runs/totals`,
    { window },
    zGetProtocolRotationRunTotalsResponse,
  ),
);

/** The protocol's rotation runs of largest volume over the window. */
export const fetchLargestRotationRuns = cache(
  (protocol: ProtocolResponse, window: WindowResponse, limit: number) =>
    apiGet(
      `/api/protocols/${protocol}/rotation-runs`,
      { window, sort: "volume", limit: String(limit) },
      zListProtocolRotationRunsResponse,
    ),
);
