/**
 * Pool detail page — "Fees" block.
 *
 * Two time-series charts over the pool's hourly history
 * (`GET /api/pools/{address}/history`):
 *
 *   - Fee revenue (USD)   — realized trading fee per hour (area)
 *   - Effective fee rate  — fees / volume in bps per hour (line)
 *
 * Server Component: maps the history into serializable `{ t, value }` point
 * arrays and hands each to the `TimeSeriesChart` client island. A chart is
 * dropped when its series is empty (no priced activity of that kind in the
 * window); the whole block collapses to an empty state when neither has data.
 */

import { getTranslations } from "next-intl/server";

import type { PoolHistoryResponse } from "@/lib/api/schema/pool-history";

import {
  TimeSeriesChart,
  type ChartPoint,
} from "./charts/time-series-chart";

const SECTION_CLASS = "px-6 lg:px-10";
const CARD_CLASS = "rounded-[8px] border border-sothoth-500/15 bg-cosmos-900/40";
const TITLE_BAR_CLASS =
  "flex items-center justify-between border-b border-sothoth-500/20 px-6 py-4";
const SECTION_TITLE_CLASS =
  "text-[11px] font-semibold tracking-[0.2em] text-slate-400 uppercase";
const CHART_TITLE_CLASS =
  "mb-2 text-[11px] font-semibold tracking-[0.15em] text-slate-400 uppercase";

// Chart colours (passed as plain strings — Client Component boundary).
const FEE_COLOR = "#5eead4"; // teal
const RATE_COLOR = "#a78bfa"; // violet

function series(
  history: PoolHistoryResponse,
  pick: (b: PoolHistoryResponse[number]) => string | null,
): ChartPoint[] {
  return history
    .filter((b) => pick(b) !== null)
    .map((b) => ({ t: Date.parse(b.bucket), value: Number(pick(b)) }));
}

export async function PoolDetailFees({
  history,
  locale,
}: {
  history: PoolHistoryResponse;
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
            <p className="max-w-[52ch] text-[13px] leading-[1.6] text-slate-400">
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
