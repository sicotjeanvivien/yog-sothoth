/**
 * A page emptied by the traversal itself: every pool ahead moved out of the anchored listing.
 * Neither "nothing observed" nor "no match"; `PoolsMovedNotice`, above it, is the way out.
 */

import { getTranslations } from "next-intl/server";

export async function PoolsEmptied() {
  const t = await getTranslations("Dashboard.Pools.movedNotice");

  return (
    <div className="mx-6 rounded-[8px] border border-sothoth-500/15 bg-cosmos-900/40 px-6 py-12 text-center lg:mx-10">
      <p className="text-[15px] text-slate-300">{t("emptyTitle")}</p>
      <p className="mt-2 text-[14px] text-slate-500">{t("emptyDescription")}</p>
    </div>
  );
}
