/** The locked liquidity block's figures: the first tokens of the ranking, then the rest of the reading. */

import type { EmbeddedTokenResponse } from "@/lib/api/schema/zod.gen";
import { amountOf, shareOf } from "@/lib/overview/totals";

export type LockedToken = {
  token: EmbeddedTokenResponse;
  /** `null` when none of its locked pools is valued. */
  usd: number | null;
  share: number | null;
};

export type LockedTokens = {
  tokens: LockedToken[];
  /** What the other tokens hold; `null` when it rounds to no cent, or a term is unknown. */
  others: { usd: number; share: number | null } | null;
};

type LockedPage = {
  items: readonly { token: EmbeddedTokenResponse; lockedUsd: string | null }[];
  total: { lockedUsd: string | null };
};

/** Each token's part of the reading's locked total, and what the tokens after them hold. */
export function lockedTokensOf(page: LockedPage): LockedTokens {
  const total = amountOf(page.total.lockedUsd);
  const tokens = page.items.map((item) => {
    const usd = amountOf(item.lockedUsd);
    return { token: item.token, usd, share: shareOf(usd, total) };
  });
  // ⚠️ The tokens add up to the reading: the rest is the total less the rows, known only if each is.
  const shown = tokens.every((t) => t.usd !== null)
    ? tokens.reduce((sum, t) => sum + (t.usd ?? 0), 0)
    : null;
  const rest = total === null || shown === null ? null : total - shown;
  const others =
    rest === null || Math.round(rest * 100) <= 0 ? null : { usd: rest, share: shareOf(rest, total) };
  return { tokens, others };
}
