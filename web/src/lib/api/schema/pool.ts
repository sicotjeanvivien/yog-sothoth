import * as z from "zod";
import { Rfc3339 } from "./shared";
import { TokenSchema } from "./token";

// ─────────────────────────────────────────────────────────────────────
// PoolResponse — mirrors `api::http::dto::response::PoolResponse`
// ─────────────────────────────────────────────────────────────────────

/**
 * Wire shape of a pool as exposed by `GET /api/pools` and
 * `GET /api/pools/{address}`.
 *
 * Rust side (api/src/http/dto/response.rs):
 *
 * ```rust
 * pub struct PoolResponse {
 *     pool_address: String,
 *     protocol: String,
 *     token_a_mint: String,
 *     token_b_mint: String,
 *     first_seen_at: DateTime<Utc>,
 *     last_seen_at: DateTime<Utc>,
 * }
 * ```
 */
export const PoolSchema = z.object({
  pool_address: z.string().min(1),
  protocol: z.string().min(1),
  token_a: TokenSchema,
  token_b: TokenSchema,
  first_seen_at: Rfc3339,
  last_seen_at: Rfc3339,
});

export type PoolResponse = z.infer<typeof PoolSchema>;