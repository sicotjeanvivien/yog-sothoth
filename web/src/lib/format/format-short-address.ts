/**
 * Truncate a Solana address for display, keeping the first and last
 * 4 characters with a fixed-width ellipsis in between:
 *
 *   "7xKXtg2CW87d9TXrismQHwzgQjhX6c..."  →  "7xKX...QrPa"
 *
 * Defaults match the convention used across DeFi dashboards. The
 * function is pure and locale-agnostic so it can be used both on the
 * server and on the client without coupling to next-intl.
 *
 * `head` and `tail` are exposed so the same helper can produce
 * shorter or longer forms for tighter UIs.
 */

const DEFAULT_HEAD = 4;
const DEFAULT_TAIL = 4;

export function formatShortAddress(
  address: string,
  options?: { head?: number; tail?: number },
): string {
  const head = options?.head ?? DEFAULT_HEAD;
  const tail = options?.tail ?? DEFAULT_TAIL;

  // Anything shorter than head + tail + 3 ("...") wouldn't gain
  // from truncation — return it unchanged.
  if (address.length <= head + tail + 3) {
    return address;
  }

  return `${address.slice(0, head)}...${address.slice(-tail)}`;
}