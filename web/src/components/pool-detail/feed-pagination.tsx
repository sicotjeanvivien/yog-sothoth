/**
 * Pagination footer for the per-pool feeds.
 *
 * Two feeds (swaps, liquidity) coexist on the same page, so each
 * feed's cursor needs its own query parameter to avoid collisions
 * (e.g. `?swaps_cursor=...&liquidity_cursor=...`). The `cursorKey`
 * prop carries the parameter name; the rest of the query string is
 * preserved verbatim across page changes so each feed paginates
 * independently.
 *
 * Forward-only, same convention as `PoolsPagination`: "Next page" if
 * a cursor is available, plus a "First page" link that drops only
 * this feed's cursor (the other feed's cursor is preserved).
 */

type FeedPaginationProps = {
  /** Path used as the base for navigation links (e.g. "/en/pools/<addr>"). */
  basePath: string;
  /** Query parameter name carrying this feed's cursor (e.g. "swaps_cursor"). */
  cursorKey: string;
  /** Cursor opaque token returned by yog-api, or null if no next page. */
  nextCursor: string | null;
  /** The other query parameters to preserve across navigation. */
  preservedParams: URLSearchParams;
  labels: {
    next: string;
    firstPage: string;
  };
};

export function FeedPagination({
  basePath,
  cursorKey,
  nextCursor,
  preservedParams,
  labels,
}: FeedPaginationProps) {
  const nextHref =
    nextCursor !== null
      ? buildHref(basePath, preservedParams, { [cursorKey]: nextCursor })
      : null;

  // "First page" for this feed drops only this feed's cursor; the
  // other feed's cursor stays in the URL.
  const firstHref = buildHref(basePath, preservedParams, { [cursorKey]: null });

  return (
    <nav
      className="flex items-center justify-between gap-3 pt-4"
      aria-label="Pagination"
    >
      <a
        href={firstHref}
        className="inline-flex items-center rounded-md border border-cosmos-700/60 px-3 py-1.5 text-xs uppercase tracking-widest text-slate-400 transition-colors hover:border-sothoth-600/40 hover:text-sothoth-400"
      >
        ← {labels.firstPage}
      </a>

      {nextHref !== null ? (
        <a
          href={nextHref}
          className="inline-flex items-center rounded-md border border-sothoth-600/40 bg-sothoth-600/10 px-4 py-1.5 text-xs uppercase tracking-widest text-sothoth-400 transition-colors hover:bg-sothoth-600/20"
        >
          {labels.next} →
        </a>
      ) : (
        <span
          className="inline-flex cursor-not-allowed items-center rounded-md border border-cosmos-700/40 px-4 py-1.5 text-xs uppercase tracking-widest text-slate-600"
          aria-disabled="true"
        >
          {labels.next} →
        </span>
      )}
    </nav>
  );
}

/**
 * Build a path with the preserved query parameters plus the overrides
 * given by `overrides`. A `null` value in `overrides` deletes the
 * corresponding key.
 */
function buildHref(
  basePath: string,
  preserved: URLSearchParams,
  overrides: Record<string, string | null>,
): string {
  // Clone so we don't mutate the caller's params object.
  const params = new URLSearchParams(preserved);
  for (const [key, value] of Object.entries(overrides)) {
    if (value === null) {
      params.delete(key);
    } else {
      params.set(key, value);
    }
  }
  const qs = params.toString();
  return qs.length > 0 ? `${basePath}?${qs}` : basePath;
}