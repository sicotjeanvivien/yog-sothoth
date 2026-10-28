/**
 * The tab 02 of a protocol's page: where its volume comes from — each bar split between the
 * rotation runs and the rest — then the runs of the period, and the largest of them. The
 * server words every reading; the chart only tracks the bar under the pointer.
 */

import { Suspense, type ReactNode } from "react";
import { getLocale, getTranslations } from "next-intl/server";

import {
  CELL_CLASS,
  GRID_CLASS,
  HEAD_ROW_CLASS,
  LABEL_CLASS,
  NUM_CLASS,
  ROW_CLASS,
} from "@/components/dashboard/overview/overview-layout";
import {
  BlockHeading,
  closedReason,
  PeriodNotice,
  utcMoment,
} from "@/components/dashboard/overview/overview-parts";
import { BlockFailed, BlockSkeleton } from "@/components/dashboard/overview/overview-states";
import { ApiClientError } from "@/lib/api/errors";
import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";
import {
  fetchLargestRotationRuns,
  fetchRotationRunsSeries,
  fetchRotationRunTotals,
} from "@/lib/api/server/protocols";
import { formatUtcMoment, type FormatLocale } from "@/lib/format/date";
import { formatCount } from "@/lib/format/format-count";
import { formatPercent } from "@/lib/format/format-percent";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { pairLabel } from "@/lib/format/pubkey";
import { axisOf, daysAgoOf, hourOf, roundTicks } from "@/lib/overview/chart";
import type { Period } from "@/lib/overview/period";
import {
  durationOf,
  splitSinceOf,
  splitSlotsOf,
  splitTotalsOf,
} from "@/lib/protocol/volume";

import { SplitBars, type SplitDetail } from "../split-bars";
import { SPLIT_TONE, type SplitTone } from "./split-tones";

/** The chart's height, and its tallest bar a little under it for the top mark's label. */
const CHART_PX = { height: 122, top: 112 } as const;

const RUNS_SHOWN = 5;

const PER_SLOT = { "24h": "perHour", "7d": "perSixHours", "30d": "perDay" } as const;

const CHART_LABEL = {
  "24h": "chartLabelHourly",
  "7d": "chartLabelSixHours",
  "30d": "chartLabelDaily",
} as const;

const SOURCES_SPAN = "col-span-12 dash-wide:col-span-8";

const STATS_SPAN = "col-span-12 dash-wide:col-span-4";

const RUNS_SPAN = "col-span-12";

const SOURCES_CLASS = `${CELL_CLASS} ${SOURCES_SPAN} min-h-[380px]`;

const STATS_CLASS = `${CELL_CLASS} ${STATS_SPAN} min-h-[380px]`;

const RUNS_CLASS = `${CELL_CLASS} ${RUNS_SPAN} min-h-[262px]`;

const RUN_COLUMNS =
  "grid-cols-[minmax(0,1.6fr)_10.5rem_minmax(0,0.7fr)_minmax(0,0.9fr)_minmax(0,0.8fr)]";

type TabProps = { period: Period; protocol: ProtocolResponse };

export async function VolumeTab({ period, protocol }: TabProps) {
  const t = await getTranslations("Dashboard.Protocol.volume");
  const tPeriod = await getTranslations("Dashboard.Overview.period");
  const short = tPeriod(`short.${period}`);

  return (
    <div className={GRID_CLASS}>
      <Suspense
        key={`sources-${period}`}
        fallback={
          <BlockSkeleton title={t("title")} span={SOURCES_SPAN} minHeight="min-h-[380px]" shape="chart" />
        }
      >
        <VolumeSources period={period} protocol={protocol} />
      </Suspense>
      <Suspense
        key={`stats-${period}`}
        fallback={
          <BlockSkeleton
            title={t("stats.title", { period: short })}
            span={STATS_SPAN}
            minHeight="min-h-[380px]"
            shape={{ rows: 3 }}
          />
        }
      >
        <RunStats period={period} protocol={protocol} />
      </Suspense>
      <Suspense
        key={`runs-${period}`}
        fallback={
          <BlockSkeleton
            title={t("runs.title", { period: short })}
            span={RUNS_SPAN}
            minHeight="min-h-[262px]"
            shape={{ rows: RUNS_SHOWN }}
          />
        }
      >
        <LargestRuns period={period} protocol={protocol} />
      </Suspense>
    </div>
  );
}

