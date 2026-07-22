/**
 * Fee-shape badge — how a pool's base fee behaves, plus a dynamic-fee pill.
 *
 * Renders one pill for the base-fee kind (constant / fee scheduler / rate
 * limiter) and, when present, a second pill flagging a volatility-based
 * dynamic fee on top. The two are orthogonal: a pool can run a scheduler *and*
 * a dynamic fee, so both pills can show at once.
 *
 * Prop-driven and free of server-only imports (labels are passed already
 * translated), so it renders in either the server detail panel or a client
 * tree. `baseFeeKind` is the opaque API string; an unknown or absent value
 * (pool seen before its `InitializePool`, or a future protocol's vocabulary we
 * have no label for) falls back to the em-dash — "factual or absent, never
 * fake". A dynamic fee can still show even when the base kind is unlabelled.
 */

import type { ReactNode } from "react";

const DASH = "—";

const PILL_CLASS =
  "inline-flex items-center rounded-full border border-sothoth-500/20 " +
  "bg-cosmos-900/60 px-2 py-0.5 text-[12px] leading-none text-slate-300";

export function FeeTypeBadge({
  baseFeeKind,
  hasDynamicFee,
  labels,
}: {
  baseFeeKind: string | null;
  hasDynamicFee: boolean | null;
  /** Already-translated: `kinds` maps the API string to its label; `dynamic`
   *  is the dynamic-fee pill text. */
  labels: { kinds: Record<string, string>; dynamic: string };
}) {
  const kindLabel = baseFeeKind === null ? null : (labels.kinds[baseFeeKind] ?? null);

  // Nothing decodable to show yet.
  if (kindLabel === null && hasDynamicFee !== true) {
    return <span className="text-slate-500">{DASH}</span>;
  }

  return (
    <span className="inline-flex flex-wrap items-center gap-1.5">
      {kindLabel !== null && <Pill>{kindLabel}</Pill>}
      {hasDynamicFee === true && <Pill>{labels.dynamic}</Pill>}
    </span>
  );
}

function Pill({ children }: { children: ReactNode }) {
  return <span className={PILL_CLASS}>{children}</span>;
}
