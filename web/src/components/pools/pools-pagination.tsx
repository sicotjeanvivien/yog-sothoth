/**
 * Pagination footer for the pools page.
 *
 * Implementation matches the commit's decisions:
 *
 *   - Forward-only: a "Next" link, present only when yog-api returned
 *     a `next_cursor`.
 *   - A "First page" link, always present, that resets `?cursor=` and
 *     brings the user back to the head of the list.
 *   - No "Previous" button — the cursor is forward-only by design.
 *     Browser back is the recommended way to revisit a previous page
 *     within the same session.
 *
 * Rendered as plain `<a>` tags (Next.js `<Link>` would offer
 * client-side navigation, but per the commit's choice we deliberately
 * reload the Server Component on every page change for the dense
 * data freshness this affords).
 */

type PoolsPaginationProps = {
  /** Path used as the base for navigation links (e.g. "/en/pools"). */
  basePath: string;
  /** Cursor opaque token returned by yog-api, or null if no next page. */
  nextCursor: string | null;
  labels: {
    next: string;
    firstPage: string;
  };
};

export function PoolsPagination({ basePath, nextCursor, labels }: PoolsPaginationProps) {
  const nextHref =
    nextCursor !== null
      ? `${basePath}?cursor=${encodeURIComponent(nextCursor)}`
      : null;

  return (
    <nav
      className="flex items-center justify-between gap-3 pt-4"
      aria-label="Pagination"
    >
      <a
        href={basePath}
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
        // Disabled state — kept in the layout to avoid the First-page
        // button jumping right when we hit the last page.
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