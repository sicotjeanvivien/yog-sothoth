/**
 * Volume by hour (24H) or by 24-hour slice (7D, 30D), all protocols summed: the page's main
 * chart. The server lays the slots and words every reading; `VolumeBars` only tracks the bar
 * under the pointer.
 */

import { getLocale, getTranslations } from "next-intl/server";

import { BlockError } from "@/components/dashboard/block-error";
import { ApiClientError } from "@/lib/api/errors";
import { fetchProtocolActivitySeries } from "@/lib/api/server/protocols";
import { formatUtcMoment, type FormatLocale } from "@/lib/format/date";
import { formatProtocolLabel } from "@/lib/format/format-protocol";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { CHART_TOP_PX, roundTicks, slotsOf, type Slot } from "@/lib/overview/chart";
import type { Period } from "@/lib/overview/period";
import { sumUsd } from "@/lib/overview/totals";

import { BlockPlaceholder } from "../block-placeholder";
import { BlockHeading, closedReason, PeriodNotice } from "./overview-parts";
import { CELL_CLASS, MIN_HEIGHT, SPAN } from "./overview-layout";
import { VolumeBars } from "./volume-bars";

const SECTION_CLASS = `${CELL_CLASS} ${SPAN.main} ${MIN_HEIGHT.volume}`;

/** The step between axis labels; the last slot always has one, the one before it none. */
const AXIS_EVERY: Record<Period, number> = { "24h": 6, "7d": 1, "30d": 7 };

export async function VolumeChart({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview.volume");
  const tPeriod = await getTranslations("Dashboard.Overview.period");
  const locale = (await getLocale()) as FormatLocale;
  const daily = period !== "24h";
  const title = t(daily ? "titleDaily" : "titleHourly", {
    period: tPeriod(`short.${period}`),
  });

  let series;
  try {
    series = await fetchProtocolActivitySeries(period);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section className={SECTION_CLASS}>
          <BlockError title={title} kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  const closed = await closedReason(period, series);
  const slots = series.asOf === null ? [] : slotsOf(period, series.asOf, series.items);
  const max = Math.max(0, ...slots.map((s) => s.value ?? 0));
  const scale = (v: number) => (max > 0 ? (v / max) * CHART_TOP_PX : 0);

  const dayLabel = (i: number) => {
    const days = slots.length - 1 - i;
    return days === 0 ? t("today") : t("daysAgo", { days });
  };
  const slotName = (slot: Slot, i: number) =>
    daily
      ? `${dayLabel(i)} · ${formatUtcMoment(slot.from, locale)} → ${formatUtcMoment(slot.to, locale)}`
      : `${hourOf(slot.from)} UTC`;

  const bars = slots.map((slot, i) => ({
    height: slot.value === null ? null : Math.max(slot.value > 0 ? 2 : 0, scale(slot.value)),
    reading: `${slotName(slot, i)} — ${
      slot.value === null ? t("noValue") : formatUsdCompact(String(slot.value))
    }`,
  }));
  const ticks = roundTicks(max).map((value) => ({
    bottom: Math.round(scale(value)),
    label: formatUsdCompact(String(value)),
  }));
  const every = AXIS_EVERY[period];
  const last = slots.length - 1;
  const axis = slots.flatMap((slot, i) =>
    i === last || (i % every === 0 && i <= last - every / 2)
      ? [daily ? dayLabel(i) : i === last ? `${hourOf(slot.from)} UTC` : hourOf(slot.from)]
      : [],
  );

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
      <BlockHeading title={title} info={t(daily ? "infoDaily" : "infoHourly")}>
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
        <VolumeBars
          bars={bars}
          ticks={ticks}
          axis={axis}
          invite={t("invite")}
          label={t("chartLabel")}
        />
      )}
    </section>
  );
}

export async function VolumeChartPlaceholder() {
  return <BlockPlaceholder className={`${SPAN.main} ${MIN_HEIGHT.volume}`} />;
}

function hourOf(date: Date): string {
  return `${String(date.getUTCHours()).padStart(2, "0")}:00`;
}
