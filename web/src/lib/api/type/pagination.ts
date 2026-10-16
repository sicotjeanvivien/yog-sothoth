import type * as z from "zod";

import type { zListPoolsQuery } from "../schema/zod.gen";

// The values come from the contract: a value renamed server-side breaks the build here.
type ListPoolsQuery = z.infer<typeof zListPoolsQuery>;

/** `next` moves further into the list, `prev` back; `next` when unspecified. */
export type PageDir = NonNullable<ListPoolsQuery["dir"]>;

export type PagePosition = NonNullable<ListPoolsQuery["position"]>;


export type SortColumn = "first_seen" | "last_seen";
export type SortDir = "asc" | "desc";

export type PoolSort = NonNullable<ListPoolsQuery["sort"]>;

// Maps the URL's `sort` value to its (column, dir) decomposition.
export function parseSortValue(sort: PoolSort): { column: SortColumn; dir: SortDir } {
  switch (sort) {
    case "first_seen_asc": return { column: "first_seen", dir: "asc" };
    case "first_seen_desc": return { column: "first_seen", dir: "desc" };
    case "last_seen_asc": return { column: "last_seen", dir: "asc" };
    case "last_seen_desc": return { column: "last_seen", dir: "desc" };
  }
}

export function buildSortValue(column: SortColumn, dir: SortDir): PoolSort {
  return `${column}_${dir}` as PoolSort;
}

/// The sort a header click targets: the active column flips, another starts at `desc`.
export function nextSort(current: PoolSort, clickedColumn: SortColumn): PoolSort {
  const { column: currentColumn, dir: currentDir } = parseSortValue(current);
  if (currentColumn === clickedColumn) {
    return buildSortValue(clickedColumn, currentDir === "asc" ? "desc" : "asc");
  }
  return buildSortValue(clickedColumn, "desc");
}