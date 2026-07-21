/**
 * Empty state shown when an active refinement (search and/or fee
 * filter) yields no matching pools.
 *
 * Distinct from <PoolsEmpty />, which means "no pools indexed at
 * all". Here the index is non-empty; the active refinement just
 * doesn't match anything.
 *
 * When a text `query` is present we echo it back (the search case);
 * otherwise the refinement is the fee filter alone and we show a
 * filter-neutral message. Either way the CTA clears everything by
 * linking back to the bare `/pools`.
 */

import { CtaLink } from "@/components/shared/cta-link";
import { getTranslations } from "next-intl/server";

export async function PoolsNoResults({
  query,
}: {
  query?: string | undefined;
}) {
  const t = await getTranslations("Dashboard.Pools.noResults");

  return (
    <div className="mx-6 rounded-[8px] border border-sothoth-500/15 bg-cosmos-900/40 px-6 py-12 text-center lg:mx-10">
      <p className="text-[15px] text-slate-300">
        {query !== undefined ? t("title", { query }) : t("titleFiltered")}
      </p>
      {/* The spelling hint only fits the text-search case; the fee filter
          has nothing to misspell. */}
      {query !== undefined && (
        <p className="mt-2 text-[14px] text-slate-500">{t("hint")}</p>
      )}
      <div className="my-5">
        <CtaLink
          href="/pools"
          label={query !== undefined ? t("clearSearch") : t("clearFilters")}
        />
      </div>
    </div>
  );
}
