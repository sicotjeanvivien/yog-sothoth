/**
 * The tab 03 of a protocol's page: where its fees go — the LPs, the protocol, the referral — over
 * the period and bar by bar. The server words every reading; the chart only tracks the bar under
 * the pointer.
 */

import { Suspense } from "react";
import { getLocale, getTranslations } from "next-intl/server";

import {
  CELL_CLASS,
  GRID_CLASS,
  HEAD_ROW_CLASS,
  LABEL_CLASS,
  NUM_CLASS,
  ROW_CLASS,
} from "@/components/dashboard/overview/overview-layout";
import { BlockHeading, closedReason, PeriodNotice } from "@/components/dashboard/overview/overview-parts";
import { BlockFailed, BlockSkeleton } from "@/components/dashboard/overview/overview-states";
import { ApiClientError } from "@/lib/api/errors";
import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";
import { fetchSwapsSeries } from "@/lib/api/server/protocols";
import { formatUtcMoment, type FormatLocale } from "@/lib/format/date";
import { formatPercent } from "@/lib/format/format-percent";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { axisOf, daysAgoOf, hourOf, roundTicks } from "@/lib/overview/chart";
import type { Period } from "@/lib/overview/period";
import { FEE_PARTS, feeSharesOf, feeSlotsOf, feeTotalsOf, type FeePart } from "@/lib/protocol/fees";

import { SplitBars, type SplitDetail } from "../split-bars";
import { FEE_TONE } from "./fee-tones";

/** The chart's height, and its tallest bar a little under it for the top mark's label. */
const CHART_PX = { height: 122, top: 112 } as const;

const SPLIT_SPAN = "col-span-12";

const SPLIT_CLASS = `${CELL_CLASS} ${SPLIT_SPAN} min-h-[380px]`;

const DESTINATION_COLUMNS = "grid-cols-[minmax(0,1fr)_96px_64px]";

const PER_SLOT = { "24h": "perHour", "7d": "perSixHours", "30d": "perDay" } as const;

const CHART_LABEL = {
  "24h": "chartLabelHourly",
  "7d": "chartLabelSixHours",
  "30d": "chartLabelDaily",
} as const;

/** The fields of the swap event each part sums. */
const FIELDS: Record<FeePart, string> = {
  lp: "claiming_fee + compounding_fee",
  protocol: "protocol_fee",
  referral: "referral_fee",
};

type TabProps = { period: Period; protocol: ProtocolResponse };

export async function FeesTab({ period, protocol }: TabProps) {
  const t = await getTranslations("Dashboard.Protocol.fees");

  return (
    <div className={GRID_CLASS}>
      <Suspense
        key={`split-${period}`}
        fallback={<BlockSkeleton title={t("title")} span={SPLIT_SPAN} minHeight="min-h-[380px]" shape="chart" />}
      >
        <FeeSplit period={period} protocol={protocol} />
      </Suspense>
    </div>
  );
}

