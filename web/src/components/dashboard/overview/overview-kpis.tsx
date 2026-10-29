/**
 * The KPI cells, each fetching on its own so a slow route holds only its cell, and adding up
 * the protocols its `scope`'s routes return; swaps and rotation are the advanced mode's. A
 * failed secondary read costs only its line.
 */

import type { ReactNode } from "react";
import { getLocale, getTranslations } from "next-intl/server";

import { ApiClientError } from "@/lib/api/errors";
import {
  fetchPoolCreationTotals,
  fetchPreviousSwapTotals,
  fetchProtocol,
  fetchProtocols,
  fetchSwapTotals,
  fetchTvl,
  fetchTvlTotals,
} from "@/lib/api/server/protocols";
import type { Scope } from "@/lib/api/server/scope";
import { volumeCoverage } from "@/lib/coverage/volume-coverage";
import { formatCount, formatCountCompact, formatRate } from "@/lib/format/format-count";
import { formatTimes } from "@/lib/format/format-percent";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { previousWindowAdjoins, previousWindowCovered } from "@/lib/overview/availability";
import { coverageUnit, DEFAULT_PERIOD, type Period } from "@/lib/overview/period";
import { changeRatio, rotation, shareOf, sumUsd, swapsPerSecond } from "@/lib/overview/totals";

import { Change, closedReason, KpiCell, Unavailable, utcMoment } from "./overview-parts";
import { KpiFailedLine, KpiSkeleton, lateStamp } from "./overview-states";
import { VolumeCoverageMark } from "./volume-coverage-mark";

export const usd = (value: number | null) =>
  value === null ? "—" : formatUsdCompact(String(value));

export async function TvlKpi({ period, scope }: { period: Period; scope: Scope }) {
  if (period === "1h") {
    return <LastTvlKpi scope={scope} />;
  }
  const t = await getTranslations("Dashboard.Overview");
  const label = t("kpis.tvl");
  const info = t("kpis.info.tvl");
  const suffix = t("kpis.tvlChange", { period: t(`period.short.${period}`) });

  let totals;
  try {
    totals = await fetchTvlTotals(scope, period);
  } catch (err) {
    return kpiError(err, label, info);
  }

  const rows = totals.items;
  const current = sumUsd(rows.map((r) => r.end.tvlUsd));
  let sub: ReactNode;
  if (current !== null) {
    const start = sumUsd(rows.map((r) => r.start?.tvlUsd ?? null));
    const discovered = sumUsd(rows.map((r) => r.discoveredTvlUsd));
    const ratio = shareOf(sumUsd(rows.map((r) => r.changeUsd)), start);
    sub =
      ratio !== null ? (
        <>
          <Change ratio={ratio} suffix={suffix} />
          {" · "}
          {t("kpis.tvlDiscovered", { amount: usd(discovered) })}
        </>
      ) : start === null ? (
        <Withheld reason={t("unavailable.tvlStart")} suffix={suffix} />
      ) : discovered === null ? (
        <Withheld reason={t("unavailable.tvlDiscovered")} suffix={suffix} />
      ) : undefined;
  }

  return (
    <KpiCell
      label={label}
      info={info}
      value={usd(current)}
      sub={sub}
      stamp={await lateStamp(totals.asOf, period)}
    />
  );
}

/** The TVL is a stock: over the last hour, its last reading alone, without a change. */
async function LastTvlKpi({ scope }: { scope: Scope }) {
  const t = await getTranslations("Dashboard.Overview");
  const label = t("kpis.tvl");
  const info = t("kpis.info.tvlLast");

  let reading;
  try {
    reading = await fetchTvl(scope);
  } catch (err) {
    return kpiError(err, label, info);
  }
  const at = reading.asOf;

  return (
    <KpiCell
      label={label}
      info={info}
      value={usd(sumUsd(reading.items.map((i) => i.tvlUsd)))}
      sub={at === null ? undefined : t("kpis.tvlAt", { at: await utcMoment(at) })}
      // ⚠️ The TVL is read by the hour whatever the period: its lag is a frozen hour's.
      stamp={await lateStamp(at, DEFAULT_PERIOD)}
    />
  );
}

