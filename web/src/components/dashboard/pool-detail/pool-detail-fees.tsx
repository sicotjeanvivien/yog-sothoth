/**
 * The pool detail's fee charts — revenue and effective rate per hour — from the history. A
 * series with no priced activity drops its chart; with neither, the block is an empty state.
 */

import { getTranslations } from "next-intl/server";

import type { PoolHistoryBucketResponse } from "@/lib/api/schema/zod.gen";

import {
  TimeSeriesChart,
  type ChartPoint,
} from "./charts/time-series-chart";

const SECTION_CLASS = "px-6 lg:px-10";
const CARD_CLASS = "rounded-[8px] border border-sothoth-500/15 bg-cosmos-900/40";
const TITLE_BAR_CLASS =
  "flex items-center justify-between border-b border-sothoth-500/20 px-6 py-4";
const SECTION_TITLE_CLASS =
  "text-[12px] font-semibold tracking-[0.2em] text-slate-400 uppercase";
const CHART_TITLE_CLASS =
  "mb-2 text-[12px] font-semibold tracking-[0.15em] text-slate-400 uppercase";

// Plain strings: they cross the client boundary.
const FEE_COLOR = "#5eead4"; // teal
const RATE_COLOR = "#a78bfa"; // violet

function series(
  history: PoolHistoryBucketResponse[],
  pick: (b: PoolHistoryBucketResponse) => string | null,
): ChartPoint[] {
  return history
    .filter((b) => pick(b) !== null)
    .map((b) => ({ t: Date.parse(b.bucket), value: Number(pick(b)) }));
}

export async function PoolDetailFees({
  history,
  locale,
}: {
  history: PoolHistoryBucketResponse[];
  locale: string;
}) {
  const t = await getTranslations("Dashboard.PoolDetail.fees");

  const feeRevenue = series(history, (b) => b.feesUsd);
  const effectiveRate = series(history, (b) => b.effectiveFeeBps);

  const hasData = feeRevenue.length > 0 || effectiveRate.length > 0;

  return (
    <section className={`mt-6 ${SECTION_CLASS}`}>
      <div className={CARD_CLASS}>
        <div className={TITLE_BAR_CLASS}>
          <h2 className={SECTION_TITLE_CLASS}>{t("title")}</h2>
        </div>

        {!hasData ? (
          <div className="flex flex-col items-center px-6 py-12 text-center">
            <p className="max-w-[52ch] text-[14px] leading-[1.6] text-slate-400">
              {t("empty")}
            </p>
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-6 p-6 lg:grid-cols-2">
            {feeRevenue.length > 0 && (
              <div>
                <h3 className={CHART_TITLE_CLASS}>{t("revenue")}</h3>
                <TimeSeriesChart
                  data={feeRevenue}
                  variant="area"
                  valueFormat="usd"
                  color={FEE_COLOR}
                  locale={locale}
                />
              </div>
            )}
            {effectiveRate.length > 0 && (
              <div>
                <h3 className={CHART_TITLE_CLASS}>{t("effectiveRate")}</h3>
                <TimeSeriesChart
                  data={effectiveRate}
                  variant="line"
                  valueFormat="bps"
                  color={RATE_COLOR}
                  locale={locale}
                />
              </div>
            )}
          </div>
        )}
      </div>
    </section>
  );
}
