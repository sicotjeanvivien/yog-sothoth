/**
 * The pair cell: two overlapping logos and the symbols. A native `<img>`, not `next/image`:
 * logo hosts vary per registry. A token without `logoUri` shows its symbol's first letter.
 */

import type { EmbeddedTokenResponse } from "@/lib/api/schema/zod.gen";

export function PoolPairCell({
  tokenA,
  tokenB,
}: {
  tokenA: EmbeddedTokenResponse;
  tokenB: EmbeddedTokenResponse;
}) {
  return (
    <div className="flex items-center gap-3">
      <div className="relative flex shrink-0 items-center">
        <TokenLogo token={tokenA} />
        <div className="-ml-2">
          <TokenLogo token={tokenB} />
        </div>
      </div>

      <span className="font-medium text-slate-100">
        {tokenA.symbol}
        <span className="mx-1 text-slate-500">/</span>
        {tokenB.symbol}
      </span>
    </div>
  );
}

function TokenLogo({ token }: { token: EmbeddedTokenResponse }) {
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

  return (
    <div
      aria-hidden="true"
      className={`${sharedClass} flex items-center justify-center text-[11px] font-bold tracking-wide text-sothoth-300 uppercase`}
    >
      {token.symbol ? token.symbol.charAt(0) : "-"}
    </div>
  );
}