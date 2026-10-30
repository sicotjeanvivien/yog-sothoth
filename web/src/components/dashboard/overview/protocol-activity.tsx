/**
 * Activity by protocol: one row per protocol of the menu, observed or "Soon"; fees and active
 * pools are the advanced mode's. Raydium and Orca, out of the domain, are written in the web.
 */

import { getLocale, getTranslations } from "next-intl/server";

import { PROTOCOL_SCOPE, type ScopeLine } from "@/components/dashboard/sidebar/sidebar-nav";
import { ApiClientError } from "@/lib/api/errors";
import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";
import { fetchProtocols, fetchSwapTotals, fetchTvl } from "@/lib/api/server/protocols";
import { volumeCoverage } from "@/lib/coverage/volume-coverage";
import { formatCount } from "@/lib/format/format-count";
import { formatPercent } from "@/lib/format/format-percent";
import { formatUsdCompact } from "@/lib/format/format-usd";
import type { Period } from "@/lib/overview/period";
import { sumUsd } from "@/lib/overview/totals";

import { BlockHeading, closedReason, PeriodNotice } from "./overview-parts";
import { BlockFailed, BlockSkeleton } from "./overview-states";
import { VolumeCoverageMark } from "./volume-coverage-mark";
import {
  ADVANCED_ONLY,
  CELL_CLASS,
  HEAD_ROW_CLASS,
  MIN_HEIGHT,
  NUM_CLASS,
  ROW_CLASS,
  SPAN,
} from "./overview-layout";

/** The API protocol behind each menu line; `null` when it has none. */
const PROTOCOL_OF: Record<ScopeLine["key"], ProtocolResponse | null> = {
  meteoraDammV2: "meteora_damm_v2",
  meteoraDlmm: "meteora_dlmm",
  meteora: null,
  raydium: null,
  orca: null,
};

// ⚠️ Each row is its own grid: an `auto` or `max-content` column would size per row and the
// columns would no longer line up.
const COLUMNS =
  "min-w-[460px] grid-cols-[minmax(0,1.7fr)_repeat(2,minmax(0,1fr))_minmax(0,1.4fr)] adv:min-w-[640px] adv:grid-cols-[minmax(0,1.7fr)_72px_108px_108px_100px_minmax(0,1.2fr)]";

const SECTION_CLASS = `${CELL_CLASS} ${SPAN.main} ${MIN_HEIGHT.protocols}`;

