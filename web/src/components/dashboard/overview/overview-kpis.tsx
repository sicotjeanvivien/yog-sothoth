/**
 * The four KPI cells, each fetching on its own so a slow route holds only its cell. The
 * reads they share are fetched once per render (`lib/api/server/protocols.ts`).
 */

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
import { Change, KpiCell, Unavailable, utcMoment } from "./overview-parts";
import { MIN_HEIGHT, SPAN } from "./overview-layout";

const usd = (value: number | null) =>
  value === null ? "—" : formatUsdCompact(String(value));

export async function TvlKpi({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview");
  const label = t("kpis.tvl");
  const info = t("kpis.info.tvl");

  let latest, hourly;
  try {
    [latest, hourly] = await Promise.all([
      fetchProtocolTvl(),
      fetchProtocolTvlHourly(period),
    ]);
  } catch (err) {
    return kpiError(err, label, info);
  }

  const current = sumUsd(latest.items.map((i) => i.tvlUsd));
  const starts = latest.items.map((i) => tvlStart(hourly.items, hourly.from, i.protocol));
  const start = starts.some((s) => s === null)
    ? null
    : sumUsd(starts.map((s) => s?.tvlUsd ?? null));
  const ratio = changeRatio(current, start);
  const suffix = t("kpis.tvlChange", { period: t(`period.short.${period}`) });

  let sub;
  if (ratio !== null) {
    sub = <Change ratio={ratio} suffix={suffix} />;
  } else if (current !== null && starts.some((s) => s === null)) {
    const firstAt = hourly.items.map((i) => i.at).sort()[0];
    const reason = firstAt
      ? t("unavailable.tvlStart", { since: await utcMoment(firstAt) })
      : t("unavailable.tvlStartUnknown");
    sub = (
      <>
        <Unavailable reason={reason} /> {suffix}
      </>
    );
  }

  return (
    <KpiCell label={label} info={info} value={usd(current)} sub={sub} />
  );
}

export async function VolumeKpi({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview");
  const short = t(`period.short.${period}`);
  const label = t("kpis.volume", { period: short });
  const info = t("kpis.info.volume");

  let now, before;
  try {
    [now, before] = await Promise.all([
      fetchProtocolActivity(period),
      fetchPreviousProtocolActivity(period),
    ]);
  } catch (err) {
    return kpiError(err, label, info);
  }

  const current = sumUsd(now.items.map((i) => i.volumeUsd));
  const suffix = t("kpis.volumeChange", { period: short });

  let sub;
  if (!previousWindowCovered(before)) {
    const reason = t("unavailable.previous", {
      since: before.coveredSince ? await utcMoment(before.coveredSince) : "—",
    });
    sub = (
      <>
        <Unavailable reason={reason} /> {suffix}
      </>
    );
  } else {
    const ratio = changeRatio(current, sumUsd(before.items.map((i) => i.volumeUsd)));
    sub = ratio === null ? undefined : <Change ratio={ratio} suffix={suffix} />;
  }

  return (
    <KpiCell label={label} info={info} value={usd(current)} sub={sub} />
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
  const label = t("kpis.pools");
  const info = t("kpis.info.pools");

  let now, catalogue;
  try {
    [now, catalogue] = await Promise.all([
      fetchProtocolActivity(period),
      fetchProtocols(),
    ]);
  } catch (err) {
    return kpiError(err, label, info);
  }

  const active = now.items.every((i) => i.activePools !== null) && now.items.length > 0
    ? now.items.reduce((sum, i) => sum + (i.activePools ?? 0), 0)
    : null;
  const births = now.items.reduce((sum, i) => sum + i.births, 0);
  const observed = catalogue.reduce((sum, p) => sum + p.poolsSeen, 0);

  return (
    <KpiCell
      label={label}
      info={info}
      value={formatCount(active)}
      sub={
        <>
          {t("kpis.poolsObserved", { observed: formatCount(observed) })}
          {now.items.length > 0 && (
            <>
              {" · "}
              {t("kpis.poolsBorn", {
                births: formatCount(births),
                period: t(`period.short.${period}`),
              })}
            </>
          )}
        </>
      }
    />
  );
}

export async function KpiPlaceholder() {
  return <BlockPlaceholder className={`${SPAN.kpi} ${MIN_HEIGHT.kpi}`} />;
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
