/**
 * Clickable column header for the pools table.
 *
 * Renders as a Server Component — the click target is a next-intl
 * <Link> with a precomputed href. No client state, no event handlers.
 *
 * Cycle semantics: clicking a column that is already the active sort
 * flips its direction. Clicking a different column lands on `desc`
 * (the "most recent first" default, intuitive for both timestamp
 * columns).
 *
 * Sort change resets pagination — the cursor pointed into the
 * previous ordering and is no longer valid. The href deliberately
 * drops `cursor`, `dir` and `position`.
 *
 * Other unrelated params (q, future filters) are preserved.
 */

import { Link } from "@/i18n/navigation";

import { ChevronUpSortableIcon, ChevronDownSortableIcon } from "@/components/shared/icon";
import { nextSort, parseSortValue, type SortColumn } from "@/lib/api/type/pagination";
import { PoolSort } from "@/lib/api/type/pagination";
import { getTranslations } from "next-intl/server";

type SortableHeaderProps = {
  /// Column this header represents. Determines what the click means.
  column: SortColumn;
  /// Visible label.
  label: string;
  /// Currently active sort, from the URL.
  currentSort: PoolSort;
  /// All current search params. Unrelated ones are preserved; the
  /// pagination triplet (cursor/dir/position) is dropped on click.
  searchParams: Record<string, string | string[] | undefined>;
  /// Base path of the listing.
  basePath: string;
  /// Optional alignment ("left" | "right") for numeric vs text columns.
  align?: "left" | "right";
};

 export async function SortableHeader({
  column,
  label,
  currentSort,
  searchParams,
  basePath,
  align = "left",
}: SortableHeaderProps) {
  const t = await getTranslations("Dashboard.Pools.table");

  const { column: activeColumn, dir: activeDir } = parseSortValue(currentSort);
  const isActive = activeColumn === column;
  const target = nextSort(currentSort, column);

  const href = buildHref(basePath, searchParams, target);

  const ariaSort = isActive
    ? activeDir === "asc"
      ? "ascending"
      : "descending"
    : "none";

  const ariaLabel = isActive
    ? activeDir === "asc"
      ? t("sortedAscending", { column: label })
      : t("sortedDescending", { column: label })
    : t("sortBy", { column: label });

  const alignmentClass = align === "right" ? "justify-end" : "justify-start";

  return (
    <Link
      href={href}
      aria-label={ariaLabel}
      aria-sort={ariaSort as "ascending" | "descending" | "none"}
      className={`
        inline-flex items-center gap-1.5 ${alignmentClass}
        text-[11px] font-medium tracking-[0.06em] text-slate-500 uppercase
        whitespace-nowrap
        transition-colors hover:text-slate-300
        focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sothoth-400
        rounded
      `}
    >
      <span>{label}</span>
      {isActive ? (
        activeDir === "asc" ? (
          <ChevronUpSortableIcon className="h-3 w-3" aria-hidden="true" />
        ) : (
          <ChevronDownSortableIcon className="h-3 w-3" aria-hidden="true" />
        )
      ) : null}
    </Link>
  );
}

/**
 * Build the target href: keep every unrelated param, set the new
 * `sort`, drop the three pagination params.
 */
function buildHref(
  basePath: string,
  searchParams: Record<string, string | string[] | undefined>,
  targetSort: PoolSort,
): string {
  const next = new URLSearchParams();

  for (const [key, value] of Object.entries(searchParams)) {
    if (key === "sort") continue;
    if (key === "cursor" || key === "dir" || key === "position") continue;
    if (value === undefined) continue;
    if (Array.isArray(value)) {
      for (const v of value) next.append(key, v);
    } else {
      next.set(key, value);
    }
  }

  next.set("sort", targetSort);

  const qs = next.toString();
  return qs.length > 0 ? `${basePath}?${qs}` : basePath;
}