/**
 * Empty state for a page emptied *by the traversal itself*.
 *
 * Distinct from the other two empties, and the distinction matters because
 * both alternatives would state something false:
 *
 *   - <PoolsEmpty />     "no pools observed yet" — but the index is full;
 *   - <PoolsNoResults /> "nothing matches your filters" — but they match
 *                        plenty, those pools just moved.
 *
 * What happened instead: every pool still ahead of the cursor became active
 * while the reader was reading, so all of them moved out of this anchored
 * listing (see <PoolsMovedNotice />, rendered directly above this and
 * carrying the way out — an empty page has no cursor of its own, so this
 * state would otherwise be a dead end).
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
