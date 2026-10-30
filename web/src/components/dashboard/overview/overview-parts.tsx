/**
 * The Overview's shared pieces: a KPI cell, a block heading, a "—" that says why, and a
 * change with its arrow. Server Components; the ⓘ is the client island.
 */

import type { ReactNode } from "react";
import { getLocale, getTranslations } from "next-intl/server";

import { InfoPopover } from "@/components/shared/info-popover";
import { formatUtcMoment, type FormatLocale } from "@/lib/format/date";
import { formatSignedPercent } from "@/lib/format/format-percent";
import type { WindowPeriodResponse } from "@/lib/api/schema/zod.gen";

import {
  BLOCK_HEAD_CLASS,
  BLOCK_TITLE_CLASS,
  kpiCellClass,
  LABEL_CLASS,
} from "./overview-layout";

export async function KpiCell({
  label,
  info,
  value,
  sub,
  stamp,
  advanced = false,
}: {
  label: string;
  info: string;
  value: ReactNode;
  sub?: ReactNode;
  /** The hour the figure stops at, shown only while the figures run late. */
  stamp?: string | undefined;
  /** Shown in the advanced mode only. */
  advanced?: boolean;
}) {
  const tShell = await getTranslations("Dashboard.shell");

  return (
    <section
      aria-label={label}
      className={kpiCellClass(advanced)}
    >
      <span className="flex min-h-6 items-center gap-1.5">
        <span className={LABEL_CLASS}>{label}</span>
        <InfoPopover label={tShell("metricInfo")} iconSize={14}>
          {info}
        </InfoPopover>
      </span>
      <span className="font-dash-mono text-[24px] leading-[1.1] font-medium tracking-[-0.02em] text-dash-ink tabular-nums">
        {value}
      </span>
      {sub !== undefined && (
        <span className="text-[12px] leading-[1.45] text-dash-ink-2">{sub}</span>
      )}
      {stamp !== undefined && (
        <span className="font-dash-mono text-[11px] text-dash-warn">{stamp}</span>
      )}
    </section>
  );
}

export async function BlockHeading({
  title,
  info,
  children,
}: {
  title: string;
  info: string;
  children?: ReactNode;
}) {
  const tShell = await getTranslations("Dashboard.shell");

  return (
    <div className={BLOCK_HEAD_CLASS}>
      <div className="flex items-center gap-1.5">
        <h2 className={BLOCK_TITLE_CLASS}>{title}</h2>
        <InfoPopover label={tShell("metricInfo")} iconSize={14}>
          {info}
        </InfoPopover>
      </div>
      {children}
    </div>
  );
}

/** A figure the facts do not cover whole, with the reason one click away. */
export async function Unavailable({ reason }: { reason: string }) {
  const t = await getTranslations("Dashboard.Overview.unavailable");

  return (
    <span className="inline-flex items-center gap-1 align-middle">
      —
      <InfoPopover label={t("label")} iconSize={12}>
        {reason}
      </InfoPopover>
    </span>
  );
}

/** "▲ +3.1 % over 7d": the arrow carries the direction in colour, the sign in text. */
export async function Change({ ratio, suffix }: { ratio: number; suffix: string }) {
  const locale = await getLocale();
  const text = formatSignedPercent(String(ratio), locale);
  // ⚠️ From the rounded figure: an arrow beside "0 %" would show a direction it does not have.
  const shown = Math.sign(ratio) * Math.round(Math.abs(ratio) * 1000);
  const arrow =
    shown > 0 ? (
      <span aria-hidden="true" className="text-[9px] text-dash-up">
        ▲{" "}
      </span>
    ) : shown < 0 ? (
      <span aria-hidden="true" className="text-[9px] text-dash-down">
        ▼{" "}
      </span>
    ) : null;

  return (
    <span className="text-dash-ink">
      {arrow}
      {text} {suffix}
    </span>
  );
}

/** A day and an hour in UTC, in the page's language. */
export async function utcMoment(input: string | Date): Promise<string> {
  const locale = (await getLocale()) as FormatLocale;
  return formatUtcMoment(input, locale) ?? "—";
}

/** Why this window is closed, or `null` when it is open. */
export async function closedReason(span: WindowPeriodResponse): Promise<string | null> {
  if (span.open) {
    return null;
  }
  const t = await getTranslations("Dashboard.Overview.period");
  if (span.asOf === null || span.coveredSince === null) {
    return t("closedEmpty");
  }
  if (span.opensAt === null) {
    return t("closedSoon");
  }
  return t("closedUntil", {
    since: await utcMoment(span.coveredSince),
    opens: await utcMoment(span.opensAt),
  });
}

/** A block's line for a period its facts do not cover whole, in place of its rows. */
export function PeriodNotice({ reason }: { reason: string }) {
  return <p className="py-6 text-[14px] text-dash-ink-2">— {reason}</p>;
}
