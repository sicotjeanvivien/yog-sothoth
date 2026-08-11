/**
 * Notice shown when pools became active after the current listing was
 * anchored.
 *
 * `last_seen_at` is rewritten on every event touching a pool, so a listing
 * sorted on it is pinned to the instant it started (`asOf`) and reads only
 * pools at or below that anchor. Without the anchor, a pool touched
 * mid-traversal would cross the pagination cursor and be skipped outright.
 * With it, the pool leaves the traversal instead — it has moved to the end of
 * the live ordering, which under `last_seen_desc` means the head of the list
 * and under `last_seen_asc` its tail.
 *
 * Either way this page cannot show it. The difference this notice makes is
 * that the reader is told, and given the one action that resolves it: re-anchor
 * at the end of the ordering where the pool now sits, keeping their search and
 * filters. Silence here would be the same data loss without the
 * acknowledgement.
 *
 * # Two things this count is not
 *
 * It is **not** "pools you missed": a pool the reader already read on an
 * earlier page and which was touched again since is counted too. Once touched,
 * a pool's previous position is gone from the database — nothing can tell the
 * two apart after the fact — so the count is an upper bound, and the wording
 * says only what is certain ("became active since {when}").
 *
 * It is **not** relative to now: `asOf` is stamped in the URL, so a bookmarked
 * or shared page replays its own anchor however old it is. That is why the
 * message names the anchor instant instead of saying "since this page loaded",
 * which would be a lie on a link opened tomorrow.
 *
 * Renders wherever the count is non-zero, first page included — a backward
 * navigation that lands back on page 1 still carries a cursor, so it carries a
 * real count, and the action still resolves it (dropping the cursor re-anchors
 * the listing and brings the moved pools back into view).
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

  // A touched pool is the most recently active there is, so it sits at the
  // end of the ordering: the head of the list under `desc`, the tail under
  // `asc`. Sending the reader to the head in both cases would point them at
  // the *least* recently active pools half the time.
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
