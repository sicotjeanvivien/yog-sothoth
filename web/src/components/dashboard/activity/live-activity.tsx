"use client";

/**
 * The live activity table: the on-chain collections but the swaps, on the page's connection
 * (`LiveProvider`), the latest first. It starts empty and says so; nothing is loaded before.
 */

import { useEffect, useState, type ReactNode } from "react";
import { useLocale, useTranslations } from "next-intl";

import { useLive } from "@/components/dashboard/live/live-provider";
import { STATUS_DOT } from "@/components/dashboard/signals/signal-display";
import { addEvent, markGap, type FeedRow } from "@/lib/activity/activity-feed";
import { activityRow, toActivityItem, type ActivityRow, type DetailPart } from "@/lib/activity/activity-row";
import { formatProtocolLabel } from "@/lib/format/format-protocol";
import { ACTIVITY_TOPICS } from "@/lib/live/live-topics";

import { HEAD_ROW_CLASS, ROW_CLASS } from "@/components/dashboard/overview/overview-layout";

/** The rows that fit the block's height in the advanced mode. */
export const VISIBLE_ROWS = 10;

const COLUMNS = {
  withProtocol: "grid-cols-[64px_minmax(0,0.8fr)_minmax(0,1.3fr)_minmax(0,1fr)_minmax(0,2fr)]",
  withoutProtocol: "grid-cols-[64px_minmax(0,1.3fr)_minmax(0,1fr)_minmax(0,2fr)]",
} as const;

export function LiveActivity({
  heading,
  tableLabel,
  withProtocol,
}: {
  heading: ReactNode;
  tableLabel: string;
  /** A column naming each event's protocol, for a table of every protocol's. */
  withProtocol: boolean;
}) {
  const columns = withProtocol ? COLUMNS.withProtocol : COLUMNS.withoutProtocol;
  const t = useTranslations("Dashboard.Activity");
  const tFeed = useTranslations("Dashboard.Signals.feed");
  const { status, subscribe, onReopen } = useLive();
  const [rows, setRows] = useState<readonly FeedRow[]>([]);

  useEffect(() => {
    const unsubscribes = ACTIVITY_TOPICS.map((topic) =>
      subscribe(topic, (raw) => {
        const item = toActivityItem(topic, raw);
        if (item === null) {
          console.warn(`live ${topic}: invalid event skipped`);
          return;
        }
        setRows((current) => addEvent(current, activityRow(item)));
      }),
    );
    return () => unsubscribes.forEach((unsubscribe) => unsubscribe());
  }, [subscribe]);

  useEffect(
    () => onReopen(() => setRows((current) => markGap(current, new Date().toISOString()))),
    [onReopen],
  );

  return (
    <>
      <div className="flex items-start justify-between gap-3">
        {heading}
        <span className="mt-1 flex shrink-0 items-center gap-2 font-dash-mono text-[11px] text-dash-ink-3">
          {t("scope")}
          <span className={`h-2 w-2 ${STATUS_DOT[status]}`} title={tFeed(`status.${status}`)}>
            <span className="sr-only">{tFeed(`status.${status}`)}</span>
          </span>
        </span>
      </div>

      {rows.length === 0 ? (
        <p className="py-6 text-[14px] text-dash-ink-2">{t("empty")}</p>
      ) : (
        <div className="overflow-x-auto">
          <div role="table" aria-label={tableLabel} className="min-w-[560px]">
            <div role="row" className={`${HEAD_ROW_CLASS} ${columns}`}>
              <div role="columnheader">{t("columns.time")}</div>
              {withProtocol && <div role="columnheader">{t("columns.protocol")}</div>}
              <div role="columnheader">{t("columns.kind")}</div>
              <div role="columnheader">{t("columns.pool")}</div>
              <div role="columnheader">{t("columns.detail")}</div>
            </div>
            {rows.slice(0, VISIBLE_ROWS).map((row) =>
              row.type === "gap" ? (
                <GapRow key={row.key} at={row.at} />
              ) : (
                <EventRow key={row.key} row={row.row} columns={columns} withProtocol={withProtocol} />
              ),
            )}
          </div>
        </div>
      )}
    </>
  );
}

function EventRow({
  row,
  columns,
  withProtocol,
}: {
  row: ActivityRow;
  columns: string;
  withProtocol: boolean;
}) {
  const t = useTranslations("Dashboard.Activity");
  const locale = useLocale();

  return (
    <div role="row" className={`${ROW_CLASS} ${columns}`}>
      <div role="cell" className="font-dash-mono text-[12px] text-dash-ink-2 tabular-nums">
        <time dateTime={row.at}>{formatTime(row.at, locale)}</time>
      </div>
      {withProtocol && (
        <div role="cell" className="truncate text-[13px] text-dash-ink-2">
          {formatProtocolLabel(row.protocol)}
        </div>
      )}
      <div role="cell" className="truncate text-dash-ink">
        {t(`kind.${row.kind}`)}
      </div>
      <div role="cell" className="truncate text-dash-ink">
        {row.pool}
      </div>
      <div role="cell" className="flex min-w-0 gap-x-2.5 truncate font-dash-mono text-[12px]">
        {row.detail.map((part, i) => (
          <Detail key={i} part={part} />
        ))}
      </div>
    </div>
  );
}

function Detail({ part }: { part: DetailPart }) {
  const t = useTranslations("Dashboard.Activity");
  if (part.type === "amount") {
    return (
      <span className="whitespace-nowrap text-dash-ink tabular-nums">
        {part.sign}
        {part.value} {part.unit}
      </span>
    );
  }
  return (
    <span className="whitespace-nowrap text-dash-ink-3">
      {t(`role.${part.role}`)} <span className="text-dash-ink-2">{part.address}</span>
    </span>
  );
}

function GapRow({ at }: { at: string }) {
  const t = useTranslations("Dashboard.Activity");
  const locale = useLocale();
  return (
    <div role="row" className={`${ROW_CLASS} grid-cols-1`}>
      <div role="cell" className="font-dash-mono text-[11px] text-dash-ink-3">
        {t("gap", { at: formatTime(at, locale) })}
      </div>
    </div>
  );
}

/** "21:14:05", in UTC like the signals and the charts. */
function formatTime(iso: string, locale: string): string {
  return new Intl.DateTimeFormat(locale, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23",
    timeZone: "UTC",
  }).format(new Date(iso));
}
