/**
 * The four KPI cells, each fetching on its own so a slow route holds only its cell. A failed
 * secondary read (the change, the catalogue) costs only its line, never the main figure.
 */

import type { ReactNode } from "react";
import { getTranslations } from "next-intl/server";

import { ApiClientError, type ApiClientErrorKind } from "@/lib/api/errors";
import {
  fetchPreviousProtocolActivity,
  fetchProtocolActivity,
  fetchProtocols,
  fetchProtocolTvl,
  fetchProtocolTvlHourly,
} from "@/lib/api/server/protocols";
import { formatCount } from "@/lib/format/format-count";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { previousWindowCovered, tvlStart } from "@/lib/overview/availability";
import type { Period } from "@/lib/overview/period";
import { changeRatio, sumUsd } from "@/lib/overview/totals";

import { BlockPlaceholder } from "../block-placeholder";
import { Change, closedReason, KpiCell, Unavailable, utcMoment } from "./overview-parts";
import { MIN_HEIGHT, SPAN } from "./overview-layout";

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
    const starts = latest.value.items.map((i) =>
      tvlStart(hourly.value.items, hourly.value.from, i.protocol),
    );
    const start = starts.some((s) => s === null)
      ? null
      : sumUsd(starts.map((s) => s?.tvlUsd ?? null));
    const ratio = changeRatio(current, start);
    sub =
      ratio !== null ? (
        <Change ratio={ratio} suffix={suffix} />
      ) : start === null ? (
        <Withheld reason={t("unavailable.tvlStart")} suffix={suffix} />
      ) : undefined;
  }

  return <KpiCell label={label} info={info} value={usd(current)} sub={sub} />;
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

  return <KpiCell label={label} info={info} value={usd(current)} sub={sub} />;
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
      value={usd(fees)}
      sub={
        protocolFees === null
          ? undefined
          : t("kpis.feesProtocol", { amount: usd(protocolFees) })
      }
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
    />
  );
}

export async function KpiPlaceholder() {
  return <BlockPlaceholder className={`${SPAN.kpi} ${MIN_HEIGHT.kpi}`} />;
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
  const t = await getTranslations("Dashboard.BlockError");
  return t(err.details.kind);
}

function kpiError(err: unknown, label: string, info: string) {
  if (!(err instanceof ApiClientError)) {
    throw err;
  }
  return <KpiFailed label={label} info={info} kind={err.details.kind} />;
}

async function KpiFailed({
  label,
  info,
  kind,
}: {
  label: string;
  info: string;
  kind: ApiClientErrorKind;
}) {
  const t = await getTranslations("Dashboard.BlockError");
  return (
    <KpiCell
      label={label}
      info={info}
      value="—"
      sub={<span className="text-dash-warn">{t(kind)}</span>}
    />
  );
}
