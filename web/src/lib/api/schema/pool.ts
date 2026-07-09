import * as z from "zod";
import { BigDecimal, FeePercent, Rfc3339 } from "./shared";
import { SeveritySchema } from "./signal";
import { TokenSchema } from "./token";

// ─────────────────────────────────────────────────────────────────────
// PoolSignalResponse — mirrors `api::http::dto::response::PoolSignalResponse`
// ─────────────────────────────────────────────────────────────────────

/**
 * One entry of a pool's recent-signals list (`signals24h`), the
 * pools-list signal indicator. Deliberately slimmer than the feed's
 * `SignalSchema`: severity, kind and recency only — the full signal
 * lives on the pool's Alerts tab.
 */
export const PoolSignalSchema = z.object({
  severity: SeveritySchema,
  detector: z.string().min(1),
  triggeredAt: Rfc3339,
});

export type PoolSignal = z.infer<typeof PoolSignalSchema>;

// ─────────────────────────────────────────────────────────────────────
// PoolResponse — mirrors `api::http::dto::response::PoolResponse`
// ─────────────────────────────────────────────────────────────────────

/**
 * Wire shape of a pool as exposed by `GET /api/pools` and
 * `GET /api/pools/{address}`.
 *
 * Rust side (api/src/http/dto/response/pool.rs):
 *
 * ```rust
 * #[serde(rename_all = "camelCase")]
 * pub struct PoolResponse {
 *     pool_address: String,
 *     protocol: String,
 *     token_a: EmbeddedTokenResponse,
 *     token_b: EmbeddedTokenResponse,
 *     fee_bps: Option<Decimal>,
 *     protocol_fee_percent: Option<u8>,
 *     partner_fee_percent: Option<u8>,
 *     referral_fee_percent: Option<u8>,
 *     tvl_usd: Option<Decimal>,
 *     volume_24h_usd: Option<Decimal>,
 *     fees_24h_usd: Option<Decimal>,
 *     protocol_fees_24h_usd: Option<Decimal>,
 *     lp_fees_24h_usd: Option<Decimal>,
 *     effective_fee_bps: Option<Decimal>,
 *     signals_24h: Vec<PoolSignalResponse>,
 *     first_seen_at: DateTime<Utc>,
 *     last_seen_at: DateTime<Utc>,
 * }
 * ```
 *
 * `feeBps` is the pool's base trading fee in basis points (its genesis
 * fee tier), null until the `InitializePool` event has been indexed.
 *
 * `protocolFeePercent` / `partnerFeePercent` / `referralFeePercent` are the
 * *configured* split of the trading fee (whole percents, 0..=100), read from
 * the pool account. All three are null together until yog-context resolves
 * the account. This is the configured split, distinct from the *realized*
 * `protocolFees24hUsd` below.
 *
 * The `*Fees24hUsd` block is the *realized* fee over the last 24h, valued
 * at trade-time prices like `volume24hUsd` (same null rules): total
 * (`fees24hUsd`), Meteora's cut (`protocolFees24hUsd`), the LP cut
 * (`lpFees24hUsd` = total − protocol). `effectiveFeeBps` is the realized
 * rate `fees / volume * 10000` — null when volume is absent or zero.
 *
 * Naming is camelCase end-to-end (Rust `rename_all = "camelCase"`),
 * so the schema mirrors that. USD-denominated values arrive as
 * strings to preserve the full `BigDecimal` precision the SQL
 * computation produces — JS `number` would lose the trailing digits
 * the moment they're parsed.
 *
 * `tvlUsd` is null when TVL cannot be computed for the pool (no
 * current state yet, or one of the two token prices is unknown).
 *
 * `volume24hUsd` is null when no priced swap happened in the last
 * 24 hours. A partial volume (some swaps priced, some not) is
 * returned as a non-null sum of priced swaps — see the API's
 * `PoolAnalytics` doc comment for the full rationale.
 */
export const PoolSchema = z.object({
  poolAddress: z.string().min(1),
  protocol: z.string().min(1),
  tokenA: TokenSchema,
  tokenB: TokenSchema,
  feeBps: BigDecimal.nullable(),
  // Fee-split percents are u8 (0..=100) on the wire — JSON numbers, not
  // BigDecimal strings. Resolved as a unit from the pool account, so all
  // three are null together until yog-context resolves it.
  protocolFeePercent: FeePercent.nullable(),
  partnerFeePercent: FeePercent.nullable(),
  referralFeePercent: FeePercent.nullable(),
  tvlUsd: BigDecimal.nullable(),
  volume24hUsd: BigDecimal.nullable(),
  fees24hUsd: BigDecimal.nullable(),
  protocolFees24hUsd: BigDecimal.nullable(),
  lpFees24hUsd: BigDecimal.nullable(),
  effectiveFeeBps: BigDecimal.nullable(),
  // Signals emitted by the pool over the last 24h, newest first,
  // per-pool capped server-side. Empty when the pool was quiet.
  signals24h: z.array(PoolSignalSchema),
  firstSeenAt: Rfc3339,
  lastSeenAt: Rfc3339,
});

export type PoolResponse = z.infer<typeof PoolSchema>;