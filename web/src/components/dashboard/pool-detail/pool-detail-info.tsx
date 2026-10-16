/**
 * The pool detail's "Pool info" (identity) and "Pool analytics" (24h figures to full
 * precision, where the KPI strip shows them compact). A Server Component; only the copy
 * buttons hydrate.
 */

import { getTranslations } from "next-intl/server";
import type { ReactNode } from "react";

import type { EmbeddedTokenResponse, PoolDetailResponse } from "@/lib/api/schema/zod.gen";

import { formatAbsoluteDate } from "@/lib/format/format-absolute-date";
import { formatComputedFeeBps, formatFeeBps, formatFeeSplit } from "@/lib/format/format-fee";
import { formatProtocolLabel } from "@/lib/format/format-protocol";
import { formatUsd, formatUsdShares } from "@/lib/format/format-usd";
import { formatRelativeTime } from "@/lib/format/format-relative-time";
import { formatShortAddress } from "@/lib/format/format-short-address";

import { CopyButton } from "@/components/shared/copy-button";
import { InfoPopover } from "@/components/shared/info-popover";

import { FeeTypeBadge } from "./fee-type-badge";

const CARD_CLASS =
  "rounded-[8px] border border-sothoth-500/15 bg-cosmos-900/40 p-6 lg:p-8";

const SECTION_TITLE_CLASS =
  "text-[12px] font-semibold tracking-[0.2em] text-slate-400 uppercase";

const ROW_CLASS =
  "grid grid-cols-[140px_1fr] gap-4 border-t border-sothoth-500/10 py-3 first:border-t-0";

const LABEL_CLASS = "text-[14px] text-slate-400";

const VALUE_CLASS =
  "flex items-center gap-2 text-[14px] text-slate-200 min-w-0";

