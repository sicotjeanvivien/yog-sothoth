/**
 * Display helpers for Solana base58 public keys.
 *
 * Pubkeys are 44 characters and unreadable in full inside table cells.
 * `shortenPubkey` collapses the middle into an ellipsis while keeping
 * enough leading/trailing characters for visual identification.
 *
 * The function is pure — no DOM, no React — and lives in `lib/format/`
 * alongside other display formatters.
 */

/** Default number of characters preserved on each side of the ellipsis. */
const DEFAULT_CHARS_EACH_SIDE = 4;

/** Character used as the middle ellipsis. Single em-dash for compactness. */
const ELLIPSIS = "…";

/**
 * Shorten a base58 public key for compact display.
 *
 * @param pubkey  The full base58 string (44 characters for a typical Solana pubkey).
 * @param chars   How many characters to keep on each side of the ellipsis. Default 4.
 *
 * Returns the input unchanged if it is already shorter than the
 * combined cutoff (defensive — avoids producing a result longer than
 * the input).
 *
 * @example
 *   shortenPubkey("CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j")
 *   // → "CGPx…Zp5j"
 */
export function shortenPubkey(
  pubkey: string,
  chars: number = DEFAULT_CHARS_EACH_SIDE,
): string {
  if (chars < 1) {
    throw new RangeError(`chars must be >= 1, got ${chars}`);
  }

  // Threshold: keep + ellipsis + keep. If the input is already that
  // short or shorter, returning it unchanged is the only correct
  // behaviour.
  const minLength = chars * 2 + ELLIPSIS.length;
  if (pubkey.length <= minLength) {
    return pubkey;
  }

  const head = pubkey.slice(0, chars);
  const tail = pubkey.slice(-chars);
  return `${head}${ELLIPSIS}${tail}`;
}