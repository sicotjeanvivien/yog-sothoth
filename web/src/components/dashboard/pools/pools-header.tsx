/**
 * Pools page header.
 *
 * Title and short description above the table. Intentionally
 * minimal: the future search bar and filter chips will land here,
 * to the right of the eyebrow/title block. For this commit, the
 * right column is empty — keeping the grid in place avoids a
 * layout shift when search/filters are added.
 */

import { getTranslations } from "next-intl/server";
import { PoolsSearch } from "./pools-search";

export async function PoolsHeader() {
  const t = await getTranslations("Dashboard.Pools.page");

  return (
    <header className="px-6 pt-8 pb-6 lg:px-10 lg:pt-10">
      <div className="grid grid-cols-1 items-end gap-4 lg:grid-cols-[1fr_auto]">
        <div>
          <p className="text-[12px] font-semibold tracking-[0.28em] text-slate-400 uppercase">
            {t("eyebrow")}
          </p>
          <h1 className="mt-2 font-display text-[28px] leading-[1.15] font-bold tracking-[0.03em] text-[#f5f2ff] lg:text-[34px]">
            {t("title")}
          </h1>
          <p className="mt-3 max-w-[68ch] text-[15px] leading-[1.6] text-slate-400">
            {t("description")}
          </p>
        </div>

        <div className="flex justify-start lg:justify-end">
          <PoolsSearch />
        </div>
        <div />
      </div>
    </header>
  );
}