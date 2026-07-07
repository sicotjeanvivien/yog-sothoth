/**
 * Pool detail page — "Alerts" block.
 *
 * The pool-filtered slice of the signal feed (`GET /api/signals?pool=`),
 * newest first, rendered with the canonical `SignalCard` — same visual
 * language as `/signals` and the Overview block. Static history: no SSE
 * here in v1 (the tab is a record, the live tail lives on `/signals`);
 * client-side filtering of the global stream can come later without
 * rework.
 *
 * The page owns the fetch and the pagination (namespaced
 * `alerts*` URL params, like swaps/liquidity); this component owns the
 * section shell, the list and the pool-specific empty state.
 */

import { getTranslations } from "next-intl/server";

import type { SignalResponse } from "@/lib/api/schema/signal";

import { SignalCard } from "@/components/dashboard/signals/signal-card";

const SECTION_CLASS = "px-6 lg:px-10";

const SECTION_TITLE_CLASS =
  "text-[12px] font-semibold tracking-[0.2em] text-slate-400 uppercase";

export async function PoolDetailAlerts({
  signals,
}: {
  signals: SignalResponse[];
}) {
  const t = await getTranslations("Dashboard.PoolDetail.alerts");

  return (
    <section className={`mt-6 ${SECTION_CLASS}`}>
      <div className="mb-4 flex items-center justify-between">
        <h2 className={SECTION_TITLE_CLASS}>{t("title")}</h2>
      </div>

      {signals.length === 0 ? (
        <p className="rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/40 px-4 py-6 text-[14px] text-slate-400">
          {t("empty")}
        </p>
      ) : (
        <ul className="flex flex-col gap-2">
          {signals.map((signal) => (
            <SignalCard key={signal.id} signal={signal} />
          ))}
        </ul>
      )}
    </section>
  );
}