async function FeeSplit({ period, protocol }: TabProps) {
  const t = await getTranslations("Dashboard.Protocol.fees");
  const tVolume = await getTranslations("Dashboard.Overview.volume");
  const tPeriod = await getTranslations("Dashboard.Overview.period");
  const locale = (await getLocale()) as FormatLocale;
  const title = t("title");
  const short = tPeriod(`short.${period}`);

  let swaps;
  try {
    swaps = await fetchSwapsSeries(protocol, period);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section aria-label={title} className={SPLIT_CLASS}>
          <BlockHeading title={title} info={t("info")} />
          <BlockFailed kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  const closed = await closedReason(period, swaps);
  const usd = (value: number) => formatUsdCompact(String(value));
  const percent = (ratio: number | undefined) =>
    ratio === undefined ? "—" : formatPercent(String(ratio), locale);

  const slots = swaps.asOf === null ? [] : feeSlotsOf(period, swaps.asOf, swaps.items);
  const totals = feeTotalsOf(slots);
  const shares = feeSharesOf(totals);
  const sumOf = (parts: Record<FeePart, number>) => parts.lp + parts.protocol + parts.referral;
  const valued = slots.map((slot) =>
    FEE_PARTS.every((part) => slot[part] === null)
      ? null
      : { lp: slot.lp ?? 0, protocol: slot.protocol ?? 0, referral: slot.referral ?? 0 },
  );
  const max = Math.max(0, ...valued.map((parts) => (parts === null ? 0 : sumOf(parts))));
  const scale = (v: number) => (max > 0 ? (v / max) * CHART_PX.top : 0);
  const daysAgo = (days: number) => (days === 0 ? tVolume("today") : tVolume("daysAgo", { days }));
  const moment = (date: Date) => formatUtcMoment(date, locale) ?? "—";

  const detailOf = (when: string, parts: Record<FeePart, number>): SplitDetail<FeePart> => {
    const split = feeSharesOf(parts);
    return {
      when,
      total: usd(sumOf(parts)),
      rows: FEE_PARTS.map((part) => ({
        tone: part,
        name: t(`parts.${part}`),
        value: usd(parts[part]),
        share: percent(split?.[part]),
      })),
    };
  };

  const bars = slots.map((slot, i) => {
    const when =
      period !== "24h"
        ? `${daysAgo(daysAgoOf(period, slots.length, i))} · ${moment(slot.from)} → ${moment(slot.to)}`
        : `${hourOf(slot.from)} UTC`;
    const parts = valued[i];
    if (parts === null || parts === undefined) {
      return { segments: [], detail: { when, total: tVolume("noValue"), rows: [] } };
    }
    const height = (v: number) => Math.max(v > 0 ? 1 : 0, Math.round(scale(v)));
    return {
      segments: FEE_PARTS.map((part) => ({ tone: part, height: height(parts[part]) })).filter(
        (s) => s.height > 0,
      ),
      detail: detailOf(when, parts),
    };
  });

  const legend = (
    <div className="flex flex-wrap gap-x-4 gap-y-1.5 text-[12px] text-dash-ink-2">
      {FEE_PARTS.map((part) => (
        <span key={part}>
          <span aria-hidden="true" className={`mr-1.5 inline-block h-2.5 w-2.5 align-[-1px] ${FEE_TONE[part]}`} />
          {t(`parts.${part}`)}
        </span>
      ))}
    </div>
  );

  return (
    <section aria-label={title} className={SPLIT_CLASS}>
      <BlockHeading title={title} info={t("info")}>
        {closed === null && legend}
      </BlockHeading>
      {closed !== null ? (
        <PeriodNotice reason={closed} />
      ) : (
        <div className="grid gap-x-8 gap-y-6 dash-wide:grid-cols-2">
          <div role="table" aria-label={t("tableLabel")}>
            <div role="row" className={`${HEAD_ROW_CLASS} ${DESTINATION_COLUMNS}`}>
              <div role="columnheader">{t("destination")}</div>
              <div role="columnheader" className={NUM_CLASS}>
                {short}
              </div>
              <div role="columnheader" className={NUM_CLASS}>
                {t("share")}
              </div>
            </div>
            {FEE_PARTS.map((part) => (
              <div key={part} role="row" className={`${ROW_CLASS} ${DESTINATION_COLUMNS}`}>
                <div role="cell" className="min-w-0">
                  <div className="text-dash-ink">
                    <span aria-hidden="true" className={`mr-1.5 inline-block h-2.5 w-2.5 align-[-1px] ${FEE_TONE[part]}`} />
                    {t(`parts.${part}`)}
                  </div>
                  <div className="mt-0.5 font-dash-mono text-[11px] text-dash-ink-3">{FIELDS[part]}</div>
                  <div aria-hidden="true" className="mt-1.5 h-1 bg-dash-rule">
                    <div
                      className={`h-full ${FEE_TONE[part]}`}
                      style={{ width: `${(shares?.[part] ?? 0) * 100}%` }}
                    />
                  </div>
                </div>
                <div role="cell" className={`${NUM_CLASS} text-dash-ink`}>
                  {usd(totals[part])}
                </div>
                <div role="cell" className={`${NUM_CLASS} text-dash-ink-2`}>
                  {percent(shares?.[part])}
                </div>
              </div>
            ))}
          </div>
          <div>
            <p className="mb-2 flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
              <span className={LABEL_CLASS}>{t(PER_SLOT[period])}</span>
              <span className="font-dash-mono text-[12px] text-dash-ink-2 tabular-nums">
                {t("total", { total: usd(sumOf(totals)) })}
              </span>
            </p>
            <SplitBars
              bars={bars}
              period={detailOf(t("periodTotal", { period: short }), totals)}
              ticks={roundTicks(max).map((value) => ({
                bottom: Math.round(scale(value)),
                label: usd(value),
              }))}
              axis={axisOf(period, slots, daysAgo)}
              height={CHART_PX.height}
              label={t(CHART_LABEL[period])}
              tones={FEE_TONE}
            />
          </div>
        </div>
      )}
    </section>
  );
}
