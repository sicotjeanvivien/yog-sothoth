/**
 * Pools page header.
 *
 * Slim: title + ⓘ popover (the page description on demand) on the
 * left, the fee filter + search box on the right. Screen height
 * belongs to the table below, not to chrome.
 *
 * The fee-tier option list is fetched here (Server Component) and
 * degrades gracefully: if the call fails the filter renders with just
 * the "all fees" option rather than breaking the whole page — the pool
 * list itself has its own error handling upstream.
 */

import { getTranslations } from "next-intl/server";

import { InfoPopover } from "@/components/shared/info-popover";
import { fetchFeeTiers, type FeeTier } from "@/lib/api/server/fee-tiers";

import { PoolsFeeFilter } from "./pools-fee-filter";
import { PoolsSearch } from "./pools-search";

export async function PoolsHeader() {
  const t = await getTranslations("Dashboard.Pools.page");
  const tShell = await getTranslations("Dashboard.shell");

  const tiers = await fetchFeeTiers().catch(() => [] as FeeTier[]);

  return (
    <header className="px-6 pt-6 pb-4 lg:px-10">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-3">
        <div className="flex items-center gap-2.5">
          <h1 className="font-display text-[20px] leading-[1.2] font-bold tracking-[0.03em] text-[#f5f2ff]">
            {t("title")}
          </h1>
          <InfoPopover label={tShell("pageInfo")}>
            {t("description")}
          </InfoPopover>
        </div>

        <div className="ml-auto flex flex-wrap items-center gap-3">
          <PoolsFeeFilter tiers={tiers} />
          <PoolsSearch />
        </div>
      </div>
    </header>
  );
}
