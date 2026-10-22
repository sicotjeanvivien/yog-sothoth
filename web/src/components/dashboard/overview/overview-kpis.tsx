/**
 * The KPI cells, each fetching on its own so a slow route holds only its cell; swaps and
 * rotation are the advanced mode's. A failed secondary read costs only its line.
 */

import type { ReactNode } from "react";
import { getLocale, getTranslations } from "next-intl/server";

import { ApiClientError } from "@/lib/api/errors";
import {
  fetchPreviousProtocolActivity,
  fetchProtocolActivity,
  fetchProtocols,
  fetchProtocolTvl,
  fetchProtocolTvlHourly,
} from "@/lib/api/server/protocols";
import { volumeCoverage } from "@/lib/coverage/volume-coverage";
import { formatCount, formatCountCompact, formatRate } from "@/lib/format/format-count";
import { formatTimes } from "@/lib/format/format-percent";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { previousWindowCovered, tvlStart } from "@/lib/overview/availability";
import type { Period } from "@/lib/overview/period";
import {
  changeRatio,
  discoveredTvl,
  rotation,
  sumUsd,
  swapsPerSecond,
  tvlChange,
} from "@/lib/overview/totals";

import { Change, closedReason, KpiCell, Unavailable, utcMoment } from "./overview-parts";
import { KpiFailedLine, KpiSkeleton, lateStamp } from "./overview-states";
import { VolumeCoverageMark } from "./volume-coverage-mark";

const usd = (value: number | null) =>
  value === null ? "—" : formatUsdCompact(String(value));

