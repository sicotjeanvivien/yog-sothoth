/**
 * A sortable column header, a Server Component of precomputed `<Link>`s: the active column
 * flips, another lands on `desc`. ⚠️ The href drops `cursor`/`dir`/`position`, which point into
 * the previous ordering.
 */

import { Link } from "@/i18n/navigation";

import { ChevronUpSortableIcon, ChevronDownSortableIcon } from "@/components/shared/icon";
import { nextSort, parseSortValue, type SortColumn } from "@/lib/api/type/pagination";
import { PoolSort } from "@/lib/api/type/pagination";
import { getTranslations } from "next-intl/server";

type SortableHeaderProps = {
  column: SortColumn;
  label: string;
  currentSort: PoolSort;
  searchParams: Record<string, string | string[] | undefined>;
  basePath: string;
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