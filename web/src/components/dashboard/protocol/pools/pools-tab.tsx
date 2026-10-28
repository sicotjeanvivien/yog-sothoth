/**
 * The tab 04 of a protocol's page: the pools born over the period, bar by bar, and those whose
 * first deposit is worth the most. The server words every reading; the chart only tracks the bar
 * under the pointer.
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
import { fetchLargestPoolCreations, fetchPoolCreationsSeries } from "@/lib/api/server/protocols";
import { formatCount } from "@/lib/format/format-count";
import { formatRelative, formatUtcMoment, type FormatLocale } from "@/lib/format/date";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { shortenPubkey } from "@/lib/format/pubkey";
import { axisOf, daysAgoOf, hourOf, roundTicks } from "@/lib/overview/chart";
import type { Period } from "@/lib/overview/period";
import { birthSlotsOf, birthTotalOf } from "@/lib/protocol/pools";

import { SplitBars, type SplitDetail } from "../split-bars";

/** The chart's height, and its tallest bar a little under it for the top mark's label. */
const CHART_PX = { height: 122, top: 112 } as const;

const DEPOSITS_SHOWN = 5;

const BIRTHS_SPAN = "col-span-12 dash-wide:col-span-7";

const DEPOSITS_SPAN = "col-span-12 dash-wide:col-span-5";

const BIRTHS_CLASS = `${CELL_CLASS} ${BIRTHS_SPAN} min-h-[262px]`;

const DEPOSITS_CLASS = `${CELL_CLASS} ${DEPOSITS_SPAN} min-h-[262px]`;

const DEPOSIT_COLUMNS = "grid-cols-[minmax(0,1.2fr)_minmax(0,0.9fr)_auto_auto]";

const BIRTH_TONE = { births: "bg-dash-flow-create" } as const;

const PER_SLOT = { "24h": "perHour", "7d": "perSixHours", "30d": "perDay" } as const;

const CHART_LABEL = {
  "24h": "chartLabelHourly",
  "7d": "chartLabelSixHours",
  "30d": "chartLabelDaily",
} as const;

type TabProps = { period: Period; protocol: ProtocolResponse };

export async function PoolsTab({ period, protocol }: TabProps) {
  const t = await getTranslations("Dashboard.Protocol.pools");
  const tPeriod = await getTranslations("Dashboard.Overview.period");

  return (
    <div className={GRID_CLASS}>
      <Suspense
        key={`births-${period}`}
        fallback={
          <BlockSkeleton title={t("births.title")} span={BIRTHS_SPAN} minHeight="min-h-[262px]" shape="chart" />
        }
      >
        <Births period={period} protocol={protocol} />
      </Suspense>
      <Suspense
        key={`deposits-${period}`}
        fallback={
          <BlockSkeleton
            title={t("deposits.title", { period: tPeriod(`short.${period}`) })}
            span={DEPOSITS_SPAN}
            minHeight="min-h-[262px]"
            shape={{ rows: DEPOSITS_SHOWN }}
          />
        }
      >
        <LargestDeposits period={period} protocol={protocol} />
      </Suspense>
    </div>
  );
}

