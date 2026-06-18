/**
 * Schema for the `GET /api/stats` response.
 *
 * Mirrors the `StatsResponse` DTO emitted by yog-api: protocol-wide
 * aggregate statistics backing the Overview page (but client-agnostic —
 * the endpoint ships raw counters, the UI derives the coverage label).
 *
 * Notes on the fields:
 *   - USD aggregates are `BigDecimal` strings (like `PoolResponse`) to
 *     preserve the full precision the SQL produces; `null` when nothing
 *     is priceable / no activity in the window. The UI formats them.
 *   - `poolsPriced` is the coverage numerator (pools that contributed to
 *     `totalTvlUsd`); `poolsObserved` is the denominator. Both are JSON
 *     numbers (BIGINT counts, well within the JS safe range here).
 *   - `poolsDiscovered24h` counts pools first seen in the last 24h.
 */

import * as z from "zod";

import { BigDecimal } from "./shared";

/** Schema for the full protocol-wide stats payload. */
export const StatsSchema = z.object({
  // Summed current TVL across priceable pools; null when none is priceable.
  totalTvlUsd: BigDecimal.nullable(),
  // How many pools contributed to totalTvlUsd (coverage numerator).
  poolsPriced: z.number().int().nonnegative(),
  // Summed realized volume / trading fee over the last 24h (trade-time valued).
  volume24hUsd: BigDecimal.nullable(),
  fees24hUsd: BigDecimal.nullable(),
  // Every pool ever observed (coverage denominator).
  poolsObserved: z.number().int().nonnegative(),
  // Pools first seen in the last 24h (the discovery pulse).
  poolsDiscovered24h: z.number().int().nonnegative(),
});

/** Validated protocol-wide stats payload. */
export type StatsResponse = z.infer<typeof StatsSchema>;