export async function VolumeKpi({ period, scope }: { period: Period; scope: Scope }) {
  const t = await getTranslations("Dashboard.Overview");
  const short = t(`period.short.${period}`);
  const label = t("kpis.volume", { period: short });
  const info = t(period === "1h" ? "kpis.info.volumeLastHour" : "kpis.info.volume");
  const suffix = t("kpis.volumeChange", { period: short });

  const [now, before] = await Promise.allSettled([
    fetchSwapTotals(scope, period),
    fetchPreviousSwapTotals(scope, period),
  ]);
  if (now.status === "rejected") {
    return kpiError(now.reason, label, info);
  }
  const closed = await closedReason(period, now.value);
  if (closed !== null) {
    return <KpiCell label={label} info={info} value={<Unavailable reason={closed} />} />;
  }

  const items = now.value.items;
  const current = sumUsd(items.map((i) => i.volumeUsd));
  let sub: ReactNode;
  if (before.status === "rejected") {
    sub = <Withheld reason={await failure(before.reason)} suffix={suffix} />;
  } else if (!previousWindowCovered(before.value)) {
    const since = before.value.coveredSince;
    const reason = t("unavailable.previous", {
      since: since === null ? "—" : await utcMoment(since),
    });
    sub = <Withheld reason={reason} suffix={suffix} />;
  } else if (!previousWindowAdjoins(before.value, now.value)) {
    sub = <Withheld reason={t("unavailable.previousMoved")} suffix={suffix} />;
  } else {
    const ratio = changeRatio(current, sumUsd(before.value.items.map((i) => i.volumeUsd)));
    sub = ratio === null ? undefined : <Change ratio={ratio} suffix={suffix} />;
  }

  return (
    <KpiCell
      label={label}
      info={info}
      value={await valued(usd(current), items, period)}
      sub={sub}
      stamp={await lateStamp(now.value.asOf, period)}
    />
  );
}

export async function FeesKpi({ period, scope }: { period: Period; scope: Scope }) {
  const t = await getTranslations("Dashboard.Overview");
  const label = t("kpis.fees", { period: t(`period.short.${period}`) });
  const info = t("kpis.info.fees");

  let now;
  try {
    now = await fetchSwapTotals(scope, period);
  } catch (err) {
    return kpiError(err, label, info);
  }
  const closed = await closedReason(period, now);
  if (closed !== null) {
    return <KpiCell label={label} info={info} value={<Unavailable reason={closed} />} />;
  }

  const items = now.items;
  const fees = sumUsd(items.map((i) => i.feesUsd));
  const protocolFees = sumUsd(items.map((i) => i.protocolFeesUsd));

  return (
    <KpiCell
      label={label}
      info={info}
      value={await valued(usd(fees), items, period)}
      sub={
        protocolFees === null
          ? undefined
          : t("kpis.feesProtocol", { amount: usd(protocolFees) })
      }
      stamp={await lateStamp(now.asOf, period)}
    />
  );
}

export async function PoolsKpi({ period, scope }: { period: Period; scope: Scope }) {
  const t = await getTranslations("Dashboard.Overview");
  const short = t(`period.short.${period}`);
  const label = t("kpis.pools");
  const info = t("kpis.info.pools");

  const [now, creations, seen] = await Promise.allSettled([
    fetchSwapTotals(scope, period),
    fetchPoolCreationTotals(scope, period),
    poolsSeen(scope),
  ]);
  if (now.status === "rejected") {
    return kpiError(now.reason, label, info);
  }
  const closed = await closedReason(period, now.value);
  if (closed !== null) {
    return <KpiCell label={label} info={info} value={<Unavailable reason={closed} />} />;
  }

  const items = now.value.items;
  const active =
    items.length > 0 && items.every((i) => i.pools !== null)
      ? items.reduce((sum, i) => sum + (i.pools ?? 0), 0)
      : null;
  const observed =
    seen.status === "fulfilled" ? (
      t("kpis.poolsObserved", { observed: formatCount(seen.value) })
    ) : (
      <>
        {t("kpis.poolsObservedBefore")} <Unavailable reason={await failure(seen.reason)} />{" "}
        {t("kpis.poolsObservedAfter")}
      </>
    );
  const born =
    creations.status === "fulfilled" ? (
      t("kpis.poolsBorn", {
        births: formatCount(creations.value.items.reduce((sum, i) => sum + i.pools, 0)),
        period: short,
      })
    ) : (
      <Unavailable reason={await failure(creations.reason)} />
    );

  return (
    <KpiCell
      label={label}
      info={info}
      value={formatCount(active)}
      sub={
        <>
          {observed}
          {" · "}
          {born}
        </>
      }
      stamp={await lateStamp(now.value.asOf, period)}
    />
  );
}

