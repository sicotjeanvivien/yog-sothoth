/**
 * The Liquidity tab's locked liquidity, in the advanced mode: at the last TVL reading, its
 * permanent and vesting parts, the tokens most locked, and the largest releases of the next days.
 */

import { getLocale, getTranslations } from "next-intl/server";

import {
  ADVANCED_ONLY,
  CELL_CLASS,
  LABEL_CLASS,
  NUM_CLASS,
  ROW_CLASS,
} from "@/components/dashboard/overview/overview-layout";
import { BlockHeading, PeriodNotice } from "@/components/dashboard/overview/overview-parts";
import { BlockFailed } from "@/components/dashboard/overview/overview-states";
import { ApiClientError } from "@/lib/api/errors";
import type { EmbeddedTokenResponse, ProtocolResponse } from "@/lib/api/schema/zod.gen";
import {
  fetchTokenTvl,
  fetchTvl,
  fetchVestingUnlocks,
} from "@/lib/api/server/protocols";
import { formatRelative, formatUtcMoment, type FormatLocale } from "@/lib/format/date";
import { formatPercent } from "@/lib/format/format-percent";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { pairLabel, shortenPubkey } from "@/lib/format/pubkey";
import { amountOf, shareOf } from "@/lib/overview/totals";
import { lockedTokensOf } from "@/lib/protocol/locked";

const TOKENS_SHOWN = 4;

const UNLOCKS_SHOWN = 5;

/** Beside the flows and the moves, on both their rows. */
export const LOCKED_SPAN = `col-span-12 dash-wide:col-span-4 dash-wide:row-span-2 ${ADVANCED_ONLY.block}`;

export const LOCKED_MIN_HEIGHT = "min-h-[420px]";

const LOCKED_CLASS = `${CELL_CLASS} ${LOCKED_SPAN} ${LOCKED_MIN_HEIGHT}`;

const UNLOCK_COLUMNS = "grid-cols-[auto_minmax(0,1fr)_auto]";

type BlockProps = { protocol: ProtocolResponse };

