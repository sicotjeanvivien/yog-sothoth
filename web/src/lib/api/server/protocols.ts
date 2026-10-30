/**
 * The catalogue and the measures of the protocols, on the server, at the root or under one
 * protocol by `Scope`. ⚠️ Each is wrapped in React's `cache`: several blocks of one page read
 * the same response, fetched once per render.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { cache } from "react";
import type { z } from "zod";

import type { Period } from "@/lib/overview/period";

import { apiGet } from "../client/server";
import {
  zGetLiquidityTurnoverTotalsResponse,
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
  zGetProtocolTvlSeriesQuery,
  zGetSwapsSeriesResponse,
  zGetSwapTotalsResponse,
  zGetTvlResponse,
  zGetTvlSeriesQuery,
  zGetTvlSeriesResponse,
  zGetTvlTotalsResponse,
  zGetWindowsResponse,
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

/** Every window: its bounds, whether it is open, and whether it runs late. */
export const fetchWindows = cache(() => apiGet("/api/windows", {}, zGetWindowsResponse));

export const fetchSwapTotals = cache((scope: Scope, window: WindowResponse) =>
  apiGet(scopedPath(scope, "swaps/totals"), { window }, zGetSwapTotalsResponse),
);

export const fetchSwapsSeries = cache((scope: Scope, window: Period) =>
  apiGet(
    scopedPath(scope, "swaps/series"),
    { window },
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

/** The windows the contract serves the TVL series over. */
type TvlSeriesWindow = NonNullable<z.infer<typeof zGetTvlSeriesQuery>["window"]>;

/** Whether the contract serves the TVL series of `scope` over `window`. */
export function servesTvlSeries(scope: Scope, window: Period): window is TvlSeriesWindow {
  const query = scope === "all" ? zGetTvlSeriesQuery : zGetProtocolTvlSeriesQuery;
  return query.shape.window.unwrap().safeParse(window).success;
}

export const fetchTvlSeries = cache((scope: Scope, window: TvlSeriesWindow) =>
  apiGet(scopedPath(scope, "tvl/series"), { window }, zGetTvlSeriesResponse),
);

/** The window's volume over the last TVL reading. */
export const fetchLiquidityTurnoverTotals = cache((scope: Scope, window: WindowResponse) =>
  apiGet(
    scopedPath(scope, "liquidity-turnover/totals"),
    { window },
    zGetLiquidityTurnoverTotalsResponse,
  ),
);

/** The last TVL reading, the one a window before it, and the change without the pools discovered. */
export const fetchTvlTotals = cache((scope: Scope, window: Period) =>
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
    { window },
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

/** The pools the protocol saw born, by bar. */
export const fetchPoolCreationsSeries = cache((protocol: ProtocolResponse, window: Period) =>
  apiGet(
    `/api/protocols/${protocol}/pool-creations/series`,
    { window },
    zGetProtocolPoolCreationsSeriesResponse,
  ),
);

/** The protocol's volume by bar, split by its rotation runs. */
export const fetchRotationRunsSeries = cache((protocol: ProtocolResponse, window: Period) =>
  apiGet(
    `/api/protocols/${protocol}/rotation-runs/series`,
    { window },
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
