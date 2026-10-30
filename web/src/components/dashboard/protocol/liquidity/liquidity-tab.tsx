/**
 * The tab 01 of a protocol's page: how its liquidity moves — the TVL hour by hour, the
 * liquidity added and removed per bar — then, in the advanced mode, its largest moves. The
 * server words every reading; the charts only track the point under the pointer.
 */

import { Suspense } from "react";
import { getLocale, getTranslations } from "next-intl/server";

import {
  ADVANCED_ONLY,
  CELL_CLASS,
  GRID_CLASS,
  HEAD_ROW_CLASS,
  NUM_CLASS,
  ROW_CLASS,
} from "@/components/dashboard/overview/overview-layout";
import {
  BlockHeading,
  closedReason,
  PeriodNotice,
  Unavailable,
} from "@/components/dashboard/overview/overview-parts";
import { BlockFailed, BlockSkeleton } from "@/components/dashboard/overview/overview-states";
import { ApiClientError } from "@/lib/api/errors";
import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";
import {
  fetchLargestLiquidityMoves,
  fetchLiquidityMoveTotals,
  fetchLiquiditySeries,
  fetchSwapTotals,
  fetchTvlSeries,
} from "@/lib/api/server/protocols";
import { formatRelative, formatUtcMoment, type FormatLocale } from "@/lib/format/date";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { pairLabel, shortenPubkey } from "@/lib/format/pubkey";
import { axisOf, daysAgoOf, hourOf, readsClock, roundTicks } from "@/lib/overview/chart";
import type { Period } from "@/lib/overview/period";
import { CURVE_BOX, curveOf, flowSlotsOf } from "@/lib/protocol/liquidity";

import { FlowBars } from "./flow-bars";
import { TvlCurve } from "./tvl-curve";

/** The flows' half height, and their tallest bar a little under it for the top mark's label. */
const FLOW_PX = { half: 64, top: 58 } as const;

const MOVES_SHOWN = 5;

const FLOWS = {
  "1h": "flowsFiveMinutes",
  "24h": "flowsHourly",
  "7d": "flowsSixHours",
  "30d": "flowsDaily",
} as const;

/** A creation enters as an addition does: its own colour, the same sign. */
const SIGN = {
  create: { sign: "+ ", tone: "text-dash-flow-create" },
  add: { sign: "+ ", tone: "text-dash-up" },
  remove: { sign: "− ", tone: "text-dash-down" },
} as const;

/** ⚠️ Side by side in the advanced mode: the moves below the chart would lengthen the page. */
const FLOWS_SPAN = "col-span-12 adv:dash-wide:col-span-8";

const MOVES_SPAN = `col-span-12 dash-wide:col-span-4 ${ADVANCED_ONLY.block}`;

const FLOWS_CLASS = `${CELL_CLASS} ${FLOWS_SPAN} min-h-[420px]`;

const MOVES_CLASS = `${CELL_CLASS} ${MOVES_SPAN} min-h-[262px]`;

const MOVE_COLUMNS = "grid-cols-[minmax(0,0.9fr)_minmax(0,1.2fr)_auto_auto]";

type TabProps = { period: Period; protocol: ProtocolResponse };

export async function LiquidityTab({ period, protocol }: TabProps) {
  const t = await getTranslations("Dashboard.Protocol.liquidity");
  const tPeriod = await getTranslations("Dashboard.Overview.period");
  const short = tPeriod(`short.${period}`);

  return (
    <div className={GRID_CLASS}>
      <Suspense
        key={`flows-${period}`}
        fallback={
          <BlockSkeleton
            title={t("title")}
            span={FLOWS_SPAN}
            minHeight="min-h-[420px]"
            shape="chart"
          />
        }
      >
        <LiquidityFlows period={period} protocol={protocol} />
      </Suspense>
      <Suspense
        key={`moves-${period}`}
        fallback={
          <BlockSkeleton
            title={t("moves.title", { period: short })}
            span={MOVES_SPAN}
            minHeight="min-h-[262px]"
            shape={{ rows: MOVES_SHOWN }}
          />
        }
      >
        <LargestMoves period={period} protocol={protocol} />
      </Suspense>
    </div>
  );
}

