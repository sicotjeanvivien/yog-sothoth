/**
 * The Signals tab of a protocol's page: its signals, newest first, a page at a time, filtered by
 * severity, read when the page opens. The page's period does not apply, and the tab says so.
 */

import { getLocale, getTranslations } from "next-intl/server";

import {
  CELL_CLASS,
  GRID_CLASS,
  HEAD_ROW_CLASS,
  NUM_CLASS,
  ROW_CLASS,
  SEGMENT_CLASS,
} from "@/components/dashboard/overview/overview-layout";
import { BlockFailed } from "@/components/dashboard/overview/overview-states";
import { SeverityBadge } from "@/components/dashboard/signals/severity-badge";
import {
  formatSignalThreshold,
  formatSignalValue,
} from "@/components/dashboard/signals/signal-display";
import { signalHeadline } from "@/components/dashboard/signals/signal-headline";
import type { PagePath } from "@/config/site";
import { Link } from "@/i18n/navigation";
import { ApiClientError } from "@/lib/api/errors";
import type {
  PageResponseSignalResponse,
  ProtocolResponse,
  SeverityResponse,
} from "@/lib/api/schema/zod.gen";
import { fetchProtocolSignals, SIGNALS_PAGE_SIZE } from "@/lib/api/server/signals";
import { formatUtcMoment, type FormatLocale } from "@/lib/format/date";
import { pageQuery, type Period } from "@/lib/overview/period";
import { signalsQueryParams, type SignalsQuery } from "@/lib/protocol/signals-query";
import { tabQuery } from "@/lib/protocol/tabs";

const FILTERS: readonly (SeverityResponse | null)[] = [null, "critical", "warning", "info"];

const COLUMNS = "grid-cols-[76px_140px_minmax(0,1fr)_140px]";

const ON_CLASS = `${SEGMENT_CLASS} bg-dash-accent font-semibold text-dash-surface`;

const LINK_CLASS = `${SEGMENT_CLASS} text-dash-ink-2 hover:bg-dash-surface hover:text-dash-ink`;

const OFF_CLASS = `${SEGMENT_CLASS} text-dash-ink-3/60`;

type Href = { pathname: PagePath; query: Record<string, string> };

type TabProps = {
  protocol: ProtocolResponse;
  pathname: PagePath;
  period: Period;
  query: SignalsQuery;
};

export async function SignalsTab({ protocol, pathname, period, query }: TabProps) {
  const t = await getTranslations("Dashboard.Protocol.signals");
  const tSeverity = await getTranslations("Dashboard.Signals.feed.severity");
  const tHeadline = await getTranslations("Dashboard.Overview.signals");
  const locale = (await getLocale()) as FormatLocale;
  const href = (next: SignalsQuery): Href => ({
    pathname,
    query: pageQuery(period, { ...tabQuery("signals"), ...signalsQueryParams(next) }),
  });

  let read;
  try {
    read = await readPage(protocol, query);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <div className={GRID_CLASS}>
          <section aria-label={t("label")} className={`${CELL_CLASS} col-span-12`}>
            <BlockFailed kind={err.details.kind} />
          </section>
        </div>
      );
    }
    throw err;
  }
  const { page, severity } = read;

  return (
    <div className={GRID_CLASS}>
      <section aria-label={t("label")} className={`${CELL_CLASS} col-span-12`}>
        <div className="mb-3 flex flex-wrap items-center justify-between gap-x-4 gap-y-2.5">
          <div role="group" aria-label={t("filterLabel")} className="inline-flex border border-dash-rule">
            {FILTERS.map((filter) => {
              const label = filter === null ? t("all") : tSeverity(filter);
              return filter === severity ? (
                <span key={filter ?? "all"} aria-current="page" className={ON_CLASS}>
                  {label}
                </span>
              ) : (
                <Link
                  key={filter ?? "all"}
                  href={href({ severity: filter, page: null })}
                  scroll={false}
                  className={LINK_CLASS}
                >
                  {label}
                </Link>
              );
            })}
          </div>
          <p className="text-[12px] text-dash-ink-2">{t("periodIgnored")}</p>
        </div>

        {page.items.length === 0 ? (
          <p className="py-6 text-[14px] text-dash-ink-2">
            {severity === null ? t("empty") : t("emptyFiltered")}
          </p>
        ) : (
          <SignalTable page={page} t={t} tHeadline={tHeadline} tSeverity={tSeverity} locale={locale} />
        )}

        <div className="mt-3.5 flex flex-wrap items-center justify-between gap-2.5">
          <p className="text-[12px] text-dash-ink-2">{t("pageSize", { count: SIGNALS_PAGE_SIZE })}</p>
          <div role="group" aria-label={t("pagesLabel")} className="inline-flex border border-dash-rule">
            <PageLink
              label={t("newer")}
              cursor={page.isFirst ? null : page.prevCursor}
              href={(cursor) => href({ severity, page: { cursor, dir: "prev" } })}
            />
            <PageLink
              label={t("older")}
              cursor={page.isLast ? null : page.nextCursor}
              href={(cursor) => href({ severity, page: { cursor, dir: "next" } })}
            />
          </div>
        </div>
      </section>
    </div>
  );
}