export async function TvlKpi({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview");
  const label = t("kpis.tvl");
  const info = t("kpis.info.tvl");
  const suffix = t("kpis.tvlChange", { period: t(`period.short.${period}`) });

  const [latest, hourly] = await Promise.allSettled([
    fetchProtocolTvl(),
    fetchProtocolTvlHourly(period),
  ]);
  if (latest.status === "rejected") {
    return kpiError(latest.reason, label, info);
  }

  const current = sumUsd(latest.value.items.map((i) => i.tvlUsd));
  let sub: ReactNode;
  if (hourly.status === "rejected") {
    sub = <Withheld reason={await failure(hourly.reason)} suffix={suffix} />;
  } else if (current !== null) {
    const starts = latest.value.items.map((i) => tvlStart(hourly.value.items, i, period));
    const start = starts.some((s) => s === null)
      ? null
      : sumUsd(starts.map((s) => s?.tvlUsd ?? null));
    const discovered = discoveredTvl(latest.value.items, period);
    const ratio = tvlChange(current, start, discovered);
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
      stamp={await lateStamp(latest.value.asOf)}
    />
  );
}

export async function VolumeKpi({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview");
  const short = t(`period.short.${period}`);
  const label = t("kpis.volume", { period: short });
  const info = t("kpis.info.volume");
  const suffix = t("kpis.volumeChange", { period: short });

  const [now, before] = await Promise.allSettled([
    fetchProtocolActivity(period),
    fetchPreviousProtocolActivity(period),
  ]);
  if (now.status === "rejected") {
    return kpiError(now.reason, label, info);
  }
  const closed = await closedReason(period, now.value);
  if (closed !== null) {
    return <KpiCell label={label} info={info} value={<Unavailable reason={closed} />} />;
  }

  const current = sumUsd(now.value.items.map((i) => i.volumeUsd));
  let sub: ReactNode;
  if (before.status === "rejected") {
    sub = <Withheld reason={await failure(before.reason)} suffix={suffix} />;
  } else if (!previousWindowCovered(before.value)) {
    const since = before.value.coveredSince;
    const reason = t("unavailable.previous", {
      since: since === null ? "—" : await utcMoment(since),
    });
    sub = <Withheld reason={reason} suffix={suffix} />;
  } else {
    const ratio = changeRatio(current, sumUsd(before.value.items.map((i) => i.volumeUsd)));
    sub = ratio === null ? undefined : <Change ratio={ratio} suffix={suffix} />;
  }

  return (
    <KpiCell
      label={label}
      info={info}
      value={await valued(usd(current), now.value.items)}
      sub={sub}
      stamp={await lateStamp(now.value.asOf)}
    />
  );
}

export async function FeesKpi({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview");
  const label = t("kpis.fees", { period: t(`period.short.${period}`) });
  const info = t("kpis.info.fees");

  let now;
  try {
    now = await fetchProtocolActivity(period);
  } catch (err) {
    return kpiError(err, label, info);
  }
  const closed = await closedReason(period, now);
  if (closed !== null) {
    return <KpiCell label={label} info={info} value={<Unavailable reason={closed} />} />;
  }

  const fees = sumUsd(now.items.map((i) => i.feesUsd));
  const protocolFees = sumUsd(now.items.map((i) => i.protocolFeesUsd));

  return (
    <KpiCell
      label={label}
      info={info}
      value={await valued(usd(fees), now.items)}
      sub={
        protocolFees === null
          ? undefined
          : t("kpis.feesProtocol", { amount: usd(protocolFees) })
      }
      stamp={await lateStamp(now.asOf)}
    />
  );
}

export async function PoolsKpi({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview");
  const short = t(`period.short.${period}`);
  const label = t("kpis.pools");
  const info = t("kpis.info.pools");

  const [now, catalogue] = await Promise.allSettled([
    fetchProtocolActivity(period),
    fetchProtocols(),
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
    items.length > 0 && items.every((i) => i.activePools !== null)
      ? items.reduce((sum, i) => sum + (i.activePools ?? 0), 0)
      : null;
  const births = items.reduce((sum, i) => sum + i.births, 0);
  const observed =
    catalogue.status === "fulfilled" ? (
      t("kpis.poolsObserved", {
        observed: formatCount(catalogue.value.reduce((sum, p) => sum + p.poolsSeen, 0)),
      })
    ) : (
      <>
        {t("kpis.poolsObservedBefore")} <Unavailable reason={await failure(catalogue.reason)} />{" "}
        {t("kpis.poolsObservedAfter")}
      </>
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
          {t("kpis.poolsBorn", { births: formatCount(births), period: short })}
        </>
      }
      stamp={await lateStamp(now.value.asOf)}
    />
  );
}

export async function SwapsKpi({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview");
  const label = t("kpis.swaps", { period: t(`period.short.${period}`) });
  const info = t("kpis.info.swaps");

  let now;
  try {
    now = await fetchProtocolActivity(period);
  } catch (err) {
    return kpiError(err, label, info, true);
  }
  const closed = await closedReason(period, now);
  if (closed !== null) {
    return (
      <KpiCell label={label} info={info} value={<Unavailable reason={closed} />} advanced />
    );
  }

  const swaps =
    now.items.length === 0 ? null : now.items.reduce((sum, i) => sum + i.swapCount, 0);

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
      stamp={await lateStamp(now.asOf)}
      advanced
    />
  );
}

export async function RotationKpi({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview");
  const locale = await getLocale();
  const label = t("kpis.rotation", { period: t(`period.short.${period}`) });
  const info = t("kpis.info.rotation");

  let now, tvl;
  try {
    [now, tvl] = await Promise.all([fetchProtocolActivity(period), fetchProtocolTvl()]);
  } catch (err) {
    return kpiError(err, label, info, true);
  }
  const closed = await closedReason(period, now);
  if (closed !== null) {
    return (
      <KpiCell label={label} info={info} value={<Unavailable reason={closed} />} advanced />
    );
  }

  const ratio = rotation(
    sumUsd(now.items.map((i) => i.volumeUsd)),
    sumUsd(tvl.items.map((i) => i.tvlUsd)),
  );

  return (
    <KpiCell
      label={label}
      info={info}
      value={await valued(ratio === null ? "—" : formatTimes(ratio, locale), now.items)}
      sub={t("kpis.rotationSub")}
      stamp={await lateStamp(now.asOf)}
      advanced
    />
  );
}

export async function KpiPlaceholder({ advanced = false }: { advanced?: boolean }) {
  return <KpiSkeleton advanced={advanced} />;
}

/** A figure built on the volume, with the mark when it leaves out pool-hours not valued. */
async function valued(
  figure: string,
  items: readonly { poolHours: number; poolHoursValued: number }[],
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
        label={(priced, total) => t("mark", { priced, total })}
      />
    </>
  );
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

function kpiError(err: unknown, label: string, info: string, advanced = false) {
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
