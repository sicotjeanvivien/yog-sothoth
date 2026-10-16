/**
 * The pools that became active after the listing's anchor (`asOf`) and left the traversal,
 * with the action that re-anchors at the end of the ordering. ⚠️ An upper bound: a pool
 * already read and touched since counts too. It names the anchor, not "since this page
 * loaded": `asOf` travels in the URL.
 */

import { getTranslations } from "next-intl/server";

import { Link } from "@/i18n/navigation";
import { buildHref } from "@/components/shared/pagination-href";
import { formatRelativeTime } from "@/lib/format/format-relative-time";
import { parseSortValue, type PoolSort } from "@/lib/api/type/pagination";

export async function PoolsMovedNotice({
  count,
  asOf,
  sort,
  locale,
  searchParams,
}: {
  count: number;
  asOf: string;
  sort: PoolSort;
  locale: string;
  searchParams: Record<string, string | string[] | undefined>;
}) {
  const t = await getTranslations("Dashboard.Pools.movedNotice");
  const { dir } = parseSortValue(sort);

  // A touched pool is the most recent: the head under `desc`, the tail under `asc`.
  const toHead = dir === "desc";
  const href = toHead
    ? buildHref("/pools", searchParams, "", {})
    : buildHref("/pools", searchParams, "", { position: "last" });

  return (
    <div
      role="status"
      className="mx-6 mb-3 flex flex-wrap items-center justify-between gap-3 rounded-[8px] border border-sothoth-500/20 bg-cosmos-900/40 px-4 py-3 lg:mx-10"
    >
      <p className="text-[14px] text-slate-300">
        {t(toHead ? "messageDesc" : "messageAsc", {
          count,
          when: formatRelativeTime(asOf, locale),
        })}
      </p>
      <Link
        href={href}
        className="text-[14px] font-semibold text-sothoth-300 underline-offset-2 hover:underline"
      >
        {t(toHead ? "actionDesc" : "actionAsc")}
      </Link>
    </div>
  );
}
