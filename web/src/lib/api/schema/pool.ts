import * as z from "zod";
import { BigDecimal, Rfc3339 } from "./shared";
import { TokenSchema } from "./token";

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
 *     first_seen_at: DateTime<Utc>,
 *     last_seen_at: DateTime<Utc>,
 * }
 * ```
 *
 * `feeBps` is the pool's base trading fee in basis points (its genesis
 * fee tier), null until the `InitializePool` event has been indexed.
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
  tvlUsd: BigDecimal.nullable(),
  volume24hUsd: BigDecimal.nullable(),
  firstSeenAt: Rfc3339,
  lastSeenAt: Rfc3339,
});

export type PoolResponse = z.infer<typeof PoolSchema>;