/**
 * Pool detail page — "Pool info" block.
 *
 * A factual key/value card for the pool's identity, sitting below
 * the header and (eventually) the KPI strip. Rows:
 *
 *   - Pool address  (full, copy-friendly)
 *   - Protocol      (display label)
 *   - Fee tier      (base trading fee, `—` until InitializePool indexed)
 *   - Effective fee (realized 24h rate: fees / volume, `—` when no volume)
 *   - Protocol cut  (Meteora's share of the realized 24h fee, USD)
 *   - Network       (hardcoded "Solana" while we only index Solana)
 *   - Token A      (symbol + truncated mint + copy)
 *   - Token B      (symbol + truncated mint + copy)
 *   - First seen   (absolute date, locale-aware)
 *   - Last activity (relative time, locale-aware)
 *
 * The block is a Server Component end-to-end; only the embedded
 * `CopyButton` islands hydrate on the client. Layout is a two-
 * column CSS grid (label / value), which scales naturally on
 * mobile by widening the value column and stacking when there's no
 * room (handled by the `min-w-0` + `whitespace-nowrap` on the
 * label).
 */

import { getTranslations } from "next-intl/server";
import type { ReactNode } from "react";

import type { PoolResponse } from "@/lib/api/schema/pool";
import type { TokenResponse } from "@/lib/api/schema/token";

import { formatAbsoluteDate } from "@/lib/format/format-absolute-date";
import { formatFeeBps } from "@/lib/format/format-fee";
import { formatProtocolLabel } from "@/lib/format/format-protocol";
import { formatUsd } from "@/lib/format/format-usd";
import { formatRelativeTime } from "@/lib/format/format-relative-time";
import { formatShortAddress } from "@/lib/format/format-short-address";

import { CopyButton } from "./copy-button";

// ── Tailwind class fragments ─────────────────────────────────────────

const CARD_CLASS =
  "mx-6 rounded-[8px] border border-sothoth-500/15 bg-cosmos-900/40 p-6 lg:mx-10 lg:p-8";

const SECTION_TITLE_CLASS =
  "text-[11px] font-semibold tracking-[0.2em] text-slate-400 uppercase";

const ROW_CLASS =
  "grid grid-cols-[140px_1fr] gap-4 border-t border-sothoth-500/10 py-3 first:border-t-0";

const LABEL_CLASS = "text-[13px] text-slate-400";

const VALUE_CLASS =
  "flex items-center gap-2 text-[14px] text-slate-200 min-w-0";

// ── Component ─────────────────────────────────────────────────────────

export async function PoolDetailInfo({
  pool,
  locale,
}: {
  pool: PoolResponse;
  locale: string;
}) {
  const t = await getTranslations("Dashboard.PoolDetail.info");

  return (
    <section className="mt-6">
      <div className={CARD_CLASS}>
        <h2 className={SECTION_TITLE_CLASS}>{t("title")}</h2>

        <div className="mt-4">
          <InfoRow label={t("poolAddress")}>
            <span className="truncate font-mono">{pool.poolAddress}</span>
            <CopyButton value={pool.poolAddress} label={t("copyPoolAddress")} />
          </InfoRow>

          <InfoRow label={t("protocol")}>
            <span>{formatProtocolLabel(pool.protocol)}</span>
          </InfoRow>

          <InfoRow label={t("feeTier")}>
            <span>{formatFeeBps(pool.feeBps)}</span>
          </InfoRow>

          <InfoRow label={t("effectiveFee")}>
            <span>{formatFeeBps(pool.effectiveFeeBps)}</span>
          </InfoRow>

          <InfoRow label={t("protocolCut")}>
            <span>{formatUsd(pool.protocolFees24hUsd)}</span>
          </InfoRow>

          <InfoRow label={t("network")}>
            <span>Solana</span>
          </InfoRow>

          <InfoRow label={t("tokenA")}>
            <TokenLine token={pool.tokenA} copyLabel={t("copyTokenMint")} />
          </InfoRow>

          <InfoRow label={t("tokenB")}>
            <TokenLine token={pool.tokenB} copyLabel={t("copyTokenMint")} />
          </InfoRow>

          <InfoRow label={t("firstSeen")}>
            <time dateTime={pool.firstSeenAt}>
              {formatAbsoluteDate(pool.firstSeenAt, locale)}
            </time>
          </InfoRow>

          <InfoRow label={t("lastActivity")}>
            <time dateTime={pool.lastSeenAt}>
              {formatRelativeTime(pool.lastSeenAt, locale)}
            </time>
          </InfoRow>
        </div>
      </div>
    </section>
  );
}

// ── Sub-components ───────────────────────────────────────────────────

/**
 * Single label/value row. The label column has a fixed width so
 * every row aligns vertically; the value column takes the rest and
 * truncates with ellipsis when needed.
 */
function InfoRow({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <div className={ROW_CLASS}>
      <div className={LABEL_CLASS}>{label}</div>
      <div className={VALUE_CLASS}>{children}</div>
    </div>
  );
}

/**
 * "SYMBOL  mint_truncated  [copy]" line, used for both token rows.
 * The symbol falls back to "—" only when the embedded token has no
 * symbol at all (token-metadata never enriched). The mint is
 * always present.
 */
function TokenLine({
  token,
  copyLabel,
}: {
  token: TokenResponse;
  copyLabel: string;
}) {
  return (
    <>
      <span className="font-medium text-slate-100">
        {token.symbol ?? "—"}
      </span>
      <span className="truncate font-mono text-slate-400">
        {token.mint ? formatShortAddress(token.mint) : "—"}
      </span>
      {token.mint && <CopyButton value={token.mint} label={copyLabel} />}
    </>
  );
}