async function LiquidityFlows({ period, protocol }: TabProps) {
  const t = await getTranslations("Dashboard.Protocol.liquidity");
  const tVolume = await getTranslations("Dashboard.Overview.volume");
  const locale = (await getLocale()) as FormatLocale;
  const title = t("title");

  const swapsRead = fetchSwapTotals(protocol, period).catch((err: unknown) => {
    if (err instanceof ApiClientError) {
      return null;
    }
    throw err;
  });
  // ⚠️ The TVL is read once an hour: the last hour has no curve, and its route refuses `1h`.
  const tvlRead = period === "1h" ? null : fetchTvlSeries(protocol, period);
  let tvl, series, totals, swaps;
  try {
    [tvl, series, totals, swaps] = await Promise.all([
      tvlRead,
      fetchLiquiditySeries(protocol, period),
      fetchLiquidityMoveTotals(protocol, period),
      swapsRead,
    ]);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section aria-label={title} className={FLOWS_CLASS}>
          <BlockHeading title={title} info={t("info")} />
          <BlockFailed kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  // ⚠️ The swaps' span, not the liquidity's: a period its liquidity covers in part still draws.
  // Without the swaps, the liquidity's own: the block does not fall with a read it can spare.
  const closed = await closedReason(period, swaps ?? series);
  const usd = (value: number | null) => formatUsdCompact(value === null ? null : String(value));
  const signed = (value: number) => `${value < 0 ? "−" : "+"}${usd(Math.abs(value))}`;

  const liquidity = totals.items[0] ?? null;
  const notMeasured = <Unavailable reason={t("notMeasured")} />;
  const legend = (
    <div className="flex flex-wrap gap-x-4 gap-y-1.5 text-[12px] text-dash-ink-2">
      <span>
        <span aria-hidden="true" className="mr-1.5 inline-block h-2.5 w-2.5 bg-dash-flow-create align-[-1px]" />
        {t("created")}{" "}
        <span className="font-dash-mono tabular-nums">
          {liquidity === null ? notMeasured : formatUsdCompact(liquidity.createdUsd)}
        </span>
      </span>
      <span>
        <span aria-hidden="true" className="mr-1.5 inline-block h-2.5 w-2.5 bg-dash-up align-[-1px]" />
        {t("added")}{" "}
        <span className="font-dash-mono tabular-nums">
          {liquidity === null ? notMeasured : formatUsdCompact(liquidity.addedUsd)}
        </span>
      </span>
      <span>
        <span aria-hidden="true" className="mr-1.5 inline-block h-2.5 w-2.5 bg-dash-flow-out align-[-1px]" />
        {t("removed")}{" "}
        <span className="font-dash-mono tabular-nums">
          {liquidity === null ? notMeasured : formatUsdCompact(liquidity.removedUsd)}
        </span>
      </span>
      <span>
        {t("net")}{" "}
        <span className="font-dash-mono tabular-nums">
          {liquidity === null
            ? notMeasured
            : signed(
                Number.parseFloat(liquidity.createdUsd) +
                  Number.parseFloat(liquidity.addedUsd) -
                  Number.parseFloat(liquidity.removedUsd),
              )}
        </span>
      </span>
    </div>
  );

  const curve = tvl === null ? null : curveOf(tvl.items);
  const moment = (date: Date) => formatUtcMoment(date, locale) ?? "—";
  const curveBlock =
    tvl === null ? (
      <div className="mt-[26px] flex h-[140px] items-center justify-center border-b border-dash-rule bg-dash-surface-2 px-4 text-center text-[14px] text-dash-ink-3">
        {t("curveNotApplicable")}
      </div>
    ) : curve === null ? (
      <p className="py-6 text-[14px] text-dash-ink-2">— {t("noCurve")}</p>
    ) : (
      <TvlCurve
        line={curve.line}
        area={curve.area}
        marks={curve.coords.map((c) => (c.x / CURVE_BOX.width) * 100)}
        readings={curve.points.map((p, i) => {
          const previous = curve.points[i - 1];
          const hours =
            previous === undefined ? 0 : Math.round((p.at.getTime() - previous.at.getTime()) / 3_600_000);
          const change =
            previous === undefined
              ? ""
              : ` · ${
                  hours === 1
                    ? t("perHour", { change: signed(p.value - previous.value) })
                    : t("perHours", { change: signed(p.value - previous.value), hours })
                }`;
          return `${moment(p.at)} — ${t("tvlAt", { tvl: usd(p.value) })}${change}`;
        })}
        rest={t("tvlNow", { tvl: usd(curve.points[curve.points.length - 1]!.value) })}
        low={usd(curve.low)}
        high={usd(curve.high)}
        last={usd(curve.points[curve.points.length - 1]!.value)}
        label={t("curveLabel", { low: usd(curve.low), high: usd(curve.high) })}
      />
    );

  const slots = series.asOf === null ? [] : flowSlotsOf(period, series.asOf, series.items);
  const max = Math.max(
    0,
    ...slots.flatMap((s) => [(s.created ?? 0) + (s.added ?? 0), s.removed ?? 0]),
  );
  const scale = (v: number) => (max > 0 ? (v / max) * FLOW_PX.top : 0);
  const height = (v: number | null) => (v === null ? null : Math.max(v > 0 ? 2 : 0, scale(v)));
  const daysAgo = (days: number) => (days === 0 ? tVolume("today") : tVolume("daysAgo", { days }));
  const bars = slots.map((slot, i) => {
    const name =
      !readsClock(period)
        ? `${daysAgo(daysAgoOf(period, slots.length, i))} · ${moment(slot.from)} → ${moment(slot.to)}`
      : `${hourOf(slot.from)} UTC`;
    const reading =
      slot.created === null || slot.added === null || slot.removed === null
        ? `${name} — ${t("notMeasuredShort")}`
        : `${name} — ${t("flowReading", {
            created: usd(slot.created),
            added: usd(slot.added),
            removed: usd(slot.removed),
            net: signed(slot.created + slot.added - slot.removed),
          })}`;
    return {
      created: height(slot.created),
      added: height(slot.added),
      down: height(slot.removed),
      reading,
    };
  });

  return (
    <section aria-label={title} className={FLOWS_CLASS}>
      <BlockHeading title={title} info={t("info")}>
        {closed === null && legend}
      </BlockHeading>
      {closed !== null ? (
        <PeriodNotice reason={closed} />
      ) : (
        <>
          {curve !== null && (
            <p className="mb-1 font-dash-mono text-[11px] font-medium tracking-[0.12em] text-dash-ink-3 uppercase">
              {t("curveCaption", { low: usd(curve.low), high: usd(curve.high) })}
            </p>
          )}
          {curveBlock}
          <p className="mt-[18px] mb-1 font-dash-mono text-[11px] font-medium tracking-[0.12em] text-dash-ink-3 uppercase">
            {t(FLOWS[period])}
          </p>
          <FlowBars
            bars={bars}
            ticks={roundTicks(max).map((value) => ({
              offset: Math.round(scale(value)),
              label: usd(value),
            }))}
            axis={axisOf(period, slots, daysAgo)}
            half={FLOW_PX.half}
            invite={tVolume("invite")}
            label={t("flowsLabel")}
          />
        </>
      )}
    </section>
  );
}

async function LargestMoves({ period, protocol }: TabProps) {
  const t = await getTranslations("Dashboard.Protocol.liquidity");
  const tPeriod = await getTranslations("Dashboard.Overview.period");
  const locale = (await getLocale()) as FormatLocale;
  const title = t("moves.title", { period: tPeriod(`short.${period}`) });

  let moves, totals;
  try {
    [moves, totals] = await Promise.all([
      fetchLargestLiquidityMoves(protocol, period, MOVES_SHOWN),
      fetchLiquidityMoveTotals(protocol, period),
    ]);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section aria-label={title} className={MOVES_CLASS}>
          <BlockHeading title={title} info={t("moves.info")} />
          <BlockFailed kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  const measured = totals.items.length > 0;
  const now = new Date();

  return (
    <section aria-label={title} className={MOVES_CLASS}>
      <BlockHeading title={title} info={t("moves.info")} />
      {!measured ? (
        <PeriodNotice reason={t("notMeasured")} />
      ) : moves.items.length === 0 ? (
        <p className="py-6 text-[14px] text-dash-ink-2">{t("moves.empty")}</p>
      ) : (
        <div className="overflow-x-auto">
          <div role="table" aria-label={t("moves.tableLabel")} className="min-w-[360px]">
            <div role="row" className={`${HEAD_ROW_CLASS} ${MOVE_COLUMNS}`}>
              <div role="columnheader">{t("moves.owner")}</div>
              <div role="columnheader">{t("moves.pool")}</div>
              <div role="columnheader" className={NUM_CLASS}>
                {t("moves.amount")}
              </div>
              <div role="columnheader" className={NUM_CLASS}>
                {t("moves.ago")}
              </div>
            </div>
            {/* ⚠️ The rank in the key: two moves of one transaction share signature and time. */}
            {moves.items.map((move, rank) => (
              <div key={`${move.signature}-${rank}`} role="row" className={`${ROW_CLASS} ${MOVE_COLUMNS}`}>
                <div role="cell" className="font-dash-mono text-[12px] whitespace-nowrap text-dash-ink-2">
                  {shortenPubkey(move.owner)}
                </div>
                <div role="cell" className="truncate text-dash-ink">
                  {pairLabel(move)}
                </div>
                <div role="cell" className={`${NUM_CLASS} text-dash-ink`}>
                  <span aria-hidden="true" className={`font-bold ${SIGN[move.kind].tone}`}>
                    {SIGN[move.kind].sign}
                  </span>
                  <span className="sr-only">{t(`moves.${move.kind}`)} </span>
                  {formatUsdCompact(move.amountUsd)}
                </div>
                <div role="cell" className={`${NUM_CLASS} text-dash-ink-2`}>
                  {formatRelative(move.at, locale, now) ?? "—"}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}
