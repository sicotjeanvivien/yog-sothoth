import * as z from "zod";
import { BigDecimal, Rfc3339, SignedBigDecimal } from "./shared";
import { TokenSchema } from "./token";

// ─────────────────────────────────────────────────────────────────────
// SignalResponse — mirrors `api::http::dto::response::SignalResponse`
// ─────────────────────────────────────────────────────────────────────

/**
 * Severity levels, mirroring the Rust `Severity` enum (and the CHECK
 * constraint on the `signals` table). A closed set — an unknown value
 * is a wire-contract drift worth failing on.
 */
export const SeveritySchema = z.enum(["info", "warning", "critical"]);
export type Severity = z.infer<typeof SeveritySchema>;

/**
 * Wire shape of a single signal, shared by the paginated feed
 * (`GET /api/signals`) and the SSE stream (`GET /api/signals/stream`)
 * — both endpoints emit exactly this object.
 */
export const SignalSchema = z.object({
  // Storage identity: stable across pages and stream events, used as
  // the reconciliation key when the live feed refetches after a
  // reconnect (and as the React list key).
  id: z.number().int(),
  detector: z.string().min(1),
  protocol: z.string().min(1),
  poolAddress: z.string().min(1),
  // The pool's token pair, embedded in the same shape as PoolResponse
  // (`EmbeddedTokenResponse` on the wire). Every field inside is
  // nullable — an unresolved pool embeds the minimal view and the UI
  // falls back to the pool address.
  tokenA: TokenSchema,
  tokenB: TokenSchema,
  severity: SeveritySchema,
  // The metric that crossed the threshold. Signed: a deviation can be
  // below as well as above the reference.
  value: SignedBigDecimal,
  // The configured threshold it crossed (always positive), when the
  // detector has a single scalar one.
  threshold: BigDecimal.nullable(),
  message: z.string().nullable(),
  triggeredAt: Rfc3339,
});

export type SignalResponse = z.infer<typeof SignalSchema>;