export async function PoolDetailInfo({
  pool,
  locale,
}: {
  pool: PoolDetailResponse;
  locale: string;
}) {
  const t = await getTranslations("Dashboard.PoolDetail.info");

  // ⚠️ One group, so the three cuts add up to the "Fees 24h" row above them.
  const [protocolCut, referralCut, lpCut] = formatUsdShares(pool.fees24hUsd, [
    pool.protocolFees24hUsd,
    pool.referralFees24hUsd,
    pool.lpFees24hUsd,
  ]);

  return (
    <section className="mt-6 px-6 lg:px-10">
      <div className="grid grid-cols-1 items-start gap-6 lg:grid-cols-2">
        <div className={CARD_CLASS}>
          <h2 className={SECTION_TITLE_CLASS}>{t("title")}</h2>

          <div className="mt-4">
            <InfoRow label={t("poolAddress")}>
              <span className="truncate font-mono">{pool.poolAddress}</span>
              <CopyButton
                value={pool.poolAddress}
                label={t("copyPoolAddress")}
              />
            </InfoRow>

            <InfoRow label={t("protocol")}>
              <span>{formatProtocolLabel(pool.protocol)}</span>
            </InfoRow>

            <InfoRow label={t("feeTier")} info={t("help.feeTier")}>
              <span>{formatFeeBps(pool.feeBps)}</span>
            </InfoRow>

            {/* Only for a decaying fee: otherwise the tier above is the whole truth. */}
            {pool.meteoraDammV2?.currentFeeBps != null && (
              <InfoRow label={t("currentFee")} info={t("help.currentFee")}>
                <span>
                  {formatComputedFeeBps(pool.meteoraDammV2.currentFeeBps)}
                  {pool.meteoraDammV2.feeSchedulerExpired === true && (
                    <span className="text-muted-foreground">
                      {" "}
                      ({t("feeSchedulerExpired")})
                    </span>
                  )}
                </span>
              </InfoRow>
            )}

            <InfoRow label={t("feeType")} info={t("help.feeType")}>
              <FeeTypeBadge
                baseFeeKind={pool.meteoraDammV2?.baseFeeKind ?? null}
                hasDynamicFee={pool.meteoraDammV2?.hasDynamicFee ?? null}
                labels={{
                  kinds: {
                    constant: t("feeKind.constant"),
                    scheduler_linear: t("feeKind.schedulerLinear"),
                    scheduler_exponential: t("feeKind.schedulerExponential"),
                    rate_limiter: t("feeKind.rateLimiter"),
                    market_cap_scheduler_linear: t(
                      "feeKind.marketCapSchedulerLinear",
                    ),
                    market_cap_scheduler_exponential: t(
                      "feeKind.marketCapSchedulerExponential",
                    ),
                  },
                  dynamic: t("feeKind.dynamic"),
                }}
              />
            </InfoRow>

            <InfoRow label={t("feeSplit")} info={t("help.feeSplit")}>
              <span>
                {formatFeeSplit(
                  pool.meteoraDammV2?.protocolFeePercent ?? null,
                  pool.meteoraDammV2?.referralFeePercent ?? null,
                  {
                    protocol: t("feeSplitProtocol"),
                    referral: t("feeSplitReferral"),
                  },
                )}
              </span>
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
              {/* Relative to now: the SSR text may lag the client by a minute. */}
              <time dateTime={pool.lastSeenAt} suppressHydrationWarning>
                {formatRelativeTime(pool.lastSeenAt, locale)}
              </time>
            </InfoRow>
          </div>
        </div>

        <div className={CARD_CLASS}>
          <h2 className={SECTION_TITLE_CLASS}>{t("analyticsTitle")}</h2>

          <div className="mt-4">
            <InfoRow label={t("tvl")}>
              <span>{formatUsd(pool.tvlUsd)}</span>
            </InfoRow>

            <InfoRow label={t("volume24h")}>
              <span>{formatUsd(pool.volume24hUsd)}</span>
            </InfoRow>

            <InfoRow label={t("fees24h")}>
              <span>{formatUsd(pool.fees24hUsd)}</span>
            </InfoRow>

            <InfoRow label={t("effectiveFee")} info={t("help.effectiveFee")}>
              <span>{formatFeeBps(pool.effectiveFeeBps)}</span>
            </InfoRow>

            <InfoRow label={t("protocolCut")} info={t("help.protocolCut")}>
              <span>{protocolCut}</span>
            </InfoRow>

            <InfoRow label={t("referralCut")} info={t("help.referralCut")}>
              <span>{referralCut}</span>
            </InfoRow>

            <InfoRow label={t("lpCut")} info={t("help.lpCut")}>
              <span>{lpCut}</span>
            </InfoRow>
          </div>
        </div>
      </div>
    </section>
  );
}

async function InfoRow({
  label,
  children,
  info,
}: {
  label: string;
  children: ReactNode;
  /** Definition of the row, shown in an ⓘ popover next to the label. */
  info?: string;
}) {
  const tShell = info ? await getTranslations("Dashboard.shell") : null;

  return (
    <div className={ROW_CLASS}>
      <div className={`${LABEL_CLASS} flex items-center gap-1.5`}>
        {label}
        {info && tShell && (
          <InfoPopover label={tShell("metricInfo")} iconSize={14}>
            {info}
          </InfoPopover>
        )}
      </div>
      <div className={VALUE_CLASS}>{children}</div>
    </div>
  );
}

/** `SYMBOL  mint…  [copy]`; the symbol is "—" for a token never enriched. */
function TokenLine({
  token,
  copyLabel,
}: {
  token: EmbeddedTokenResponse;
  copyLabel: string;
}) {
  return (
    <>
      <span className="font-medium text-slate-100">{token.symbol ?? "—"}</span>
      <span className="truncate font-mono text-slate-400">
        {token.mint ? formatShortAddress(token.mint) : "—"}
      </span>
      {token.mint && <CopyButton value={token.mint} label={copyLabel} />}
    </>
  );
}
