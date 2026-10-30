/**
 * The most traded tokens over the period, all protocols together. A token whose volume
 * covers only part of its pool-hours carries the coverage mark.
 */

import { getTranslations } from "next-intl/server";

import { ApiClientError } from "@/lib/api/errors";
import { fetchTopTokens } from "@/lib/api/server/tokens";
import { volumeCoverage } from "@/lib/coverage/volume-coverage";
import { formatCount } from "@/lib/format/format-count";
import { formatShortAddress } from "@/lib/format/format-short-address";
import { formatUsdCompact } from "@/lib/format/format-usd";
import type { Period } from "@/lib/overview/period";

import { BlockHeading, closedReason, PeriodNotice } from "./overview-parts";
import {
  CELL_CLASS,
  HEAD_ROW_CLASS,
  MIN_HEIGHT,
  NUM_CLASS,
  ROW_CLASS,
  SPAN,
} from "./overview-layout";
import { BlockFailed, BlockSkeleton } from "./overview-states";
import { VolumeCoverageMark } from "./volume-coverage-mark";

const TOP_TOKENS = 10;

const COLUMNS = "grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)_minmax(0,0.6fr)]";

const SECTION_CLASS = `${CELL_CLASS} ${SPAN.main} ${MIN_HEIGHT.tokens}`;

export async function TopTokens({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview");
  const short = t(`period.short.${period}`);
  const title = t("tokens.title", { period: short });

  let page;
  try {
    page = await fetchTopTokens(period, TOP_TOKENS);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section aria-label={title} className={SECTION_CLASS}>
          <BlockHeading title={title} info={t("tokens.info")} />
          <BlockFailed kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  const closed = await closedReason(period, page);

  return (
    <section aria-label={title} className={SECTION_CLASS}>
      <BlockHeading title={title} info={t("tokens.info")} />
      {closed !== null ? (
        <PeriodNotice reason={closed} />
      ) : page.items.length === 0 ? (
        <p className="py-6 text-[14px] text-dash-ink-2">{t("tokens.empty")}</p>
      ) : (
        <div role="table" aria-label={t("tokens.tableLabel")}>
          <div role="row" className={`${HEAD_ROW_CLASS} ${COLUMNS}`}>
            <div role="columnheader">{t("tokens.token")}</div>
            <div role="columnheader" className="text-right">
              {t("tokens.volume", { period: short })}
            </div>
            <div role="columnheader" className="text-right">
              {t("tokens.pools")}
            </div>
          </div>
          {page.items.map((item, i) => (
            <div key={item.token.mint ?? i} role="row" className={`${ROW_CLASS} ${COLUMNS}`}>
              <div role="cell" className="truncate">
                <span className="text-dash-ink">
                  {item.token.symbol ??
                    (item.token.mint ? formatShortAddress(item.token.mint) : "—")}
                </span>
                {item.token.name && (
                  <span className="ml-1.5 text-[12px] text-dash-ink-3">{item.token.name}</span>
                )}
              </div>
              <div role="cell" className={NUM_CLASS}>
                {formatUsdCompact(item.volumeUsd)}
                <VolumeCoverageMark
                  coverage={volumeCoverage(item)}
                  label={(priced, total) => t(`coverage.${page.coverageUnit}.mark`, { priced, total })}
                />
              </div>
              <div role="cell" className={`${NUM_CLASS} text-dash-ink-2`}>
                {formatCount(item.pools)}
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

export async function TopTokensPlaceholder({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview");
  return (
    <BlockSkeleton
      title={t("tokens.title", { period: t(`period.short.${period}`) })}
      span={SPAN.main}
      minHeight={MIN_HEIGHT.tokens}
      shape={{ rows: 11 }}
    />
  );
}
