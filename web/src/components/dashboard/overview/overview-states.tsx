/**
 * The Overview's states besides its figures: waiting slots shaped like their block, a
 * block's error, and figures running late. Server Components.
 */

import { getLocale, getTranslations } from "next-intl/server";

import { AlertTriangleIcon } from "@/components/shared/icon";
import { RetryButton } from "@/components/shared/retry-button";
import type { ApiClientErrorKind } from "@/lib/api/errors";
import { ApiClientError } from "@/lib/api/errors";
import { fetchSwapTotals } from "@/lib/api/server/protocols";
import type { Scope } from "@/lib/api/server/scope";
import { formatRelative, type FormatLocale } from "@/lib/format/date";
import type { Period } from "@/lib/overview/period";
import { isStale } from "@/lib/overview/staleness";

import {
  BLOCK_HEAD_CLASS,
  ADVANCED_ONLY,
  BLOCK_TITLE_CLASS,
  CELL_CLASS,
  kpiCellClass,
} from "./overview-layout";
import { utcMoment } from "./overview-parts";

/** A block's error, in place of its rows: the cause in words, and a retry of the page. */
export async function BlockFailed({ kind }: { kind: ApiClientErrorKind }) {
  const t = await getTranslations("Dashboard.Overview.error");
  const tRetry = await getTranslations("Dashboard.BlockError");

  return (
    <div role="alert" className="flex items-start gap-3 border border-dash-rule bg-dash-bg p-4">
      <AlertTriangleIcon size={18} className="mt-0.5 shrink-0 text-dash-warn" />
      <div>
        <p className="font-semibold text-dash-ink">{t("title")}</p>
        <p className="mt-1 max-w-[62ch] text-[14px] text-dash-ink-2">
          {t(kind)} {t("rest")}
        </p>
        <div className="mt-3">
          <RetryButton label={tRetry("retry")} pendingLabel={tRetry("retryPending")} variant="box" />
        </div>
      </div>
    </div>
  );
}

/** "as of 20 Oct, 14:00 UTC" under a figure, only while the figures run late. */
export async function lateStamp(
  asOf: string | null,
  period: Period,
): Promise<string | undefined> {
  if (asOf === null || !isStale(asOf, new Date(), period)) {
    return undefined;
  }
  const t = await getTranslations("Dashboard.Overview.stale");
  return t("stamp", { at: await utcMoment(asOf) });
}

/**
 * The band over the page when its figures stop over two hours back. ⚠️ Read on `asOf`, not
 * on the indexer: "Live" speaks of ingestion, which can be fine while the figures stall.
 */
export async function StaleBanner({ period, scope }: { period: Period; scope: Scope }) {
  const t = await getTranslations("Dashboard.Overview.stale");
  const locale = (await getLocale()) as FormatLocale;

  let asOf;
  try {
    asOf = (await fetchSwapTotals(scope, period)).asOf;
  } catch (err) {
    if (err instanceof ApiClientError) {
      return null;
    }
    throw err;
  }
  const now = new Date();
  if (asOf === null || !isStale(asOf, now, period)) {
    return null;
  }

  return (
    <div
      role="status"
      className="flex items-center gap-3 border-b border-dash-rule bg-dash-warn/10 px-4 py-2.5 text-[14px] text-dash-ink rail:px-8"
    >
      <span aria-hidden="true" className="h-2 w-2 shrink-0 bg-dash-warn" />
      <span>
        <strong>{t("title")}</strong>{" "}
        {t("body", { at: await utcMoment(asOf), ago: formatRelative(asOf, locale, now) ?? "" })}
      </span>
    </div>
  );
}

const BAR = "block dash-skeleton";

/** A KPI's waiting slot: label, figure and line as bars. */
export async function KpiSkeleton({ advanced = false }: { advanced?: boolean }) {
  const t = await getTranslations("Dashboard.shell");
  return (
    <div
      aria-busy="true"
      className={kpiCellClass(advanced)}
    >
      <span className={`${BAR} mt-1.5 h-[11px] w-[72px]`} />
      <span className={`${BAR} mt-1 h-6 w-[132px]`} />
      <span className={`${BAR} h-3 w-24`} />
      <span className="sr-only">{t("loading")}</span>
    </div>
  );
}

/** A block's waiting slot: its title, then rows or a chart as bars. */
export async function BlockSkeleton({
  title,
  span,
  minHeight,
  shape,
}: {
  title: string;
  span: string;
  minHeight: string;
  shape: { rows: number } | "chart";
}) {
  const t = await getTranslations("Dashboard.shell");
  return (
    <section aria-busy="true" className={`${CELL_CLASS} ${span} ${minHeight}`}>
      <div className={BLOCK_HEAD_CLASS}>
        <h2 className={BLOCK_TITLE_CLASS}>{title}</h2>
      </div>
      {shape === "chart" ? (
        <>
          <span className={`${BAR} mt-[26px] h-[170px]`} />
          <span className={`${BAR} mt-[66px] h-20 ${ADVANCED_ONLY.block}`} />
        </>
      ) : (
        Array.from({ length: shape.rows }, (_, i) => (
          <span
            key={i}
            className={`${BAR} my-[13px] h-3.5 adv:my-2.5`}
            style={{ width: `${[92, 78, 88, 70, 84][i % 5]}%` }}
          />
        ))
      )}
      <span className="sr-only">{t("loading")}</span>
    </section>
  );
}

/** The label of a failed KPI and its retry, under its "—". */
export async function KpiFailedLine() {
  const t = await getTranslations("Dashboard.Overview.error");
  const tRetry = await getTranslations("Dashboard.BlockError");
  return (
    <span className="flex flex-col items-start gap-1">
      <span>{t("kpi")}</span>
      <RetryButton label={tRetry("retry")} pendingLabel={tRetry("retryPending")} variant="link" />
    </span>
  );
}
