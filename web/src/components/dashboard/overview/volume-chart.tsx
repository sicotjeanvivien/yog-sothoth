/**
 * Volume by 5-minute tranche (1H), by hour (24H) or by 6- or 24-hour slice (7D, 30D), all protocols summed: the page's main
 * chart, and the fees under it in the advanced mode. The server lays the slots and words every
 * reading; `VolumeBars` only tracks the bar under the pointer.
 */

import { getLocale, getTranslations } from "next-intl/server";

import { ApiClientError } from "@/lib/api/errors";
import type { ProtocolSwapActivityBucketResponse } from "@/lib/api/schema/zod.gen";
import { fetchSwapsSeries } from "@/lib/api/server/protocols";
import { formatUtcMoment, type FormatLocale } from "@/lib/format/date";
import { formatProtocolLabel } from "@/lib/format/format-protocol";
import { formatUsdCompact } from "@/lib/format/format-usd";
import {
  axisOf,
  CHART_PX,
  daysAgoOf,
  hourOf,
  readsClock,
  roundTicks,
  slotsOf,
  type Slot,
} from "@/lib/overview/chart";
import type { Period } from "@/lib/overview/period";
import { sumUsd } from "@/lib/overview/totals";

import { BlockHeading, closedReason, PeriodNotice } from "./overview-parts";
import { ADVANCED_ONLY, CELL_CLASS, MIN_HEIGHT, SPAN } from "./overview-layout";
import { BlockFailed, BlockSkeleton } from "./overview-states";
import { VolumeBars } from "./volume-bars";

const SECTION_CLASS = `${CELL_CLASS} ${SPAN.main} ${MIN_HEIGHT.volume}`;

const TITLE = { "1h": "titleFiveMinutes", "24h": "titleHourly", "7d": "titleSixHours", "30d": "titleDaily" } as const;

const INFO = { "1h": "infoFiveMinutes", "24h": "infoHourly", "7d": "infoSixHours", "30d": "infoDaily" } as const;

const FEES = { "1h": "feesFiveMinutes", "24h": "feesHourly", "7d": "feesSixHours", "30d": "feesDaily" } as const;

export async function VolumeChart({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview.volume");
  const tPeriod = await getTranslations("Dashboard.Overview.period");
  const locale = (await getLocale()) as FormatLocale;
  const title = t(TITLE[period], {
    period: tPeriod(`short.${period}`),
  });

  let series;
  try {
    series = await fetchSwapsSeries("all", period);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section aria-label={title} className={SECTION_CLASS}>
          <BlockHeading title={title} info={t(INFO[period])} />
          <BlockFailed kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  const closed = await closedReason(period, series);
  const slotsFor = (pick: (bar: ProtocolSwapActivityBucketResponse) => string | null) =>
    slotsOf(
      series,
      series.items.map((bar) => ({ from: bar.from, value: pick(bar) })),
    );
  const slots = slotsFor((bar) => bar.volumeUsd);
  const feeSlots = slotsFor((bar) => bar.feesUsd);

  const daysAgo = (days: number) => (days === 0 ? t("today") : t("daysAgo", { days }));
  const dayLabel = (i: number) => daysAgo(daysAgoOf(slots, i));
  const slotName = (slot: Slot, i: number) =>
    !readsClock(period)
      ? `${dayLabel(i)} · ${formatUtcMoment(slot.from, locale)} → ${formatUtcMoment(slot.to, locale)}`
      : `${hourOf(slot.from)} UTC`;

  const drawn = (values: readonly Slot[], top: number, noValue: string) => {
    const max = Math.max(0, ...values.map((s) => s.value ?? 0));
    const scale = (v: number) => (max > 0 ? (v / max) * top : 0);
    return {
      bars: values.map((slot, i) => ({
        height: slot.value === null ? null : Math.max(slot.value > 0 ? 2 : 0, scale(slot.value)),
        reading: `${slotName(slot, i)} — ${
          slot.value === null ? noValue : formatUsdCompact(String(slot.value))
        }`,
      })),
      ticks: roundTicks(max).map((value) => ({
        bottom: Math.round(scale(value)),
        label: formatUsdCompact(String(value)),
      })),
    };
  };
  const volume = drawn(slots, CHART_PX.volume.top, t("noValue"));
  const fees = drawn(feeSlots, CHART_PX.fees.top, t("feesNoValue"));
  const feesTotal = feeSlots.some((s) => s.value !== null)
    ? String(feeSlots.reduce((sum, s) => sum + (s.value ?? 0), 0))
    : null;
  const axis = axisOf(period, slots, daysAgo);

  // Like the API's totals, a bar without a valued volume adds nothing.
  const protocols = [...new Set(series.items.map((i) => i.protocol))].map((protocol) => {
    const total = sumUsd(
      series.items.flatMap((i) =>
        i.protocol === protocol && i.volumeUsd !== null ? [i.volumeUsd] : [],
      ),
    );
    return {
      name: formatProtocolLabel(protocol),
      total: formatUsdCompact(total === null ? null : String(total)),
    };
  });

  return (
    <section aria-label={title} className={SECTION_CLASS}>
      <BlockHeading title={title} info={t(INFO[period])}>
        {closed === null && (
          <div className="flex flex-wrap gap-x-4 gap-y-1.5 text-[12px] text-dash-ink-2">
            {protocols.map((p) => (
              <span key={p.name}>
                <span
                  aria-hidden="true"
                  className="mr-1.5 inline-block h-2.5 w-2.5 bg-dash-accent align-[-1px]"
                />
                {p.name} <span className="font-dash-mono tabular-nums">{p.total}</span>
              </span>
            ))}
          </div>
        )}
      </BlockHeading>
      {closed !== null ? (
        <PeriodNotice reason={closed} />
      ) : (
        <>
          <VolumeBars
            bars={volume.bars}
            ticks={volume.ticks}
            axis={axis}
            height={CHART_PX.volume.height}
            invite={t("invite")}
            label={t("chartLabel")}
          />
          <div className={`mt-[22px] ${ADVANCED_ONLY.block}`}>
            <VolumeBars
              bars={fees.bars}
              ticks={fees.ticks}
              axis={axis}
              height={CHART_PX.fees.height}
              caption={t(FEES[period])}
              invite={t("feesTotal", { total: formatUsdCompact(feesTotal) })}
              label={t("feesChartLabel")}
            />
          </div>
        </>
      )}
    </section>
  );
}

export async function VolumeChartPlaceholder({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview");
  return (
    <BlockSkeleton
      title={t(`volume.${TITLE[period]}`, {
        period: t(`period.short.${period}`),
      })}
      span={SPAN.main}
      minHeight={MIN_HEIGHT.volume}
      shape="chart"
    />
  );
}
