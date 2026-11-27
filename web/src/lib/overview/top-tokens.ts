/** How the top tokens block names a token. */

import type { EmbeddedTokenResponse } from "@/lib/api/schema/zod.gen";
import { formatShortAddress } from "@/lib/format/format-short-address";
import { formatTokenText } from "@/lib/format/format-token-text";

/** Its symbol, its short mint while the symbol is unknown, "—" without either.
 * ⚠️ Symbol and name isolated (FSI…PDI): they share a cell, a right-to-left one would reorder both. */
export function topTokenSymbol(token: EmbeddedTokenResponse): string {
  if (token.symbol !== null) {
    return `\u2068${formatTokenText(token.symbol)}\u2069`;
  }
  return token.mint === null ? "—" : formatShortAddress(token.mint);
}

/** Its name, `null` when unknown or empty. */
export function topTokenName(token: EmbeddedTokenResponse): string | null {
  return token.name ? `\u2068${formatTokenText(token.name)}\u2069` : null;
}
