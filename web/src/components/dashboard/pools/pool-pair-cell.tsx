/**
 * Token pair cell — two overlapping logos + "SYMBOL_A / SYMBOL_B".
 *
 * Uses a native `<img>` rather than `next/image`:
 *   - logos are 24px and one per row; the optimisation gain from
 *     `next/image` is negligible at this scale,
 *   - external hosts vary across token registries — going native
 *     avoids a maintenance burden on `next.config.ts`.
 *
 * If a token has no `logoUri` we fall back to a filled circle
 * showing the first letter of its symbol. That covers tokens
 * Helius DAS does not return metadata for.
 */

import type { TokenResponse } from "@/lib/api/schema/token";

export function PoolPairCell({
  tokenA,
  tokenB,
}: {
  tokenA: TokenResponse;
  tokenB: TokenResponse;
}) {
  return (
    <div className="flex items-center gap-3">
      {/* Stacked logos */}
      <div className="relative flex shrink-0 items-center">
        <TokenLogo token={tokenA} />
        <div className="-ml-2">
          <TokenLogo token={tokenB} />
        </div>
      </div>

      {/* Symbols */}
      <span className="font-medium text-slate-100">
        {tokenA.symbol}
        <span className="mx-1 text-slate-500">/</span>
        {tokenB.symbol}
      </span>
    </div>
  );
}

// ── Sub-component ─────────────────────────────────────────────────────

function TokenLogo({ token }: { token: TokenResponse }) {
  const sharedClass =
    "h-6 w-6 rounded-full border border-cosmos-900 bg-cosmos-800 ring-1 ring-sothoth-500/20";

  if (token.logoUri) {
    return (
      // eslint-disable-next-line @next/next/no-img-element
      <img
        src={token.logoUri}
        alt=""
        loading="lazy"
        decoding="async"
        className={`${sharedClass} object-cover`}
      />
    );
  }

  // Fallback — filled circle with the first letter of the symbol.
  return (
    <div
      aria-hidden="true"
      className={`${sharedClass} flex items-center justify-center text-[10px] font-bold tracking-wide text-sothoth-300 uppercase`}
    >
      {token.symbol ? token.symbol.charAt(0) : "-"}
    </div>
  );
}