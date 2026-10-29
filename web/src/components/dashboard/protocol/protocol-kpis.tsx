/**
 * The KPI cells a protocol's page has and the Overview has not: its fees with their effective
 * rate, and in the advanced mode the protocol's fees with their share. The others are the
 * Overview's, on the protocol's routes.
 */

import { getLocale, getTranslations } from "next-intl/server";

import { closedReason, KpiCell, Unavailable } from "@/components/dashboard/overview/overview-parts";
import { kpiError, usd, valued } from "@/components/dashboard/overview/overview-kpis";
import { lateStamp } from "@/components/dashboard/overview/overview-states";
import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";
import { fetchSwapTotals } from "@/lib/api/server/protocols";
import { formatFeeRate, formatPercent } from "@/lib/format/format-percent";
import type { FrozenPeriod } from "@/lib/overview/period";
import { shareOf, sumUsd } from "@/lib/overview/totals";

type KpiProps = { period: FrozenPeriod; protocol: ProtocolResponse };

export async function FeesRateKpi({ period, protocol }: KpiProps) {
  const t = await getTranslations("Dashboard.Protocol.kpis");
  const tOverview = await getTranslations("Dashboard.Overview");
  const locale = await getLocale();
  const label = tOverview("kpis.fees", { period: tOverview(`period.short.${period}`) });
  const info = t("info.fees");

  let now;
  try {
    now = await fetchSwapTotals(protocol, period);
  } catch (err) {
    return kpiError(err, label, info);
  }
  const closed = await closedReason(period, now);
  if (closed !== null) {
    return <KpiCell label={label} info={info} value={<Unavailable reason={closed} />} />;
  }

  const items = now.items;
  const fees = sumUsd(items.map((i) => i.feesUsd));
  const rate = shareOf(fees, sumUsd(items.map((i) => i.volumeUsd)));

  return (
    <KpiCell
      label={label}
      info={info}
      value={await valued(usd(fees), items, period)}
      sub={rate === null ? undefined : t("feesRate", { rate: formatFeeRate(rate, locale) })}
      stamp={await lateStamp(now.asOf)}
    />
  );
}

export async function ProtocolFeesKpi({ period, protocol }: KpiProps) {
  const t = await getTranslations("Dashboard.Protocol.kpis");
  const tOverview = await getTranslations("Dashboard.Overview");
  const locale = await getLocale();
  const label = t("protocolFees", { period: tOverview(`period.short.${period}`) });
  const info = t("info.protocolFees");

  let now;
  try {
    now = await fetchSwapTotals(protocol, period);
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
  const protocolFees = sumUsd(items.map((i) => i.protocolFeesUsd));
  const share = shareOf(protocolFees, sumUsd(items.map((i) => i.feesUsd)));

  return (
    <KpiCell
      label={label}
      info={info}
      value={await valued(usd(protocolFees), items, period)}
      sub={
        share === null
          ? undefined
          : t("protocolFeesShare", { share: formatPercent(String(share), locale) })
      }
      stamp={await lateStamp(now.asOf)}
      advanced
    />
  );
}
