import * as z from "zod";
import { BigDecimal, Rfc3339 } from "./shared";

// ─────────────────────────────────────────────────────────────────────
// PoolHistory — mirrors `api::http::dto::response::PoolHistoryBucketResponse`
// ─────────────────────────────────────────────────────────────────────

/**
 * One hourly bucket of a pool's activity history, as returned by
 * `GET /api/pools/{address}/history?days=N`.
 *
 * Rust side (api/src/http/dto/response/pool_history.rs), camelCase:
 *
 * ```rust
 * #[serde(rename_all = "camelCase")]
 * pub struct PoolHistoryBucketResponse {
 *     bucket: DateTime<Utc>,
 *     volume_usd: Option<Decimal>,
 *     fees_usd: Option<Decimal>,
 *     protocol_fees_usd: Option<Decimal>,
 *     lp_fees_usd: Option<Decimal>,
 *     effective_fee_bps: Option<Decimal>,
 *     liquidity_added_usd: Option<Decimal>,
 *     liquidity_removed_usd: Option<Decimal>,
 *     fees_claimed_usd: Option<Decimal>,
 *     rewards_claimed_usd: Option<Decimal>,
 *     swap_count: Option<i64>,
 * }
 * ```
 *
 * USD metrics arrive as precision-safe decimal strings (`BigDecimal`),
 * null when no priced activity of that kind happened in the bucket.
 * `swapCount` is a plain JSON number. The endpoint returns the buckets as
 * an ordered array (oldest → newest), not a paginated page.
 */
export const PoolHistoryBucketSchema = z.object({
  bucket: Rfc3339,
  volumeUsd: BigDecimal.nullable(),
  feesUsd: BigDecimal.nullable(),
  protocolFeesUsd: BigDecimal.nullable(),
  lpFeesUsd: BigDecimal.nullable(),
  effectiveFeeBps: BigDecimal.nullable(),
  liquidityAddedUsd: BigDecimal.nullable(),
  liquidityRemovedUsd: BigDecimal.nullable(),
  feesClaimedUsd: BigDecimal.nullable(),
  rewardsClaimedUsd: BigDecimal.nullable(),
  swapCount: z.number().int().nullable(),
});

export const PoolHistorySchema = z.array(PoolHistoryBucketSchema);

export type PoolHistoryBucketResponse = z.infer<typeof PoolHistoryBucketSchema>;
export type PoolHistoryResponse = z.infer<typeof PoolHistorySchema>;