async function Births({ period, protocol }: TabProps) {
  const t = await getTranslations("Dashboard.Protocol.pools.births");
  const tVolume = await getTranslations("Dashboard.Overview.volume");
  const tPeriod = await getTranslations("Dashboard.Overview.period");
  const locale = (await getLocale()) as FormatLocale;
  const title = t("title");

  let series;
  try {
    series = await fetchPoolCreationsSeries(protocol, period);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section aria-label={title} className={BIRTHS_CLASS}>
          <BlockHeading title={title} info={t("info")} />
          <BlockFailed kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  const closed = await closedReason(period, series);
  const slots = series.asOf === null ? [] : birthSlotsOf(period, series.asOf, series.items);
  const total = birthTotalOf(slots);
  const max = Math.max(0, ...slots.map((s) => s.value ?? 0));
  const scale = (v: number) => (max > 0 ? (v / max) * CHART_PX.top : 0);
  const daysAgo = (days: number) => (days === 0 ? tVolume("today") : tVolume("daysAgo", { days }));
  const moment = (date: Date) => formatUtcMoment(date, locale) ?? "—";

  const detailOf = (when: string, pools: number | null): SplitDetail<"births"> => ({
    when,
    total: formatCount(pools),
    rows: [{ tone: "births", name: t("pools"), value: formatCount(pools), share: "" }],
  });

  const bars = slots.map((slot, i) => {
    const when =
      period !== "24h"
        ? `${daysAgo(daysAgoOf(period, slots.length, i))} · ${moment(slot.from)} → ${moment(slot.to)}`
        : `${hourOf(slot.from)} UTC`;
    if (slot.value === null) {
      return { segments: [], detail: { when, total: t("noValue"), rows: [] } };
    }
    const height = Math.max(slot.value > 0 ? 1 : 0, Math.round(scale(slot.value)));
    return {
      segments: height > 0 ? [{ tone: "births" as const, height }] : [],
      detail: detailOf(when, slot.value),
    };
  });

  return (
    <section aria-label={title} className={BIRTHS_CLASS}>
      <BlockHeading title={title} info={t("info")} />
      {closed !== null ? (
        <PeriodNotice reason={closed} />
      ) : (
        <>
          <p className="mb-2 flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
            <span className={LABEL_CLASS}>{t(PER_SLOT[period])}</span>
            <span className="font-dash-mono text-[12px] text-dash-ink-2 tabular-nums">
              {t("total", { total: formatCount(total) })}
            </span>
          </p>
          <SplitBars
            bars={bars}
            period={detailOf(t("periodTotal", { period: tPeriod(`short.${period}`) }), total)}
            ticks={roundTicks(max).map((value) => ({
              bottom: Math.round(scale(value)),
              label: formatCount(value),
            }))}
            axis={axisOf(period, slots, daysAgo)}
            height={CHART_PX.height}
            label={t(CHART_LABEL[period])}
            tones={BIRTH_TONE}
          />
        </>
      )}
    </section>
  );
}

async function LargestDeposits({ period, protocol }: TabProps) {
  const t = await getTranslations("Dashboard.Protocol.pools.deposits");
  const tPeriod = await getTranslations("Dashboard.Overview.period");
  const locale = (await getLocale()) as FormatLocale;
  const title = t("title", { period: tPeriod(`short.${period}`) });

  let creations;
  try {
    creations = await fetchLargestPoolCreations(protocol, period, DEPOSITS_SHOWN);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section aria-label={title} className={DEPOSITS_CLASS}>
          <BlockHeading title={title} info={t("info")} />
          <BlockFailed kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  const closed = await closedReason(period, creations);
  const now = new Date();

  return (
    <section aria-label={title} className={DEPOSITS_CLASS}>
      <BlockHeading title={title} info={t("info")} />
      {closed !== null ? (
        <PeriodNotice reason={closed} />
      ) : creations.items.length === 0 ? (
        <p className="py-6 text-[14px] text-dash-ink-2">{t("empty")}</p>
      ) : (
        <div className="overflow-x-auto">
          <div role="table" aria-label={t("tableLabel")} className="min-w-[360px]">
            <div role="row" className={`${HEAD_ROW_CLASS} ${DEPOSIT_COLUMNS}`}>
              <div role="columnheader">{t("pair")}</div>
              <div role="columnheader">{t("creator")}</div>
              <div role="columnheader" className={NUM_CLASS}>
                {t("deposit")}
              </div>
              <div role="columnheader" className={NUM_CLASS}>
                {t("ago")}
              </div>
            </div>
            {/* ⚠️ The rank in the key: two creations of one transaction share signature and time. */}
            {creations.items.map((creation, rank) => (
              <div key={`${creation.signature}-${rank}`} role="row" className={`${ROW_CLASS} ${DEPOSIT_COLUMNS}`}>
                <div role="cell" className="truncate text-dash-ink">
                  {creation.tokenASymbol !== null && creation.tokenBSymbol !== null
                    ? `${creation.tokenASymbol} / ${creation.tokenBSymbol}`
                    : shortenPubkey(creation.poolAddress)}
                </div>
                <div role="cell" className="font-dash-mono text-[12px] whitespace-nowrap text-dash-ink-2">
                  {shortenPubkey(creation.owner)}
                </div>
                <div role="cell" className={`${NUM_CLASS} text-dash-ink`}>
                  {formatUsdCompact(creation.amountUsd)}
                </div>
                <div role="cell" className={`${NUM_CLASS} text-dash-ink-2`}>
                  {formatRelative(creation.at, locale, now) ?? "—"}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}
