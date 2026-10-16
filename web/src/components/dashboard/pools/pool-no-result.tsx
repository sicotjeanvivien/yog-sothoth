/**
 * No pool matches the search or fee filter — unlike `PoolsEmpty`, nothing indexed. The CTA
 * clears everything.
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