export async function LockedLiquidity({ protocol }: BlockProps) {
  const t = await getTranslations("Dashboard.Protocol.liquidity.locked");
  const locale = (await getLocale()) as FormatLocale;
  const title = t("title");

  let reading, tokens, unlocks;
  try {
    [reading, tokens, unlocks] = await Promise.all([
      fetchTvl(protocol),
      fetchTokenTvl(protocol, "locked", TOKENS_SHOWN),
      fetchVestingUnlocks(protocol, "7d", "amount", UNLOCKS_SHOWN),
    ]);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section aria-label={title} className={LOCKED_CLASS}>
          <BlockHeading title={title} info={t("info")} />
          <BlockFailed kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  const block = reading.items[0]?.meteoraDammV2 ?? null;
  if (block === null) {
    return (
      <section aria-label={title} className={LOCKED_CLASS}>
        <BlockHeading title={title} info={t("info")} />
        <PeriodNotice reason={t("notMeasured")} />
      </section>
    );
  }

  const locked = amountOf(block.lockedUsd);
  const permanentShare = shareOf(amountOf(block.permanentUsd), locked);
  const vestingShare = shareOf(amountOf(block.vestingUsd), locked);
  const percent = (share: number | null) =>
    share === null ? "—" : formatPercent(String(share), locale);
  const byToken = lockedTokensOf(tokens);
  const tokenLabel = (token: EmbeddedTokenResponse) =>
    token.symbol ?? (token.mint === null ? "—" : shortenPubkey(token.mint));
  const rows = [
    ...byToken.tokens.map((row) => ({
      key: row.token.mint ?? tokenLabel(row.token),
      label: tokenLabel(row.token),
      usd: row.usd,
      share: row.share,
    })),
    ...(byToken.others === null
      ? []
      : [{ key: "others", label: t("others"), ...byToken.others }]),
  ];
  const now = new Date();

  return (
    <section aria-label={title} className={LOCKED_CLASS}>
      <BlockHeading title={title} info={t("info")} />
      <p className="font-dash-mono text-[28px] leading-tight font-semibold tabular-nums text-dash-ink">
        {formatUsdCompact(block.lockedUsd)}
      </p>
      <div
        role="img"
        aria-label={t("splitLabel", {
          permanent: formatUsdCompact(block.permanentUsd),
          vesting: formatUsdCompact(block.vestingUsd),
        })}
        className="mt-2.5 flex h-2.5 w-full overflow-hidden bg-dash-surface-2"
      >
        <div className="bg-dash-lock-permanent" style={{ width: `${(permanentShare ?? 0) * 100}%` }} />
        <div className="bg-dash-lock-vesting" style={{ width: `${(vestingShare ?? 0) * 100}%` }} />
      </div>
      <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1.5 text-[12px] text-dash-ink-2">
        <span>
          <span aria-hidden="true" className="mr-1.5 inline-block h-2.5 w-2.5 bg-dash-lock-permanent align-[-1px]" />
          {t("permanent")}{" "}
          <span className="font-dash-mono tabular-nums">{formatUsdCompact(block.permanentUsd)}</span>
        </span>
        <span>
          <span aria-hidden="true" className="mr-1.5 inline-block h-2.5 w-2.5 bg-dash-lock-vesting align-[-1px]" />
          {t("vesting")}{" "}
          <span className="font-dash-mono tabular-nums">{formatUsdCompact(block.vestingUsd)}</span>
        </span>
      </div>
      <p className="mt-1.5 text-[12px] text-dash-ink-3">
        {t("coverage", { pools: block.poolsLocked, unvalued: block.poolsLockedUnvalued })}
        {block.vestingCoveredSince !== null &&
          t("coverageSince", { date: formatUtcMoment(block.vestingCoveredSince, locale) ?? "—" })}
      </p>

      <p className={`${LABEL_CLASS} mt-[18px] mb-1`}>{t("byToken")}</p>
      {rows.length === 0 ? (
        <p className="py-2 text-[14px] text-dash-ink-2">{t("byTokenEmpty")}</p>
      ) : (
        <ul aria-label={t("byTokenLabel")}>
          {rows.map((row) => (
            <li key={row.key} className="py-1.5">
              <div className="flex justify-between gap-3 text-[14px]">
                <span className="truncate text-dash-ink">{row.label}</span>
                <span className="font-dash-mono whitespace-nowrap tabular-nums text-dash-ink">
                  {formatUsdCompact(row.usd === null ? null : String(row.usd))}{" "}
                  <span className="text-dash-ink-3">{percent(row.share)}</span>
                </span>
              </div>
              <div aria-hidden="true" className="mt-1 h-1 w-full bg-dash-surface-2">
                <div className="h-1 bg-dash-accent" style={{ width: `${(row.share ?? 0) * 100}%` }} />
              </div>
            </li>
          ))}
        </ul>
      )}

      <p className={`${LABEL_CLASS} mt-[18px] mb-1`}>{t("unlocks")}</p>
      {unlocks.items.length === 0 ? (
        <p className="py-2 text-[14px] text-dash-ink-2">{t("unlocksEmpty")}</p>
      ) : (
        <div role="table" aria-label={t("unlocksLabel")}>
          {unlocks.items.map((unlock) => (
            <div
              key={`${unlock.position}-${unlock.unlocksAt}`}
              role="row"
              className={`${ROW_CLASS} ${UNLOCK_COLUMNS}`}
            >
              <div role="cell" className="font-dash-mono text-[12px] whitespace-nowrap text-dash-ink-2">
                {formatRelative(unlock.unlocksAt, locale, now) ?? "—"}
              </div>
              <div role="cell" className="truncate text-dash-ink">
                {pairLabel(unlock)}
              </div>
              <div role="cell" className={`${NUM_CLASS} text-dash-ink`}>
                {formatUsdCompact(unlock.amountUsd)}
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
