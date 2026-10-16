/**
 * The base-fee kind, and a dynamic-fee pill on top when there is one; labels come translated,
 * so it renders in a server or a client tree. An unknown or absent kind is "—".
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
  labels: { kinds: Record<string, string>; dynamic: string };
}) {
  const kindLabel = baseFeeKind === null ? null : (labels.kinds[baseFeeKind] ?? null);

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
