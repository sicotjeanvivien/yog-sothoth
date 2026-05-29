/**
 * Traversal direction relative to a cursor.
 *
 * `next` moves further into the list (older pools), `prev` moves
 * back toward newer pools. Defaults to `next` server-side when
 * unspecified.
 */
export type PageDir = "next" | "prev";

/**
 * Absolute jump to a list boundary, ignoring any cursor.
 */
export type PagePosition = "first" | "last";


export type SortColumn = "first_seen" | "last_seen";
export type SortDir = "asc" | "desc";

export type PoolSort =
  | "first_seen_asc"
  | "first_seen_desc"
  | "last_seen_asc"
  | "last_seen_desc";

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

/// Compute the target sort when the user clicks a column header.
/// - Same column as current sort → flip direction.
/// - Different column → start at desc (the "most recent first" intent
///   is the most intuitive default for both timestamp columns).
export function nextSort(current: PoolSort, clickedColumn: SortColumn): PoolSort {
  const { column: currentColumn, dir: currentDir } = parseSortValue(current);
  if (currentColumn === clickedColumn) {
    return buildSortValue(clickedColumn, currentDir === "asc" ? "desc" : "asc");
  }
  return buildSortValue(clickedColumn, "desc");
}