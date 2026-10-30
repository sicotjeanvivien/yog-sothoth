/**
 * The tab 03 of a protocol's page: where its fees go — the LPs, the protocol, the referral — over
 * the period and bar by bar, then what the LPs claimed. The server words every reading; the chart
 * only tracks the bar under the pointer.
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
import { BlockHeading, closedReason, PeriodNotice } from "@/components/dashboard/overview/overview-parts";
import { BlockFailed, BlockSkeleton } from "@/components/dashboard/overview/overview-states";
import { ApiClientError } from "@/lib/api/errors";
import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";
import {
  fetchPositionFeeClaimTotals,
  fetchRewardClaimTotals,
  fetchSwapsSeries,
} from "@/lib/api/server/protocols";
import { formatCount } from "@/lib/format/format-count";
import { formatUtcMoment, type FormatLocale } from "@/lib/format/date";
import { formatPercent } from "@/lib/format/format-percent";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { axisOf, daysAgoOf, hourOf, readsClock, roundTicks } from "@/lib/overview/chart";
import type { Period } from "@/lib/overview/period";
import {
  FEE_PARTS,
  feeSharesOf,
  feeSlotsOf,
  feeSumOf,
  feeTotalsOf,
  type FeePart,
  type FeeParts,
} from "@/lib/protocol/fees";

import { SplitBars, type SplitDetail } from "../split-bars";
import { FEE_TONE } from "./fee-tones";

/** The chart's height, and its tallest bar a little under it for the top mark's label. */
const CHART_PX = { height: 122, top: 112 } as const;

const SPLIT_SPAN = "col-span-12 dash-wide:col-span-8";

const CLAIMS_SPAN = "col-span-12 dash-wide:col-span-4";

const CLAIMS_CLASS = `${CELL_CLASS} ${CLAIMS_SPAN} min-h-[380px]`;

const SPLIT_CLASS = `${CELL_CLASS} ${SPLIT_SPAN} min-h-[380px]`;

const DESTINATION_COLUMNS = "grid-cols-[minmax(0,1fr)_96px_64px]";

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
      <Suspense
        key={`claims-${period}`}
        fallback={
          <BlockSkeleton
            title={t("claims.title")}
            span={CLAIMS_SPAN}
            minHeight="min-h-[380px]"
            shape={{ rows: 3 }}
          />
        }
      >
        <LpClaims period={period} protocol={protocol} />
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

  const closed = await closedReason(swaps);
  const usd = (value: number | null) => formatUsdCompact(value === null ? null : String(value));
  const percent = (ratio: number | null | undefined) =>
    ratio === null || ratio === undefined ? "—" : formatPercent(String(ratio), locale);

  const slots = feeSlotsOf(swaps);
  const totals = feeTotalsOf(slots);
  const shares = totals === null ? null : feeSharesOf(totals);
  const max = Math.max(0, ...slots.map(feeSumOf));
  const scale = (v: number) => (max > 0 ? (v / max) * CHART_PX.top : 0);
  const daysAgo = (days: number) => (days === 0 ? tVolume("today") : tVolume("daysAgo", { days }));
  const moment = (date: Date) => formatUtcMoment(date, locale) ?? "—";

  const detailOf = (when: string, parts: FeeParts | null): SplitDetail<FeePart> => {
    const split = parts === null ? null : feeSharesOf(parts);
    return {
      when,
      total: usd(parts === null ? null : feeSumOf(parts)),
      rows: FEE_PARTS.map((part) => ({
        tone: part,
        name: t(`parts.${part}`),
        value: usd(parts === null ? null : parts[part]),
        share: percent(split?.[part]),
      })),
    };
  };

  const bars = slots.map((slot, i) => {
    const when =
      !readsClock(period)
        ? `${daysAgo(daysAgoOf(slots, i))} · ${moment(slot.from)} → ${moment(slot.to)}`
        : `${hourOf(slot.from)} UTC`;
    if (FEE_PARTS.every((part) => slot[part] === null)) {
      return { segments: [], detail: { when, total: t("noValue"), rows: [] } };
    }
    const height = (v: number | null) => (v === null ? 0 : Math.max(v > 0 ? 1 : 0, Math.round(scale(v))));
    return {
      segments: FEE_PARTS.map((part) => ({ tone: part, height: height(slot[part]) })).filter(
        (s) => s.height > 0,
      ),
      detail: detailOf(when, slot),
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
                  {usd(totals === null ? null : totals[part])}
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
                {t("total", { total: usd(totals === null ? null : feeSumOf(totals)) })}
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

async function LpClaims({ period, protocol }: TabProps) {
  const t = await getTranslations("Dashboard.Protocol.fees.claims");
  const tPeriod = await getTranslations("Dashboard.Overview.period");
  const title = t("title");
  const short = tPeriod(`short.${period}`);

  let fees, rewards;
  try {
    [fees, rewards] = await Promise.all([
      fetchPositionFeeClaimTotals(protocol, period),
      fetchRewardClaimTotals(protocol, period),
    ]);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section aria-label={title} className={CLAIMS_CLASS}>
          <BlockHeading title={title} info={t("info")} />
          <BlockFailed kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  const closed = await closedReason(fees);
  const feeClaims = fees.items.find((i) => i.protocol === protocol);
  const rewardClaims = rewards.items.find((i) => i.protocol === protocol);

  const stat = (label: string, big: string, sub: ReactNode, last = false) => (
    <div className={`flex flex-col gap-1 py-3 ${last ? "" : "border-b border-dash-rule"}`}>
      <span className={LABEL_CLASS}>{label}</span>
      <span className="font-dash-mono text-[24px] leading-[1.1] font-medium tracking-[-0.02em] text-dash-ink tabular-nums">
        {big}
      </span>
      <span className="text-[12px] text-dash-ink-2">{sub}</span>
    </div>
  );
  const claimed = (
    totals: { claims: number; claimsValued: number; amountUsd: string; owners: number } | undefined,
    event: string,
  ) => (
    <>
      {totals === undefined ? "—" : t("addresses", { owners: formatCount(totals.owners), count: totals.owners })}
      {totals !== undefined && totals.claimsValued < totals.claims && (
        <>
          {" · "}
          {t("unvalued", {
            n: formatCount(totals.claims - totals.claimsValued),
            count: totals.claims - totals.claimsValued,
          })}
        </>
      )}
      {" · "}
      <span className="font-dash-mono text-[11px] text-dash-ink-3">{event}</span>
    </>
  );

  return (
    <section aria-label={title} className={CLAIMS_CLASS}>
      <BlockHeading title={title} info={t("info")} />
      {closed !== null ? (
        <PeriodNotice reason={closed} />
      ) : (
        <>
          {stat(
            t("feesClaimed", { period: short }),
            formatUsdCompact(feeClaims?.amountUsd ?? null),
            claimed(feeClaims, "claim_position_fee"),
          )}
          {stat(
            t("rewardsClaimed", { period: short }),
            formatUsdCompact(rewardClaims?.amountUsd ?? null),
            claimed(rewardClaims, "claim_reward"),
            true,
          )}
        </>
      )}
    </section>
  );
}