async function VolumeSources({ period, protocol }: TabProps) {
  const t = await getTranslations("Dashboard.Protocol.volume");
  const tVolume = await getTranslations("Dashboard.Overview.volume");
  const tPeriod = await getTranslations("Dashboard.Overview.period");
  const locale = (await getLocale()) as FormatLocale;
  const title = t("title");

  let runs;
  try {
    runs = await fetchRotationRunsSeries(protocol, period);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section aria-label={title} className={SOURCES_CLASS}>
          <BlockHeading title={title} info={t("info")} />
          <BlockFailed kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  const closed = await closedReason(period, runs);
  const usd = (value: number | null) => formatUsdCompact(value === null ? null : String(value));
  const share = (part: number, whole: number) =>
    whole > 0 ? formatPercent(String(part / whole), locale) : "—";

  const slots = runs.asOf === null ? [] : splitSlotsOf(period, runs.asOf, runs.items);
  const totals = splitTotalsOf(slots);
  const max = Math.max(0, ...slots.map((s) => s.total ?? 0));
  const scale = (v: number) => (max > 0 ? (v / max) * CHART_PX.top : 0);
  const daysAgo = (days: number) => (days === 0 ? tVolume("today") : tVolume("daysAgo", { days }));
  const moment = (date: Date) => formatUtcMoment(date, locale) ?? "—";

  const detailOf = (
    when: string,
    inRuns: number,
    outside: number,
    unsplit: number,
  ): SplitDetail<SplitTone> => {
    // ⚠️ The shares over the split volume only, as the legend's: an unsplit part is not outside.
    const split = inRuns + outside;
    return {
      when,
      total: usd(split + unsplit),
      rows: [
        ...(inRuns + outside > 0 || unsplit === 0
          ? [
              { tone: "in" as const, name: t("inRuns"), value: usd(inRuns), share: share(inRuns, split) },
              { tone: "out" as const, name: t("outside"), value: usd(outside), share: share(outside, split) },
            ]
          : []),
        ...(unsplit > 0
          ? [{ tone: "unsplit" as const, name: t("unsplit"), value: usd(unsplit), share: "" }]
          : []),
      ],
    };
  };

  const bars = slots.map((slot, i) => {
    const when =
      period !== "24h"
        ? `${daysAgo(daysAgoOf(period, slots.length, i))} · ${moment(slot.from)} → ${moment(slot.to)}`
      : `${hourOf(slot.from)} UTC`;
    if (slot.total === null) {
      return { segments: [], detail: { when, total: tVolume("noValue"), rows: [] } };
    }
    const height = (v: number) => Math.max(v > 0 ? 1 : 0, Math.round(scale(v)));
    if (slot.inRuns === null || slot.outside === null) {
      return {
        segments: [{ tone: "unsplit" as const, height: height(slot.total) }],
        detail: detailOf(when, 0, 0, slot.total),
      };
    }
    return {
      segments: [
        { tone: "in" as const, height: height(slot.inRuns) },
        { tone: "out" as const, height: height(slot.outside) },
      ].filter((s) => s.height > 0),
      detail: detailOf(when, slot.inRuns, slot.outside, 0),
    };
  });

  const measuredSince = splitSinceOf(runs.items);
  const partly = totals.unsplit > 0;
  const legend = (
    <div className="flex flex-wrap gap-x-4 gap-y-1.5 text-[12px] text-dash-ink-2">
      <span>
        <span aria-hidden="true" className={`mr-1.5 inline-block h-2.5 w-2.5 align-[-1px] ${SPLIT_TONE.in}`} />
        {t("inRuns")} <span className="font-dash-mono tabular-nums">{usd(totals.inRuns)}</span>
      </span>
      <span>
        <span aria-hidden="true" className={`mr-1.5 inline-block h-2.5 w-2.5 align-[-1px] ${SPLIT_TONE.out}`} />
        {t("outside")} <span className="font-dash-mono tabular-nums">{usd(totals.outside)}</span>
      </span>
      <span>
        {t("share")}{" "}
        <span className="font-dash-mono tabular-nums">
          {totals.share === null ? "—" : formatPercent(String(totals.share), locale)}
        </span>
      </span>
    </div>
  );

  return (
    <section aria-label={title} className={SOURCES_CLASS}>
      <BlockHeading title={title} info={t("info")}>
        {closed === null && legend}
      </BlockHeading>
      {closed !== null ? (
        <PeriodNotice reason={closed} />
      ) : (
        <>
          <p className="mb-2 flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
            <span className={LABEL_CLASS}>{t(PER_SLOT[period])}</span>
            {partly && (
              <span className="text-[12px] text-dash-ink-3">
                {measuredSince === null
                  ? t("notMeasured")
                  : t("measuredSince", { since: await utcMoment(measuredSince) })}
              </span>
            )}
          </p>
          <SplitBars
            bars={bars}
            period={detailOf(
              t("periodTotal", { period: tPeriod(`short.${period}`) }),
              totals.inRuns,
              totals.outside,
              totals.unsplit,
            )}
            ticks={roundTicks(max).map((value) => ({
              bottom: Math.round(scale(value)),
              label: usd(value),
            }))}
            axis={axisOf(period, slots, daysAgo)}
            height={CHART_PX.height}
            label={t(CHART_LABEL[period])}
            tones={SPLIT_TONE}
          />
        </>
      )}
    </section>
  );
}

async function RunStats({ period, protocol }: TabProps) {
  const t = await getTranslations("Dashboard.Protocol.volume");
  const tPeriod = await getTranslations("Dashboard.Overview.period");
  const locale = (await getLocale()) as FormatLocale;
  const title = t("stats.title", { period: tPeriod(`short.${period}`) });

  let totals;
  try {
    totals = await fetchRotationRunTotals(protocol, period);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section aria-label={title} className={STATS_CLASS}>
          <BlockHeading title={title} info={t("stats.info")} />
          <BlockFailed kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  const mine = totals.items.find((i) => i.protocol === protocol);
  const closed = (await closedReason(period, totals)) ?? t("notMeasured");
  const stat = (label: string, big: string, sub: ReactNode, last = false) => (
    <div className={`flex flex-col gap-1 py-3 ${last ? "" : "border-b border-dash-rule"}`}>
      <span className={LABEL_CLASS}>{label}</span>
      <span className="font-dash-mono text-[24px] leading-[1.1] font-medium tracking-[-0.02em] text-dash-ink tabular-nums">
        {big}
      </span>
      <span className="text-[12px] text-dash-ink-2">{sub}</span>
    </div>
  );

  return (
    <section aria-label={title} className={STATS_CLASS}>
      <BlockHeading title={title} info={t("stats.info")} />
      {mine === undefined ? (
        <PeriodNotice reason={closed} />
      ) : mine.runs === 0 ? (
        <p className="py-6 text-[14px] text-dash-ink-2">{t("stats.none")}</p>
      ) : (
        <>
          {stat(
            t("stats.runs"),
            formatCount(mine.runs),
            <>
              {t("stats.onPools", { pools: formatCount(mine.pools), count: mine.pools })}
              {period === "24h" && (
                <>
                  {" · "}
                  <span className="text-dash-ink">
                    <span aria-hidden="true" className="mr-1.5 inline-block h-1.5 w-1.5 bg-dash-up align-[1px]" />
                    {t("stats.inProgress", { count: formatCount(mine.inProgress) })}
                  </span>
                </>
              )}
            </>,
          )}
          {stat(
            t("stats.duration"),
            mine.medianDurationSeconds === null ? "—" : durationOf(mine.medianDurationSeconds),
            mine.p90DurationSeconds === null
              ? t("stats.median")
              : t("stats.medianAndP90", { p90: durationOf(mine.p90DurationSeconds) }),
          )}
          {stat(
            t("stats.firstHour"),
            Number.parseFloat(mine.volumeUsd) > 0
              ? formatPercent(
                  String(Number.parseFloat(mine.firstHourVolumeUsd) / Number.parseFloat(mine.volumeUsd)),
                  locale,
                )
              : "—",
            t("stats.firstHourSub"),
            true,
          )}
        </>
      )}
    </section>
  );
}

async function LargestRuns({ period, protocol }: TabProps) {
  const t = await getTranslations("Dashboard.Protocol.volume");
  const tPeriod = await getTranslations("Dashboard.Overview.period");
  const locale = (await getLocale()) as FormatLocale;
  const title = t("runs.title", { period: tPeriod(`short.${period}`) });

  let runs;
  try {
    runs = await fetchLargestRotationRuns(protocol, period, RUNS_SHOWN);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section aria-label={title} className={RUNS_CLASS}>
          <BlockHeading title={title} info={t("runs.info")} />
          <BlockFailed kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  const closed = await closedReason(period, runs);

  return (
    <section aria-label={title} className={RUNS_CLASS}>
      <BlockHeading title={title} info={t("runs.info")} />
      {closed !== null ? (
        <PeriodNotice reason={closed} />
      ) : runs.items.length === 0 ? (
        <p className="py-6 text-[14px] text-dash-ink-2">{t("runs.empty")}</p>
      ) : (
        <div className="overflow-x-auto">
          <div role="table" aria-label={t("runs.tableLabel")} className="min-w-[560px]">
            <div role="row" className={`${HEAD_ROW_CLASS} ${RUN_COLUMNS}`}>
              <div role="columnheader">{t("runs.pool")}</div>
              <div role="columnheader">{t("runs.start")}</div>
              <div role="columnheader" className={NUM_CLASS}>
                {t("runs.duration")}
              </div>
              <div role="columnheader" className={NUM_CLASS}>
                {t("runs.volume")}
              </div>
              <div role="columnheader" className={NUM_CLASS}>
                {t("runs.state")}
              </div>
            </div>
            {runs.items.map((run) => (
                <div key={`${run.poolAddress}-${run.startedAt}`} role="row" className={`${ROW_CLASS} ${RUN_COLUMNS}`}>
                  <div role="cell" className="truncate text-dash-ink">
                    {pairLabel(run)}
                  </div>
                  <div role="cell" className="font-dash-mono text-[12px] whitespace-nowrap text-dash-ink-2">
                    {formatUtcMoment(run.startedAt, locale) ?? "—"}
                  </div>
                  <div role="cell" className={`${NUM_CLASS} text-dash-ink`}>
                    {durationOf((Date.parse(run.endedAt) - Date.parse(run.startedAt)) / 1000)}
                  </div>
                  <div role="cell" className={`${NUM_CLASS} text-dash-ink`}>
                    {formatUsdCompact(run.volumeUsd)}
                  </div>
                  <div role="cell" className={NUM_CLASS}>
                    {run.extinct ? (
                      <span className="text-dash-ink-2">{t("runs.done")}</span>
                    ) : (
                      <span className="text-dash-ink">
                        <span aria-hidden="true" className="mr-1.5 inline-block h-1.5 w-1.5 bg-dash-up align-[1px]" />
                        {t("runs.live")}
                      </span>
                    )}
                  </div>
                </div>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}
