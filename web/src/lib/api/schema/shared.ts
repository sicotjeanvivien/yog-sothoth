import * as z from "zod";

// ─────────────────────────────────────────────────────────────────────
// Shared primitives
// ─────────────────────────────────────────────────────────────────────

/**
 * A decimal u128 as emitted by yog-api: a non-empty digit-only string.
 * Components that need numeric semantics call `BigInt(value)` themselves.
 */
export const U128String = z.string().regex(/^\d+$/, "expected a non-negative decimal integer");

/** RFC 3339 timestamp with timezone offset — matches Rust's `chrono::DateTime<Utc>` output. */
export const Rfc3339 = z.iso.datetime({ offset: true });

export const BigDecimal = z.string().regex(/^\d+(\.\d+)?$/, {
  message: "Doit être un nombre valide sous forme de string (ex: '86.6384' ou '0.00098')",
});

/**
 * A decimal that may be negative, as emitted by yog-api for values that
 * carry a direction — e.g. a signal's `value` (a price deviation of
 * `-0.2157` is 21.57% *below* the oracle). `BigDecimal` above is
 * unsigned on purpose (amounts, prices); do not widen it.
 */
export const SignedBigDecimal = z.string().regex(/^-?\d+(\.\d+)?$/, {
  message: "expected a decimal number as a string, optionally negative (e.g. '-0.2157')",
});

/**
 * A fee-split percent as emitted by yog-api: a `u8` in `0..=100`, sent as a
 * JSON number (not a string). Used for the protocol/partner/referral cuts.
 */
export const FeePercent = z.number().int().min(0).max(100);