/**
 * The catalogue and the measures of the protocols, on the server, at the root or under one
 * protocol by `Scope`. ⚠️ Each is wrapped in React's `cache`: several blocks of one page read
 * the same response, fetched once per render.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { cache } from "react";

import { SERIES_INTERVAL, type FrozenPeriod, type Period } from "@/lib/overview/period";

import { apiGet } from "../client/server";
import {
  zGetPoolCreationTotalsResponse,
  zGetProtocolLiquidityMovesSeriesResponse,
  zGetProtocolLiquidityMoveTotalsResponse,
  zGetProtocolPoolCreationsSeriesResponse,
  zGetProtocolPositionChangeTotalsResponse,
  zGetProtocolPositionFeeClaimTotalsResponse,
  zGetProtocolResponse,
  zGetProtocolRewardClaimTotalsResponse,
  zGetProtocolRotationRunsSeriesResponse,
  zGetProtocolRotationRunTotalsResponse,
  zGetSwapsSeriesResponse,
  zGetSwapTotalsResponse,
  zGetTvlResponse,
  zGetTvlSeriesResponse,
  zGetTvlTotalsResponse,
  zListProtocolLiquidityMovesResponse,
  zListProtocolRotationRunsResponse,
  zListProtocolsResponse,
  type LiquidityMoveKindResponse,
  type ProtocolResponse,
  type WindowResponse,
} from "../schema/zod.gen";
import { scopedPath, type Scope } from "./scope";

/** The catalogue's widest page: every protocol of the domain. */
export const fetchProtocols = cache(() =>
  apiGet("/api/protocols", { limit: 200 }, zListProtocolsResponse),
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

export const fetchSwapsSeries = cache((scope: Scope, window: Period) =>
  apiGet(
    scopedPath(scope, "swaps/series"),
    { window, interval: SERIES_INTERVAL[window] },
    zGetSwapsSeriesResponse,
  ),
);

export const fetchPoolCreationTotals = cache((scope: Scope, window: WindowResponse) =>
  apiGet(scopedPath(scope, "pool-creations/totals"), { window }, zGetPoolCreationTotalsResponse),
);

/** The last TVL reading of each protocol. */
export const fetchTvl = cache((scope: Scope) =>
  apiGet(scopedPath(scope, "tvl"), {}, zGetTvlResponse),
);

export const fetchTvlSeries = cache((scope: Scope, window: FrozenPeriod) =>
  apiGet(scopedPath(scope, "tvl/series"), { window, interval: "1h" }, zGetTvlSeriesResponse),
);

/** The TVL a window before its last reading, and the change without the pools discovered. */
export const fetchTvlTotals = cache((scope: Scope, window: FrozenPeriod) =>
  apiGet(scopedPath(scope, "tvl/totals"), { window }, zGetTvlTotalsResponse),
);

/** No row while the protocol's liquidity does not cover the window whole. */
export const fetchLiquidityMoveTotals = cache(
  (protocol: ProtocolResponse, window: Period) =>
    apiGet(
      `/api/protocols/${protocol}/liquidity-moves/totals`,
      { window },
      zGetProtocolLiquidityMoveTotalsResponse,
    ),
);

/** The fees the protocol's positions claimed over the window, valued. */
export const fetchPositionFeeClaimTotals = cache(
  (protocol: ProtocolResponse, window: Period) =>
    apiGet(
      `/api/protocols/${protocol}/position-fee-claims/totals`,
      { window },
      zGetProtocolPositionFeeClaimTotalsResponse,
    ),
);

/** The rewards the protocol's positions claimed over the window, valued. */
export const fetchRewardClaimTotals = cache((protocol: ProtocolResponse, window: Period) =>
  apiGet(
    `/api/protocols/${protocol}/reward-claims/totals`,
    { window },
    zGetProtocolRewardClaimTotalsResponse,
  ),
);

/** The protocol's liquidity flows, on the bars of `fetchSwapsSeries`. */
export const fetchLiquiditySeries = cache((protocol: ProtocolResponse, window: Period) =>
  apiGet(
    `/api/protocols/${protocol}/liquidity-moves/series`,
    { window, interval: SERIES_INTERVAL[window] },
    zGetProtocolLiquidityMovesSeriesResponse,
  ),
);

/** The protocol's largest valued liquidity moves over the window. */
export const fetchLargestLiquidityMoves = cache(
  (protocol: ProtocolResponse, window: Period, limit: number, kind?: LiquidityMoveKindResponse) =>
    apiGet(
      `/api/protocols/${protocol}/liquidity-moves`,
      { window, sort: "amount", limit: String(limit), ...(kind === undefined ? {} : { kind }) },
      zListProtocolLiquidityMovesResponse,
    ),
);

/** The positions the protocol opened, locked, split and closed over the window. */
export const fetchPositionChangeTotals = cache(
  (protocol: ProtocolResponse, window: Period) =>
    apiGet(
      `/api/protocols/${protocol}/position-changes/totals`,
      { window },
      zGetProtocolPositionChangeTotalsResponse,
    ),
);

/** The pools the protocol saw born, hour by hour up to 7 days, by 24-hour slices beyond. */
export const fetchPoolCreationsSeries = cache((protocol: ProtocolResponse, window: Period) =>
  apiGet(
    `/api/protocols/${protocol}/pool-creations/series`,
    { window, interval: SERIES_INTERVAL[window] },
    zGetProtocolPoolCreationsSeriesResponse,
  ),
);

/** The protocol's volume by bar, split by its rotation runs. */
export const fetchRotationRunsSeries = cache((protocol: ProtocolResponse, window: Period) =>
  apiGet(
    `/api/protocols/${protocol}/rotation-runs/series`,
    { window, interval: SERIES_INTERVAL[window] },
    zGetProtocolRotationRunsSeriesResponse,
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