export async function SwapsKpi({ period, scope }: { period: Period; scope: Scope }) {
  const t = await getTranslations("Dashboard.Overview");
  const label = t("kpis.swaps", { period: t(`period.short.${period}`) });
  const info = t("kpis.info.swaps");

  let now;
  try {
    now = await fetchSwapTotals(scope, period);
  } catch (err) {
    return kpiError(err, label, info, true);
  }
  const closed = await closedReason(period, now);
  if (closed !== null) {
    return (
      <KpiCell label={label} info={info} value={<Unavailable reason={closed} />} advanced />
    );
  }

  const swaps = now.items.length === 0 ? null : now.items.reduce((sum, i) => sum + i.swaps, 0);

  return (
    <KpiCell
      label={label}
      info={info}
      value={formatCountCompact(swaps)}
      sub={
        swaps === null
          ? undefined
          : t("kpis.swapsRate", { rate: formatRate(swapsPerSecond(swaps, period)) })
      }
      stamp={await lateStamp(now.asOf, period)}
      advanced
    />
  );
}

export async function RotationKpi({ period, scope }: { period: Period; scope: Scope }) {
  const t = await getTranslations("Dashboard.Overview");
  const locale = await getLocale();
  const label = t("kpis.rotation", { period: t(`period.short.${period}`) });
  const info = t("kpis.info.rotation");

  let now, tvl;
  try {
    [now, tvl] = await Promise.all([fetchSwapTotals(scope, period), fetchTvl(scope)]);
  } catch (err) {
    return kpiError(err, label, info, true);
  }
  const closed = await closedReason(period, now);
  if (closed !== null) {
    return (
      <KpiCell label={label} info={info} value={<Unavailable reason={closed} />} advanced />
    );
  }

  const items = now.items;
  const ratio = rotation(
    sumUsd(items.map((i) => i.volumeUsd)),
    sumUsd(tvl.items.map((i) => i.tvlUsd)),
  );

  return (
    <KpiCell
      label={label}
      info={info}
      value={await valued(ratio === null ? "—" : formatTimes(ratio, locale), items, period)}
      sub={t("kpis.rotationSub")}
      stamp={await lateStamp(now.asOf, period)}
      advanced
    />
  );
}

export async function KpiPlaceholder({ advanced = false }: { advanced?: boolean }) {
  return <KpiSkeleton advanced={advanced} />;
}

/** A figure built on the volume, with the mark when it leaves out pool-hours not valued. */
export async function valued(
  figure: string,
  items: readonly { poolHours: number; poolHoursValued: number }[],
  period: Period,
): Promise<ReactNode> {
  const t = await getTranslations("Dashboard.Overview.coverage");
  const coverage = volumeCoverage({
    poolHours: items.reduce((sum, i) => sum + i.poolHours, 0),
    poolHoursValued: items.reduce((sum, i) => sum + i.poolHoursValued, 0),
  });
  return (
    <>
      {figure}
      {/* ⚠️ A place to break: in a narrow cell the mark goes under the figure, not over the edge. */}
      <wbr />
      <VolumeCoverageMark
        coverage={coverage}
        label={(priced, total) => t(`${coverageUnit(period)}.mark`, { priced, total })}
      />
    </>
  );
}

/** The pools seen by `scope`'s protocols, from the catalogue. */
async function poolsSeen(scope: Scope): Promise<number> {
  if (scope !== "all") {
    return (await fetchProtocol(scope)).poolsSeen;
  }
  return (await fetchProtocols()).items.reduce((sum, p) => sum + p.poolsSeen, 0);
}

/** "— ⓘ over 7d": a secondary figure withheld, its reason one click away. */
function Withheld({ reason, suffix }: { reason: string; suffix: string }) {
  return (
    <>
      <Unavailable reason={reason} /> {suffix}
    </>
  );
}

/** The sentence of a failed read; anything but an `ApiClientError` is re-thrown. */
async function failure(err: unknown): Promise<string> {
  if (!(err instanceof ApiClientError)) {
    throw err;
  }
  const t = await getTranslations("Dashboard.Overview.error");
  return t(err.details.kind);
}

export function kpiError(err: unknown, label: string, info: string, advanced = false) {
  if (!(err instanceof ApiClientError)) {
    throw err;
  }
  return <KpiFailed label={label} info={info} advanced={advanced} />;
}

/** "—", "Unavailable for now" and a retry; the cause sits in the blocks' error boxes. */
function KpiFailed({ label, info, advanced }: { label: string; info: string; advanced: boolean }) {
  return (
    <KpiCell
      label={label}
      info={info}
      value={<span className="text-dash-ink-3">—</span>}
      sub={<KpiFailedLine />}
      advanced={advanced}
    />
  );
}
