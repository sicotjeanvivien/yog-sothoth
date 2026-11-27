/** Shortens a base58 pubkey for a table cell, keeping both ends. */

import { formatTokenText } from "./format-token-text";

const DEFAULT_CHARS_EACH_SIDE = 4;

const ELLIPSIS = "…";

/**
 * `"CGPx…Zp5j"`, keeping `chars` on each side; an input already that short is returned unchanged.
 */
export function shortenPubkey(
  pubkey: string,
  chars: number = DEFAULT_CHARS_EACH_SIDE,
): string {
  if (chars < 1) {
    throw new RangeError(`chars must be >= 1, got ${chars}`);
  }

  const minLength = chars * 2 + ELLIPSIS.length;
  if (pubkey.length <= minLength) {
    return pubkey;
  }

  const head = pubkey.slice(0, chars);
  const tail = pubkey.slice(-chars);
  return `${head}${ELLIPSIS}${tail}`;
}

/** A pool named by its pair, `"SOL / USDC"`, or by its shortened address while a symbol is unknown.
 * ⚠️ Each symbol isolated (FSI…PDI): a right-to-left one would reverse the line. */
export function pairLabel(pool: {
  poolAddress: string;
  tokenASymbol: string | null;
  tokenBSymbol: string | null;
}): string {
  return pool.tokenASymbol !== null && pool.tokenBSymbol !== null
    ? `\u2068${formatTokenText(pool.tokenASymbol)}\u2069 / \u2068${formatTokenText(pool.tokenBSymbol)}\u2069`
    : shortenPubkey(pool.poolAddress);
}
