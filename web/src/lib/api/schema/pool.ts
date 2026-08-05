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
 *     tvl_usd: Option<Decimal>,
 *     volume_24h_usd: Option<Decimal>,
 *     fees_24h_usd: Option<Decimal>,
 *     protocol_fees_24h_usd: Option<Decimal>,
 *     lp_fees_24h_usd: Option<Decimal>,
 *     effective_fee_bps: Option<Decimal>,
 *     swap_buckets_24h: i64,
 *     swap_buckets_priced_24h: i64,
 *     signals_24h: Vec<PoolSignalResponse>,
 *     first_seen_at: DateTime<Utc>,
 *     last_seen_at: DateTime<Utc>,
 * }
 * ```
 *
 * `feeBps` is the pool's base trading fee in basis points (its genesis
 * fee tier), null until the `InitializePool` event has been indexed.
 *
 * The cp-amm fee properties (the configured fee split, the fee shape) are NOT
 * here: they are protocol-specific and live on `PoolDetailSchema`'s
 * `meteoraDammV2` block, returned only by `GET /api/pools/{address}`.
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
 * `volume24hUsd` is null only when NO hour of the window could be valued at
 * all. A partially covered window is returned as a non-null sum of the hours
 * that could be valued — a sub-total. `swapBucketsPriced24h` /
 * `swapBuckets24h` is the coverage that tells the two apart; without it a
 * 58 %-covered figure reads exactly like a complete one. It applies to
 * `volume24hUsd`, `fees24hUsd` and the two fee splits at once (they share one
 * valuation), but NOT to `effectiveFeeBps`, whose numerator and denominator
 * are lost on the same hours and therefore cancel.
 */
export const PoolSchema = z.object({
  poolAddress: z.string().min(1),
  protocol: z.string().min(1),
  tokenA: TokenSchema,
  tokenB: TokenSchema,
  feeBps: BigDecimal.nullable(),
  tvlUsd: BigDecimal.nullable(),
  volume24hUsd: BigDecimal.nullable(),
  fees24hUsd: BigDecimal.nullable(),
  protocolFees24hUsd: BigDecimal.nullable(),
  lpFees24hUsd: BigDecimal.nullable(),
  effectiveFeeBps: BigDecimal.nullable(),
  // Coverage of the four USD figures above: hours of the window that traded,
  // and how many of them could be valued.
  swapBuckets24h: z.number().int().nonnegative(),
  swapBucketsPriced24h: z.number().int().nonnegative(),
  // Signals emitted by the pool over the last 24h, newest first,
  // per-pool capped server-side. Empty when the pool was quiet.
  signals24h: z.array(PoolSignalSchema),
  firstSeenAt: Rfc3339,
  lastSeenAt: Rfc3339,
});

export type PoolResponse = z.infer<typeof PoolSchema>;

/**
 * Pool properties that only exist for Meteora DAMM v2 (cp-amm).
 *
 * `protocolFeePercent` / `referralFeePercent` are the *configured* split of the
 * trading fee (whole percents, 0..=100), read from the pool account — both null
 * together until yog-context resolves it. Distinct from the *realized*
 * `protocolFees24hUsd` on the pool itself.
 *
 * There is no partner cut: `partnerFeePercent` was served until migration 037,
 * decoded a padding byte of the account, and was always 0.
 *
 * `baseFeeKind` is how the base fee behaves over time — an opaque string
 * ("constant" | "scheduler_linear" | "scheduler_exponential" |
 * "rate_limiter") — and `hasDynamicFee` whether a volatility fee sits on top.
 * Both decoded from the genesis fee config; null until InitializePool is
 * indexed, or if the fee blob failed to decode.
 *
 * The two groups have different writers and either can land first, so a block
 * with only one group filled is a normal state.
 */
export const MeteoraDammV2PropertiesSchema = z.object({
  protocolFeePercent: FeePercent.nullable(),
  referralFeePercent: FeePercent.nullable(),
  baseFeeKind: z.string().nullable(),
  hasDynamicFee: z.boolean().nullable(),
});

/**
 * `GET /api/pools/{address}` — everything in {@link PoolSchema}, plus the
 * block of properties specific to the pool's own protocol.
 *
 * The shared fields are flattened server-side, so this extends the list schema
 * rather than nesting it. The protocol block is keyed by protocol name and is
 * **absent** (not null) when the pool belongs to another protocol, or has no
 * resolved properties yet — hence `.optional()`.
 */
export const PoolDetailSchema = PoolSchema.extend({
  meteoraDammV2: MeteoraDammV2PropertiesSchema.optional(),
});

export type PoolDetailResponse = z.infer<typeof PoolDetailSchema>;