export async function ProtocolActivity({ period }: { period: Period }) {
  const locale = await getLocale();
  const t = await getTranslations("Dashboard.Overview");
  const tNav = await getTranslations("Dashboard.Sidebar.nav");
  const short = t(`period.short.${period}`);
  const title = t("protocols.title", { period: short });

  let catalogue, tvl, activity;
  try {
    [catalogue, tvl, activity] = await Promise.all([
      fetchProtocols(),
      fetchTvl("all"),
      fetchSwapTotals("all", period),
    ]);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section aria-label={title} className={SECTION_CLASS}>
          <BlockHeading title={title} info={t("protocols.info")} />
          <BlockFailed kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  const closed = await closedReason(period, activity);
  const unit = activity.coverageUnit;
  const total = sumUsd(activity.items.map((i) => i.volumeUsd));
  const observed = new Set(catalogue.items.filter((p) => p.observed).map((p) => p.protocol));

  return (
    <section aria-label={title} className={SECTION_CLASS}>
      <BlockHeading title={title} info={t("protocols.info")} />
      {closed !== null && <PeriodNotice reason={closed} />}
      <div className="dash-scroll overflow-x-auto">
        <div role="table" aria-label={t("protocols.tableLabel")}>
          <div role="row" className={`${HEAD_ROW_CLASS} ${COLUMNS}`}>
            <div role="columnheader">{t("protocols.protocol")}</div>
            <div role="columnheader" className="text-right">
              {t("protocols.tvl")}
            </div>
            <div role="columnheader" className="text-right">
              {t("protocols.volume", { period: short })}
            </div>
            <div role="columnheader" className={`${ADVANCED_ONLY.block} text-right`}>
              {t("protocols.fees", { period: short })}
            </div>
            <div role="columnheader" className={`${ADVANCED_ONLY.block} text-right`}>
              {t("protocols.activePools")}
            </div>
            <div role="columnheader" className="text-right">
              {t("protocols.share")}
            </div>
          </div>
          {PROTOCOL_SCOPE.map((line) => {
            const name = line.labelKeys.map((key) => tNav(key)).join(" ");
            const protocol = PROTOCOL_OF[line.key];
            if (protocol === null || !observed.has(protocol)) {
              return (
                <div key={line.key} role="row" className={`${ROW_CLASS} ${COLUMNS}`}>
                  <div role="cell">
                    <span className="text-dash-ink-3">{name}</span>{" "}
                    <span className="border border-dashed border-dash-ink-3 px-1.5 py-px font-dash-mono text-[11px] tracking-[0.08em] whitespace-nowrap text-dash-ink-3 uppercase">
                      {tNav("soon")}
                    </span>
                  </div>
                  <div role="cell" className={`${NUM_CLASS} text-dash-ink-3`}>—</div>
                  <div role="cell" className={`${NUM_CLASS} text-dash-ink-3`}>—</div>
                  <div role="cell" className={`${ADVANCED_ONLY.block} ${NUM_CLASS} text-dash-ink-3`}>—</div>
                  <div role="cell" className={`${ADVANCED_ONLY.block} ${NUM_CLASS} text-dash-ink-3`}>—</div>
                  <div role="cell" className={`${NUM_CLASS} text-dash-ink-3`}>—</div>
                </div>
              );
            }
            const row = activity.items.find((i) => i.protocol === protocol);
            const volume = row ? sumUsd([row.volumeUsd]) : null;
            const share = volume !== null && total !== null && total > 0 ? volume / total : null;
            const protocolTvl = tvl.items.find((i) => i.protocol === protocol)?.tvlUsd ?? null;
            return (
              <div key={line.key} role="row" className={`${ROW_CLASS} ${COLUMNS}`}>
                <div role="cell">
                  <span className="text-dash-ink">{name}</span>{" "}
                  <span className="font-dash-mono text-[11px] whitespace-nowrap text-dash-ink-3">
                    {t("protocols.live")}
                  </span>
                </div>
                <div role="cell" className={NUM_CLASS}>
                  {formatUsdCompact(protocolTvl)}
                </div>
                <div role="cell" className={NUM_CLASS}>
                  {formatUsdCompact(row?.volumeUsd ?? null)}
                  <VolumeCoverageMark
                    coverage={row ? volumeCoverage(row) : null}
                    label={(priced, total) => t(`coverage.${unit}.mark`, { priced, total })}
                  />
                </div>
                <div role="cell" className={`${ADVANCED_ONLY.block} ${NUM_CLASS}`}>
                  {formatUsdCompact(row?.feesUsd ?? null)}
                  <VolumeCoverageMark
                    coverage={row ? volumeCoverage(row) : null}
                    label={(priced, total) => t(`coverage.${unit}.mark`, { priced, total })}
                  />
                </div>
                <div role="cell" className={`${ADVANCED_ONLY.block} ${NUM_CLASS}`}>
                  {formatCount(row?.pools ?? null)}
                </div>
                <div role="cell" className="flex items-center justify-end gap-2">
                  <div className="h-1.5 max-w-[120px] flex-auto bg-dash-surface-2">
                    <div
                      className="h-1.5 bg-dash-accent"
                      style={{ width: `${(share ?? 0) * 100}%` }}
                    />
                  </div>
                  <span className={`${NUM_CLASS} min-w-12`}>
                    {share === null ? "—" : formatPercent(String(share), locale)}
                  </span>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}

export async function ProtocolActivityPlaceholder({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview");
  return (
    <BlockSkeleton
      title={t("protocols.title", { period: t(`period.short.${period}`) })}
      span={SPAN.main}
      minHeight={MIN_HEIGHT.protocols}
      shape={{ rows: 5 }}
    />
  );
}
