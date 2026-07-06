/**
 * Overview page — the 5 latest signals.
 *
 * Self-contained async Server Component, same contract as
 * `OverviewTopPools`: fetches `GET /api/signals?limit=5` itself and
 * degrades to a `BlockError` on failure so a signal hiccup never takes
 * down the rest of the page. Static snapshot on purpose — the live SSE
 * tail belongs to `/signals`, which the title links to.
 *
 * One compact row per signal: severity icon · pair (short address
 * while unresolved) · severity-colored value · relative time. Each row
 * links to the pool; the wording/summary stays on the feed page.
 */

import { getLocale, getTranslations } from "next-intl/server";

import { BlockError } from "@/components/dashboard/block-error";
import { PoolPairCell } from "@/components/dashboard/pools/pool-pair-cell";
import {
  KNOWN_DETECTORS,
  SEVERITY_COLOR,
  SEVERITY_ICON,
} from "@/components/dashboard/signals/signal-display";
import { Link } from "@/i18n/navigation";
import { ApiClientError } from "@/lib/api/errors";
import type { SignalResponse } from "@/lib/api/schema/signal";
import { fetchSignals } from "@/lib/api/server/signals";
import { formatSignedPercent } from "@/lib/format/format-percent";
import { formatRelativeTime } from "@/lib/format/format-relative-time";
import { formatShortAddress } from "@/lib/format/format-short-address";

const LATEST_COUNT = 5;

export async function OverviewLatestSignals() {
  const t = await getTranslations("Dashboard.Overview.latestSignals");
  const locale = await getLocale();

  let signals: readonly SignalResponse[];
  try {
    signals = (await fetchSignals(LATEST_COUNT)).items;
  } catch (err) {
    if (err instanceof ApiClientError) {
      return <BlockError title={t("title")} kind={err.details.kind} />;
    }
    throw err;
  }

  return (
    <div>
      <div className="mb-4 flex items-baseline justify-between">
        <h2 className="text-[13px] font-semibold tracking-[0.28em] text-slate-400 uppercase">
          {t("title")}
        </h2>
        <Link
          href="/signals"
          className="text-[13px] text-sothoth-200 underline-offset-4 hover:underline"
        >
          {t("seeAll")}
        </Link>
      </div>

      {signals.length === 0 ? (
        <p className="rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/40 px-4 py-6 text-[14px] text-slate-400">
          {t("empty")}
        </p>
      ) : (
        <ul className="overflow-hidden rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/40">
          {signals.map((signal) => (
            <LatestSignalRow key={signal.id} signal={signal} locale={locale} />
          ))}
        </ul>
      )}
    </div>
  );
}

async function LatestSignalRow({
  signal,
  locale,
}: {
  signal: SignalResponse;
  locale: string;
}) {
  const t = await getTranslations("Dashboard.Signals.feed");

  const pairResolved =
    signal.tokenA.symbol !== null && signal.tokenB.symbol !== null;
  const value = KNOWN_DETECTORS.has(signal.detector)
    ? formatSignedPercent(signal.value, locale)
    : signal.value;
  const severityLabel = t(`severity.${signal.severity}`);
  const SeverityIcon = SEVERITY_ICON[signal.severity];

  return (
    <li className="border-b border-sothoth-500/10 last:border-b-0">
      <Link
        href={`/pools/${signal.poolAddress}`}
        className="flex items-center gap-3 px-4 py-3 transition-colors hover:bg-sothoth-500/[0.04]"
      >
        <span
          title={severityLabel}
          className={SEVERITY_COLOR[signal.severity]}
        >
          <SeverityIcon size={20} />
          <span className="sr-only">{severityLabel}</span>
        </span>

        <span className="min-w-0">
          {pairResolved ? (
            <PoolPairCell tokenA={signal.tokenA} tokenB={signal.tokenB} />
          ) : (
            <span className="font-mono text-[14px] text-sothoth-200">
              {formatShortAddress(signal.poolAddress)}
            </span>
          )}
        </span>

        <span
          className={`ml-auto truncate font-mono text-[15px] font-semibold ${SEVERITY_COLOR[signal.severity]}`}
        >
          {value}
        </span>

        <time
          dateTime={signal.triggeredAt}
          className="w-[7.5rem] text-right text-[13px] whitespace-nowrap text-slate-500"
        >
          {formatRelativeTime(signal.triggeredAt, locale)}
        </time>
      </Link>
    </li>
  );
}
