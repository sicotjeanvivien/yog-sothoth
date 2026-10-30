/**
 * The tab 04 of a protocol's page: the pools born over the period, bar by bar, and those whose
 * first deposit is worth the most. The server words every reading; the chart only tracks the bar
 * under the pointer.
 */

import { Suspense } from "react";
import { getLocale, getTranslations } from "next-intl/server";

import {
  ADVANCED_ONLY,
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
import {
  fetchLargestLiquidityMoves,
  fetchLiquidityMoveTotals,
  fetchPoolCreationsSeries,
  fetchPositionChangeTotals,
} from "@/lib/api/server/protocols";
import { formatCount } from "@/lib/format/format-count";
import { formatRelative, formatUtcMoment, type FormatLocale } from "@/lib/format/date";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { pairLabel, shortenPubkey } from "@/lib/format/pubkey";
import { axisOf, daysAgoOf, hourOf, readsClock, roundTicks } from "@/lib/overview/chart";
import type { Period } from "@/lib/overview/period";
import { birthSlotsOf, birthTotalOf, lifeCountOf, lockKey } from "@/lib/protocol/pools";

import { SplitBars, type SplitDetail } from "../split-bars";

/** The chart's height, and its tallest bar a little under it for the top mark's label. */
const CHART_PX = { height: 122, top: 112 } as const;

const DEPOSITS_SHOWN = 5;

/** ⚠️ Three side by side in the advanced mode: the life cycle below would lengthen the page. */
const BIRTHS_SPAN = "col-span-12 dash-wide:col-span-7 adv:dash-wide:col-span-3";

const DEPOSITS_SPAN = "col-span-12 dash-wide:col-span-5";

const LIFE_SPAN = `col-span-12 dash-wide:col-span-4 ${ADVANCED_ONLY.block}`;

const LIFE_CLASS = `${CELL_CLASS} ${LIFE_SPAN} min-h-[262px]`;

const LIFE_COLUMNS = "grid-cols-[minmax(0,1fr)_auto]";

/** The life cycle's rows, each with the event it counts; the owners are the liquidity moves'. */
const LIFE_ROWS = [
  { key: "opened", event: "create_position" },
  { key: "vestingLocks", event: "lock_position" },
  { key: "permanentLocks", event: "permanent_lock_position" },
  { key: "splits", event: "split_position" },
  { key: "closed", event: "close_position" },
] as const;

const BIRTHS_CLASS = `${CELL_CLASS} ${BIRTHS_SPAN} min-h-[262px]`;

const DEPOSITS_CLASS = `${CELL_CLASS} ${DEPOSITS_SPAN} min-h-[262px]`;

const DEPOSIT_COLUMNS = "grid-cols-[minmax(0,1.2fr)_minmax(0,0.9fr)_auto_auto_auto]";

const BIRTH_TONE = { births: "bg-dash-flow-create" } as const;

const PER_SLOT = {
  "1h": "perFiveMinutes",
  "24h": "perHour",
  "7d": "perSixHours",
  "30d": "perDay",
} as const;

const CHART_LABEL = {
  "1h": "chartLabelFiveMinutes",
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
      <Suspense
        key={`life-${period}`}
        fallback={
          <BlockSkeleton
            title={t("life.title", { period: tPeriod(`short.${period}`) })}
            span={LIFE_SPAN}
            minHeight="min-h-[262px]"
            shape={{ rows: 6 }}
          />
        }
      >
        <LifeCycle period={period} protocol={protocol} />
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
      !readsClock(period)
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
            ticks={roundTicks(max)
              .filter(Number.isInteger)
              .map((value) => ({
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
    creations = await fetchLargestLiquidityMoves(protocol, period, DEPOSITS_SHOWN, "create");
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
          <div role="table" aria-label={t("tableLabel")} className="min-w-[420px]">
            <div role="row" className={`${HEAD_ROW_CLASS} ${DEPOSIT_COLUMNS}`}>
              <div role="columnheader">{t("pair")}</div>
              <div role="columnheader">{t("creator")}</div>
              <div role="columnheader">{t("lock")}</div>
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
                  {pairLabel(creation)}
                </div>
                <div role="cell" className="font-dash-mono text-[12px] whitespace-nowrap text-dash-ink-2">
                  {shortenPubkey(creation.owner)}
                </div>
                <div role="cell" className="font-dash-mono text-[12px] whitespace-nowrap text-dash-ink-2">
                  {t(`locks.${lockKey(creation.lock)}`)}
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

/** The positions' life cycle over the period: one count per event, and the distinct LPs. */
async function LifeCycle({ period, protocol }: TabProps) {
  const t = await getTranslations("Dashboard.Protocol.pools.life");
  const tPeriod = await getTranslations("Dashboard.Overview.period");
  const title = t("title", { period: tPeriod(`short.${period}`) });

  let changes, moves;
  try {
    [changes, moves] = await Promise.all([
      fetchPositionChangeTotals(protocol, period),
      fetchLiquidityMoveTotals(protocol, period),
    ]);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section aria-label={title} className={LIFE_CLASS}>
          <BlockHeading title={title} info={t("info")} />
          <BlockFailed kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  const rows = [
    ...LIFE_ROWS.map(({ key, event }) => ({
      key,
      event,
      count: lifeCountOf(changes.items, (r) => r[key]),
    })),
    {
      key: "owners" as const,
      event: "owner (liquidity)",
      count: lifeCountOf(moves.items, (r) => r.owners),
    },
  ];

  return (
    <section aria-label={title} className={LIFE_CLASS}>
      <BlockHeading title={title} info={t("info")} />
      <div role="table" aria-label={title}>
        <div role="row" className={`${HEAD_ROW_CLASS} ${LIFE_COLUMNS}`}>
          <div role="columnheader">{t("step")}</div>
          <div role="columnheader" className={NUM_CLASS}>
            {t("count")}
          </div>
        </div>
        {rows.map((row) => (
          <div key={row.key} role="row" className={`${ROW_CLASS} ${LIFE_COLUMNS}`}>
            <div role="cell" className="min-w-0">
              <div className="truncate text-dash-ink">{t(`rows.${row.key}`)}</div>
              <div className="truncate font-dash-mono text-[11px] text-dash-ink-3">{row.event}</div>
            </div>
            <div role="cell" className={`${NUM_CLASS} text-dash-ink`}>
              {formatCount(row.count)}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}
