/**
 * Empty state shown when a search yields no matching pools.
 *
 * Distinct from <PoolsEmpty />, which means "no pools indexed at
 * all". Here the index is non-empty; the active query just doesn't
 * match anything. We echo the query back and offer a way out.
 */

import { CtaLink } from "@/components/shared/cta-link";
import { getTranslations } from "next-intl/server";

export async function PoolsNoResults({ query }: { query: string }) {
  const t = await getTranslations("Dashboard.Pools.noResults");

  return (
    <div className="mx-6 rounded-[8px] border border-sothoth-500/15 bg-cosmos-900/40 px-6 py-12 text-center lg:mx-10">
      <p className="text-[15px] text-slate-300">
        {t("title", { query })}
      </p>
      <p className="mt-2 text-[14px] text-slate-500">{t("hint")}</p>
      <div className="my-5">
        <CtaLink href="/pools" label={t("clearSearch")} />
      </div>
    </div>
  );
}