/**
 * The page asked for. ⚠️ A cursor the API refuses (400) reads as the first page, filter kept:
 * the cursor is opaque, only the API can tell a bad one.
 */
async function readPage(
  protocol: ProtocolResponse,
  query: SignalsQuery,
): Promise<{ page: PageResponseSignalResponse; severity: SeverityResponse | null }> {
  try {
    return { page: await fetchProtocolSignals(protocol, query), severity: query.severity };
  } catch (err) {
    const refused =
      err instanceof ApiClientError && err.details.kind === "http" && err.details.status === 400;
    if (refused && query.page !== null) {
      const first = { severity: query.severity, page: null };
      return { page: await fetchProtocolSignals(protocol, first), severity: query.severity };
    }
    throw err;
  }
}

type Translator = Awaited<ReturnType<typeof getTranslations>>;

function SignalTable({
  page,
  t,
  tHeadline,
  tSeverity,
  locale,
}: {
  page: PageResponseSignalResponse;
  t: Translator;
  tHeadline: Translator;
  tSeverity: Translator;
  locale: FormatLocale;
}) {

  return (
    <div className="overflow-x-auto">
      <div role="table" aria-label={t("tableLabel")} className="min-w-[600px]">
        <div role="row" className={`${HEAD_ROW_CLASS} ${COLUMNS}`}>
          <div role="columnheader">{t("columns.severity")}</div>
          <div role="columnheader">{t("columns.triggered")}</div>
          <div role="columnheader">{t("columns.signal")}</div>
          <div role="columnheader" className={NUM_CLASS}>
            {t("columns.valueThreshold")}
          </div>
        </div>
        {page.items.map((signal) => (
          <div key={signal.id} role="row" className={`${ROW_CLASS} ${COLUMNS}`}>
            <div role="cell">
              <SeverityBadge severity={signal.severity} label={tSeverity(signal.severity)} />
            </div>
            <div role="cell" className="font-dash-mono text-[12px] text-dash-ink-2 tabular-nums">
              <time dateTime={signal.triggeredAt}>
                {formatUtcMoment(signal.triggeredAt, locale) ?? "—"}
              </time>
            </div>
            <div role="cell" className="min-w-0">
              <div className="text-dash-ink">{signalHeadline(signal, locale, tHeadline)}</div>
              <div className="mt-0.5 truncate font-dash-mono text-[11px] text-dash-ink-3">
                {signal.detector}
              </div>
            </div>
            <div role="cell" className={`${NUM_CLASS} text-dash-ink`}>
              {formatSignalValue(signal, locale)}{" "}
              <span className="text-dash-ink-3">· {formatSignalThreshold(signal, locale)}</span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

/** A link to the page `cursor` opens; greyed, not a link, at the list's edge. */
function PageLink({
  label,
  cursor,
  href,
}: {
  label: string;
  cursor: string | null;
  href: (cursor: string) => Href;
}) {
  if (cursor === null) {
    return (
      <span aria-disabled="true" className={OFF_CLASS}>
        {label}
      </span>
    );
  }
  return (
    <Link href={href(cursor)} scroll={false} className={LINK_CLASS}>
      {label}
    </Link>
  );
}
