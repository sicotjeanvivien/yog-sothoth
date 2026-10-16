/** Shortens a base58 pubkey for a table cell, keeping both ends. */

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